//! Sophisticated Backpacks for Horizon: a clean-room port.
//!
//! This crate is the mod's native half. It gives each backpack item a
//! container (`minecraft:container`, the way a shulker box keeps its items),
//! opens an interactive menu for it, moves stacks between that menu and the
//! player, runs the installed upgrades, and gives the placeable block form its
//! slots and the contents of the item it was placed from.
//!
//! Everything here is original code written against Horizon's public mod API
//! (`horizon_client::mod_api`); the original Java mod is All Rights Reserved
//! and neither its code nor its assets were used.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use horizon_client::mod_api::{
    BlockEntityContainerEvent, BlockEntityEvent, BlockEntityTickEvent, BlockEntityTicksEvent,
    Compound, EquipmentSlot, Flow, Hooks, Host, InventoryTickEvent, ItemContainerEvent, ItemStack,
    KeyEvent, MenuClickedEvent, MenuClosedEvent, ModContainer, ModsEvent, NativeMod,
    NetFromClientEvent, PlaceBlockEvent, Text, UseItemEvent,
};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Backpacks)
}

/// Tier id suffix, slot count and menu rows. The menu rows are exactly
/// `slots / 9`, so a menu slot index is a backpack slot index.
const TIERS: [(&str, usize); 5] = [
    ("leather_backpack", 9),
    ("iron_backpack", 18),
    ("gold_backpack", 27),
    ("diamond_backpack", 36),
    ("netherite_backpack", 54),
];

const MOD: &str = "sophisticated_backpacks";

/// The key's name in Options > Mods, and the key it starts as.
const KEY_NAME: &str = "open_backpack";
const KEY_DEFAULT: &str = "key.keyboard.b";
/// The client asks its server to open the worn backpack on this channel.
const CHANNEL_OPEN: &str = "sophisticated_backpacks:open";
/// The chest armour slot, as `inventory_tick` numbers it.
const CHEST_SLOT: i32 = 102;
/// The off hand, as `inventory_tick` numbers it.
const OFFHAND_SLOT: i32 = -106;

/// The item a carried stack is inside a backpack menu, by id.
fn item_slots(item: &str) -> Option<usize> {
    let name = item.strip_prefix(MOD)?.strip_prefix(':')?;
    TIERS
        .iter()
        .find(|(tier, _)| *tier == name)
        .map(|(_, slots)| *slots)
}

/// The backpack item a placed block stands for, by block id.
fn block_info(block: &str) -> Option<(&'static str, usize)> {
    let name = block.strip_prefix(MOD)?.strip_prefix(':')?;
    let tier = name.strip_suffix("_block")?;
    let (item, slots) = TIERS.iter().find(|(known, _)| *known == tier)?;
    Some((item, *slots))
}

fn menu_type(slots: usize) -> &'static str {
    match slots / 9 {
        1 => "minecraft:generic_9x1",
        2 => "minecraft:generic_9x2",
        3 => "minecraft:generic_9x3",
        4 => "minecraft:generic_9x4",
        5 => "minecraft:generic_9x5",
        _ => "minecraft:generic_9x6",
    }
}

/// What the player asked for with a click, done on the next inventory tick,
/// where the acting player's own slots can be written.
#[derive(Clone, Debug)]
enum Action {
    /// From the player's inventory slot into the backpack.
    In { player_slot: usize, amount: u32 },
    /// Out of backpack slot `index` to the player.
    Out { index: usize, amount: u32 },
}

#[derive(Clone, Debug)]
struct Session {
    window: i32,
    /// The player's inventory slot the backpack was in when it was opened.
    slot: i32,
    item: String,
    slots: usize,
    contents: Vec<Option<ItemStack>>,
    actions: Vec<Action>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<u128, Session>,
    /// The selected hotbar slot, from `inventory_tick`'s `selected`.
    selected: HashMap<u128, i32>,
    /// Backpack block contents to install on the first tick of a placed
    /// block, by level and position: `block_entity_tick` isn't told the
    /// dimension, so it is read from the level in place (`host.dimension()`).
    pending: HashMap<(&'static str, [i32; 3]), Vec<Option<ItemStack>>>,
    restock_wait: HashMap<u128, u32>,
    compact_wait: HashMap<u128, u32>,
    pickup_wait: HashMap<u128, u32>,
}

fn state() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The slots of one item of `stack`, empty ones included.
fn contents_of(stack: &ItemStack, size: usize) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; size];
    for (index, item) in stack.data().map_or(&[][..], |data| &data.container[..]) {
        if item.count > 0
            && let Some(slot) = slots.get_mut(usize::from(*index))
        {
            *slot = Some(item.clone());
        }
    }
    slots
}

/// One item of `stack` holding `slots`, its other data kept, as the game
/// writes a shulker box's contents.
fn with_contents(stack: &ItemStack, slots: &[Option<ItemStack>]) -> ItemStack {
    let mut one = stack.clone();
    one.count = 1;
    let data = one.data_mut();
    data.container = slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| Some((i as u8, s.clone().filter(|s| s.count > 0)?)))
        .collect();
    data.wire = Default::default();
    one.tidy();
    one
}

fn same_item_in_slot(held: &ItemStack, item: &ItemStack) -> bool {
    held.item == item.item && held.damage == item.damage && held.data() == item.data()
}

/// Puts up to `want` of `item` into `contents`; how many it placed.
fn insert_into(contents: &mut [Option<ItemStack>], item: &ItemStack, want: u32, max: u32) -> u32 {
    let max = max.clamp(1, 99);
    let mut left = want.min(item.count);
    for slot in contents.iter_mut().flatten() {
        if left == 0 {
            break;
        }
        if same_item_in_slot(slot, item) && slot.count < max {
            let room = (max - slot.count).min(left);
            slot.count += room;
            left -= room;
        }
    }
    for slot in contents.iter_mut() {
        if left == 0 {
            break;
        }
        if slot.is_none() {
            let put = max.min(left);
            *slot = Some(ItemStack {
                count: put,
                ..item.clone()
            });
            left -= put;
        }
    }
    want - left
}

/// Takes up to `want` of `item` out of `contents`; how many it took.
fn take_from(contents: &mut [Option<ItemStack>], item: &ItemStack, want: u32) -> u32 {
    let mut left = want;
    for slot in contents.iter_mut() {
        if left == 0 {
            break;
        }
        let Some(held) = slot.as_mut() else { continue };
        if same_item_in_slot(held, item) {
            let take = held.count.min(left);
            held.count -= take;
            left -= take;
            if held.count == 0 {
                *slot = None;
            }
        }
    }
    want - left
}

fn has_upgrade(contents: &[Option<ItemStack>], upgrade: &str) -> bool {
    let id = format!("{MOD}:{upgrade}");
    contents
        .iter()
        .flatten()
        .any(|stack| stack.item.as_ref() == id.as_str() && stack.count > 0)
}

fn read_slot(host: &mut dyn Host, slot: i32) -> Option<ItemStack> {
    host.carried_stack(slot)
        .filter(|stack| stack.count > 0)
        .cloned()
}

fn write_slot(host: &mut dyn Host, slot: i32, stack: Option<ItemStack>) {
    if let Some(held) = host.carried_stack(slot) {
        match stack {
            Some(stack) => *held = stack,
            // A zero count empties the slot at the end of the inventory tick.
            None => held.count = 0,
        }
    }
}

/// Every slot a backpack may sit in: the 36 inventory slots, the chest armour
/// slot (102) and the off hand.
const BACKPACK_SLOTS: [i32; 38] = {
    let mut slots = [0i32; 38];
    let mut i = 0;
    while i < 36 {
        slots[i] = i as i32;
        i += 1;
    }
    slots[36] = 102;
    slots[37] = -106;
    slots
};

fn hotbar(slot: i32) -> bool {
    (0..9).contains(&slot)
}

/// The player's inventory slot a menu slot past the backpack's own slots
/// names (`ChestMenu`: the menu's slots, then 27, then the hotbar).
fn player_slot(menu_slots: usize, slot: i32) -> Option<usize> {
    let n = usize::try_from(slot).ok()?.checked_sub(menu_slots)?;
    match n {
        0..27 => Some(n + 9),
        27..36 => Some(n - 27),
        _ => None,
    }
}

struct Backpacks;

impl Backpacks {
    fn open(&self, host: &mut dyn Host, stack: &ItemStack) -> Flow {
        let Some(slots) = item_slots(&stack.item) else {
            return Flow::Continue;
        };
        let Some(player) = host.acting_player() else {
            return Flow::Continue;
        };
        let uuid = player.uuid;
        let selected = state().selected.get(&uuid).copied();
        let carried = host.player_stacks(uuid);
        let mut found = selected.filter(|slot| {
            carried
                .as_ref()
                .and_then(|stacks| stacks.get(*slot as usize))
                .and_then(Option::as_ref)
                .is_some_and(|held| held.item == stack.item)
        });
        if found.is_none() {
            // Used from the off hand: `player_stacks` only has the 36
            // inventory slots, so open the backpack where it lies.
            found = host
                .carried_stack(OFFHAND_SLOT)
                .filter(|held| held.item == stack.item)
                .map(|_| OFFHAND_SLOT);
        }
        if found.is_none() {
            found = carried.as_ref().and_then(|stacks| {
                stacks
                    .iter()
                    .position(|held| {
                        held.as_ref().is_some_and(|held| held.item == stack.item)
                    })
                    .map(|slot| slot as i32)
            });
        }
        let Some(slot) = found else {
            return Flow::Continue;
        };
        let contents = contents_of(stack, slots);
        let Some(window) = host.open_menu(
            uuid,
            (slots / 9) as u8,
            Text::literal("Backpack"),
            contents.clone(),
        ) else {
            return Flow::Continue;
        };
        state().sessions.insert(
            uuid,
            Session {
                window,
                slot,
                item: stack.item.to_string(),
                slots,
                contents,
                actions: Vec::new(),
            },
        );
        // The item's own use (equipping it, a chestplate's pose) doesn't run.
        Flow::Cancel
    }

    /// Opens the backpack worn in the chest armour slot, for the player
    /// `uuid` (any player on the server, from a key on their client).
    ///
    /// The slot itself is read and written through a living handle, because
    /// a received message has no acting player swapped in; the menu's clicks
    /// then go through `flush_actions` on that slot's inventory tick, as a
    /// backpack opened from the hand does.
    fn open_worn(&self, host: &mut dyn Host, uuid: u128) -> bool {
        let Some(player) = host
            .server_players()
            .into_iter()
            .find(|player| player.uuid == uuid)
        else {
            return false;
        };
        let Ok(entity_id) = u32::try_from(player.entity_id) else {
            return false;
        };
        let Some(entity) = host.entity(entity_id) else {
            return false;
        };
        let Some(living) = host.living(entity) else {
            return false;
        };
        let Some(backpack) = living.equipment(host, EquipmentSlot::Chest) else {
            return false;
        };
        let Some(slots) = item_slots(&backpack.item) else {
            return false;
        };
        let contents = contents_of(&backpack, slots);
        let Some(window) = host.open_menu(
            uuid,
            (slots / 9) as u8,
            Text::literal("Backpack"),
            contents.clone(),
        ) else {
            return false;
        };
        state().sessions.insert(
            uuid,
            Session {
                window,
                slot: CHEST_SLOT,
                item: backpack.item.to_string(),
                slots,
                contents,
                actions: Vec::new(),
            },
        );
        true
    }

    /// Runs the clicks queued for an open backpack, then shows the result.
    fn flush_actions(&self, host: &mut dyn Host, uuid: u128, ticked: i32) {
        let Some((session, actions)) = ({
            let mut st = state();
            st.sessions.get_mut(&uuid).and_then(|session| {
                if session.slot != ticked || session.actions.is_empty() {
                    return None;
                }
                let actions = std::mem::take(&mut session.actions);
                Some((session.clone(), actions))
            })
        }) else {
            return;
        };
        let Some(mut backpack) = read_slot(host, session.slot) else {
            return;
        };
        if backpack.item.as_ref() != session.item {
            return;
        }
        let mut contents = contents_of(&backpack, session.slots);
        // Two stacks of the same kind in the inventory are interchangeable, so
        // a batch of clicks made before this tick is safe to apply as it is.
        for action in actions {
            match action {
                Action::In {
                    player_slot,
                    amount,
                } => {
                    let Some(source) = read_slot(host, player_slot as i32) else {
                        continue;
                    };
                    let max = host.max_stack_size(&source.item).unwrap_or(64);
                    let want = if amount == 0 {
                        source.count
                    } else {
                        amount.min(source.count)
                    };
                    let moved = insert_into(&mut contents, &source, want, max);
                    if moved > 0 {
                        // The backpack takes the items first; the player's slot is
                        // only shortened once that write landed.
                        backpack = with_contents(&backpack, &contents);
                        let Some(held) = host.carried_stack(session.slot) else {
                            continue;
                        };
                        *held = backpack.clone();
                        let mut left = source;
                        left.count -= moved;
                        write_slot(
                            host,
                            player_slot as i32,
                            (left.count > 0).then_some(left),
                        );
                    }
                }
                Action::Out { index, amount } => {
                    let Some(held) = contents.get_mut(index).and_then(Option::take) else {
                        continue;
                    };
                    let take = if amount == 0 {
                        held.count
                    } else {
                        amount.min(held.count)
                    };
                    if take == 0 {
                        contents[index] = Some(held);
                        continue;
                    }
                    let mut given = held.clone();
                    given.count = take;
                    if take < held.count {
                        let mut rest = held;
                        rest.count -= take;
                        contents[index] = Some(rest);
                    }
                    // The backpack keeps the change before the player gets it; an
                    // item that doesn't fit there goes back in, never lost.
                    let written = with_contents(&backpack, &contents);
                    let Some(held) = host.carried_stack(session.slot) else {
                        continue;
                    };
                    *held = written;
                    if let Some(leftover) = host.give_stack(uuid, given) {
                        let max = host.max_stack_size(&leftover.item).unwrap_or(64);
                        insert_into(&mut contents, &leftover, leftover.count, max);
                        backpack = with_contents(&backpack, &contents);
                        if let Some(held) = host.carried_stack(session.slot) {
                            *held = backpack.clone();
                        }
                    }
                }
            }
        }
        backpack = with_contents(&backpack, &contents);
        write_slot(host, session.slot, Some(backpack));
        if let Some(session) = state().sessions.get_mut(&uuid)
            && session.slot == ticked
        {
            session.contents = contents.clone();
        }
        host.set_menu_items(uuid, contents);
    }

    /// Pickup, compacting and restock for the backpack in `slot`.
    fn upgrades(&self, host: &mut dyn Host, uuid: u128, slot: i32) {
        let Some(mut backpack) = read_slot(host, slot) else {
            return;
        };
        if item_slots(&backpack.item).is_none() {
            return;
        }
        let size = item_slots(&backpack.item).unwrap_or(0);
        let mut contents = contents_of(&backpack, size);
        let mut changed = false;

        // Pickup: pull drops around the player into the backpack.
        if has_upgrade(&contents, "pickup_upgrade") {
            let ready = {
                let mut st = state();
                let wait = st.pickup_wait.entry(uuid).or_insert(0);
                if *wait == 0 {
                    *wait = 10;
                    true
                } else {
                    *wait -= 1;
                    false
                }
            };
            if ready && let Some(player) = host.acting_player() {
                let dimension = player.dimension.clone();
                let [x, y, z] = player.position;
                let drops = host.item_entities(
                    &dimension,
                    [x - 4.0, y - 2.0, z - 4.0],
                    [x + 4.0, y + 2.0, z + 4.0],
                );
                for drop in drops {
                    if drop.pickup_delay > 0 || drop.stack.count == 0 {
                        continue;
                    }
                    let max = host.max_stack_size(&drop.stack.item).unwrap_or(64);
                    if insert_into(&mut contents, &drop.stack, drop.stack.count, max)
                        == drop.stack.count
                    {
                        host.remove_entity(&dimension, drop.id);
                        changed = true;
                    }
                }
            }
        }

        // Compacting: nine of a material become its block.
        if has_upgrade(&contents, "compacting_upgrade") {
            let ready = {
                let mut st = state();
                let wait = st.compact_wait.entry(uuid).or_insert(0);
                if *wait == 0 {
                    *wait = 40;
                    true
                } else {
                    *wait -= 1;
                    false
                }
            };
            if ready {
                changed |= compact(&mut contents);
            }
        }

        if changed {
            backpack = with_contents(&backpack, &contents);
            write_slot(host, slot, Some(backpack));
            if let Some(session) = state().sessions.get_mut(&uuid)
                && session.slot == slot
            {
                session.contents = contents.clone();
                host.set_menu_items(uuid, contents);
            }
        }
    }
}

/// Nine of a compactable item become one block, for every pair it can.
fn compact(contents: &mut [Option<ItemStack>]) -> bool {
    const PAIRS: [(&str, &str); 10] = [
        ("minecraft:iron_ingot", "minecraft:iron_block"),
        ("minecraft:gold_ingot", "minecraft:gold_block"),
        ("minecraft:copper_ingot", "minecraft:copper_block"),
        ("minecraft:diamond", "minecraft:diamond_block"),
        ("minecraft:emerald", "minecraft:emerald_block"),
        ("minecraft:lapis_lazuli", "minecraft:lapis_block"),
        ("minecraft:redstone", "minecraft:redstone_block"),
        ("minecraft:coal", "minecraft:coal_block"),
        ("minecraft:netherite_ingot", "minecraft:netherite_block"),
        ("minecraft:wheat", "minecraft:hay_block"),
    ];
    let mut changed = false;
    for (from, to) in PAIRS {
        loop {
            let total: u32 = contents
                .iter()
                .flatten()
                .filter(|stack| &*stack.item == from)
                .map(|stack| stack.count)
                .sum();
            if total < 9 {
                break;
            }
            let mut left = 9;
            for slot in contents.iter_mut() {
                if left == 0 {
                    break;
                }
                let Some(held) = slot.as_mut() else { continue };
                if &*held.item != from {
                    continue;
                }
                let take = held.count.min(left);
                held.count -= take;
                left -= take;
                if held.count == 0 {
                    *slot = None;
                }
            }
            let block = ItemStack::new(to, 1);
            if insert_into(contents, &block, 1, 64) == 0 {
                // No room at all: put the nine back rather than lose them.
                insert_into(contents, &ItemStack::new(from, 9), 9, 64);
                break;
            }
            changed = true;
        }
    }
    changed
}

/// Tops the hotbar stack in `target_slot` up from every carried backpack that
/// has a restock upgrade, one item at a time, never past its stack size.
fn restock_slot(host: &mut dyn Host, uuid: u128, target_slot: i32) {
    let Some(target) = read_slot(host, target_slot) else {
        return;
    };
    if item_slots(&target.item).is_some() {
        return;
    }
    let max = host.max_stack_size(&target.item).unwrap_or(64);
    if target.count >= max {
        return;
    }
    let ready = {
        let mut st = state();
        let wait = st.restock_wait.entry(uuid).or_insert(0);
        if *wait == 0 {
            *wait = 5;
            true
        } else {
            *wait -= 1;
            false
        }
    };
    if !ready {
        return;
    }
    let mut topped = target.clone();
    for slot in BACKPACK_SLOTS {
        if slot == target_slot || topped.count >= max {
            continue;
        }
        let Some(backpack) = read_slot(host, slot) else {
            continue;
        };
        let Some(size) = item_slots(&backpack.item) else {
            continue;
        };
        let mut contents = contents_of(&backpack, size);
        if !has_upgrade(&contents, "restock_upgrade") {
            continue;
        }
        let moved = take_from(&mut contents, &topped, max - topped.count);
        if moved == 0 {
            continue;
        }
        // Backpack first, then the hotbar stack: a failed write loses nothing.
        write_slot(host, slot, Some(with_contents(&backpack, &contents)));
        topped.count += moved;
        write_slot(host, target_slot, Some(topped.clone()));
        if let Some(session) = state().sessions.get_mut(&uuid)
            && session.slot == slot
        {
            session.contents = contents.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, count: u32) -> ItemStack {
        ItemStack::new(item, count)
    }

    #[test]
    fn tiers_and_blocks_know_their_slots() {
        assert_eq!(item_slots("sophisticated_backpacks:leather_backpack"), Some(9));
        assert_eq!(item_slots("sophisticated_backpacks:netherite_backpack"), Some(54));
        assert_eq!(item_slots("minecraft:diamond"), None);
        assert_eq!(
            block_info("sophisticated_backpacks:diamond_backpack_block"),
            Some(("diamond_backpack", 36))
        );
        assert_eq!(block_info("minecraft:barrel"), None);
    }

    #[test]
    fn menus_have_exactly_the_slots_of_their_rows() {
        for (_, slots) in TIERS {
            let menu = menu_type(slots);
            match slots / 9 {
                1 => assert_eq!(menu, "minecraft:generic_9x1"),
                2 => assert_eq!(menu, "minecraft:generic_9x2"),
                3 => assert_eq!(menu, "minecraft:generic_9x3"),
                4 => assert_eq!(menu, "minecraft:generic_9x4"),
                6 => assert_eq!(menu, "minecraft:generic_9x6"),
                rows => panic!("no menu for {rows} rows"),
            }
        }
    }

    #[test]
    fn contents_round_trip_through_the_item() {
        let backpack = stack("sophisticated_backpacks:iron_backpack", 1);
        let mut contents = contents_of(&backpack, 18);
        contents[0] = Some(stack("minecraft:stone", 64));
        contents[17] = Some(stack("minecraft:diamond", 3));
        let written = with_contents(&backpack, &contents);
        assert_eq!(written.count, 1);
        assert_eq!(contents_of(&written, 18), contents);
    }

    #[test]
    fn insert_tops_up_then_fills_empty_slots() {
        let item = stack("minecraft:stone", 100);
        let mut contents = vec![None; 3];
        contents[0] = Some(stack("minecraft:stone", 60));
        let moved = insert_into(&mut contents, &item, item.count, 64);
        assert_eq!(moved, 100);
        assert_eq!(contents[0].as_ref().unwrap().count, 64);
        assert_eq!(contents[1].as_ref().unwrap().count, 64);
        assert_eq!(contents[2].as_ref().unwrap().count, 32);
    }

    #[test]
    fn insert_never_mixes_two_items() {
        let mut contents = vec![Some(stack("minecraft:dirt", 60)), None];
        let moved = insert_into(&mut contents, &stack("minecraft:stone", 10), 10, 64);
        assert_eq!(moved, 10);
        assert_eq!(contents[0].as_ref().unwrap().item.as_ref(), "minecraft:dirt");
        assert_eq!(contents[1].as_ref().unwrap().item.as_ref(), "minecraft:stone");
    }

    #[test]
    fn take_stops_at_what_is_there() {
        let mut contents = vec![Some(stack("minecraft:stone", 5)), None];
        let took = take_from(&mut contents, &stack("minecraft:stone", 1), 12);
        assert_eq!(took, 5);
        assert!(contents[0].is_none());
    }

    #[test]
    fn compacting_turns_nine_into_one_block() {
        let mut contents = vec![None; 4];
        contents[0] = Some(stack("minecraft:iron_ingot", 64));
        contents[1] = Some(stack("minecraft:iron_ingot", 8));
        assert!(compact(&mut contents));
        let ingots: u32 = contents
            .iter()
            .flatten()
            .filter(|s| &*s.item == "minecraft:iron_ingot")
            .map(|s| s.count)
            .sum();
        let blocks: u32 = contents
            .iter()
            .flatten()
            .filter(|s| &*s.item == "minecraft:iron_block")
            .map(|s| s.count)
            .sum();
        assert_eq!((ingots, blocks), (0, 8));
    }

    #[test]
    fn compacting_needs_nine() {
        let mut contents = vec![Some(stack("minecraft:diamond", 8))];
        assert!(!compact(&mut contents));
    }

    #[test]
    fn menu_slots_map_to_the_player_inventory() {
        assert_eq!(player_slot(27, 27), Some(9));
        assert_eq!(player_slot(27, 53), Some(35));
        assert_eq!(player_slot(27, 54), Some(0));
        assert_eq!(player_slot(27, 62), Some(8));
        assert_eq!(player_slot(27, 63), None);
        assert_eq!(player_slot(27, 26), None);
    }
}

impl Hooks for Backpacks {
    /// Each backpack item's slots. API 370 (`Capabilities.Item.ITEM`).
    fn item_container(
        &self,
        _host: &dyn Host,
        event: &mut ItemContainerEvent<'_>,
    ) -> Option<u32> {
        item_slots(event.item).map(|slots| slots as u32)
    }

    /// The placed form's block entity. A mod block has none unless its block
    /// declares one, and its container is reached through it.
    fn block_entity(
        &self,
        _host: &mut dyn Host,
        event: &mut BlockEntityEvent<'_>,
    ) -> Option<(String, Compound)> {
        block_info(event.block)
            .map(|_| (event.block.to_string(), Compound::default()))
    }

    /// The placed form: same slots, shown by a vanilla chest screen.
    fn block_entity_container(
        &self,
        _host: &dyn Host,
        event: &mut BlockEntityContainerEvent<'_>,
    ) -> Option<ModContainer> {
        let (_, slots) = block_info(event.block)?;
        Some(ModContainer {
            slots,
            faces: std::array::from_fn(|_| (0..slots).collect()),
            inputs: (0..slots).collect(),
            outputs: (0..slots).collect(),
            menu: Some(menu_type(slots).to_string()),
        })
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if block_info(event.block).is_some() {
            event.ticks = true;
        }
    }

    /// Right-clicking a backpack in the hand opens it.
    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        self.open(host, event.stack)
    }

    /// Options > Mods lists the mod and its key.
    fn mods(&self, _host: &mut dyn Host, event: &mut ModsEvent<'_>) {
        event.mods.push(NativeMod {
            id: MOD.to_owned(),
            title: "Sophisticated Backpacks".to_owned(),
            description: "Wearable backpacks with tiers and upgrades. Right-click one to open it, \
                          or press the backpack key to open the one you are wearing."
                .to_owned(),
            keys: vec![[
                KEY_NAME.to_owned(),
                "Open worn backpack".to_owned(),
                KEY_DEFAULT.to_owned(),
            ]],
        });
    }

    /// The backpack key: ask the server to open the worn backpack. The server
    /// owns the contents, so this only sends the request.
    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        if !event.pressed || !host.playing() {
            return;
        }
        if event.key == host.mod_key(MOD, KEY_NAME, KEY_DEFAULT) {
            let _ = host.net_send_to_server(CHANNEL_OPEN, b"open");
        }
    }

    /// A player asked for their worn backpack.
    fn net_from_client(&self, host: &mut dyn Host, event: &mut NetFromClientEvent<'_>) {
        if event.channel != CHANNEL_OPEN {
            return;
        }
        if !self.open_worn(host, event.player) {
            host.action_bar(event.player, Text::literal("No backpack equipped"));
        }
    }

    fn menu_clicked(&self, host: &mut dyn Host, event: &mut MenuClickedEvent) {
        let uuid = event.uuid;
        let (mode, button, slot) = (event.mode, event.button, event.slot);
        let Some((menu_slots, window)) = state()
            .sessions
            .get(&uuid)
            .map(|session| (session.slots, session.window))
        else {
            return;
        };
        if event.window != window {
            return;
        }
        // 0: pickup (button 0 whole, 1 half), 1: shift-click. Other modes
        // (drag, number keys, throwing) are not handled yet.
        let amount = match (mode, button) {
            (0, 0) | (1, _) => 0,
            (0, 1) => 1,
            _ => return,
        };
        if slot >= 0 && (slot as usize) < menu_slots {
            let index = slot as usize;
            let count = state()
                .sessions
                .get(&uuid)
                .and_then(|session| session.contents.get(index))
                .and_then(Option::as_ref)
                .map_or(0, |stack| stack.count);
            if count == 0 {
                return;
            }
            let amount = if amount == 1 { count.div_ceil(2) } else { 0 };
            if let Some(session) = state().sessions.get_mut(&uuid) {
                session.actions.push(Action::Out { index, amount });
            }
            return;
        }
        let Some(player_slot) = player_slot(menu_slots, slot) else {
            return;
        };
        let count = host
            .player_stacks(uuid)
            .and_then(|stacks| stacks.get(player_slot).cloned().flatten())
            .map_or(0, |stack| stack.count);
        if count == 0 {
            return;
        }
        let amount = if amount == 1 { count.div_ceil(2) } else { 0 };
        if let Some(session) = state().sessions.get_mut(&uuid) {
            session.actions.push(Action::In {
                player_slot,
                amount,
            });
        }
    }

    fn menu_closed(&self, _host: &mut dyn Host, event: &mut MenuClosedEvent) {
        state().sessions.remove(&event.uuid);
    }

    fn inventory_tick(&self, host: &mut dyn Host, event: &mut InventoryTickEvent) {
        let Some(player) = host.acting_player() else {
            return;
        };
        let uuid = player.uuid;
        let slot = event.slot;
        if event.selected {
            state().selected.insert(uuid, slot);
        }
        self.flush_actions(host, uuid, slot);
        self.upgrades(host, uuid, slot);
        if hotbar(slot) {
            restock_slot(host, uuid, slot);
        }
    }

    /// A placed backpack takes the contents of the item it came from.
    fn place_block(&self, host: &mut dyn Host, event: &mut PlaceBlockEvent<'_>) {
        let block = host.block(event.pos);
        let Some((_, slots)) = block_info(&block) else {
            return;
        };
        let contents = contents_of(event.stack, slots);
        if contents.iter().any(Option::is_some) {
            state().pending.insert((host.dimension(), event.pos), contents);
        }
    }

    fn block_entity_tick(&self, host: &mut dyn Host, event: &mut BlockEntityTickEvent<'_>) {
        let Some(contents) = state().pending.remove(&(host.dimension(), event.pos)) else {
            return;
        };
        if event.entity.items.len() < contents.len() {
            event.entity.items.resize(contents.len(), None);
        }
        for (slot, item) in contents.into_iter().enumerate() {
            event.entity.items[slot] = item;
        }
    }
}
