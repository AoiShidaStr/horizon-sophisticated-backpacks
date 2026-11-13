# Sophisticated Backpacks — Horizon port

A clean-room port of [Sophisticated Backpacks](https://github.com/P3pp3rF1y/SophisticatedBackpacks)
to **Horizon** (the Rust rewrite of Minecraft Java 26.3). It is not a Java-mod
bridge: the content is a content mod (`.hmod`) and the behaviour is a native
trait mod compiled into the game.

One file holds both halves: `dist/sophisticated_backpacks.hmod`.

## Done

### Content (`horizon-mod.toml`)

- Five backpack tiers, **wearable** (chestplate slot) and **placeable**:

  | Item | Slots | Placed block | Menu |
  |---|---|---|---|
  | `leather_backpack` | 9 | `leather_backpack_block` | 1 row |
  | `iron_backpack` | 18 | `iron_backpack_block` | 2 rows |
  | `gold_backpack` | 27 | `gold_backpack_block` | 3 rows |
  | `diamond_backpack` | 36 | `diamond_backpack_block` | 4 rows |
  | `netherite_backpack` | 54 | `netherite_backpack_block` | 6 rows |

- Three upgrades, items the backpack holds: `pickup_upgrade`,
  `restock_upgrade`, `compacting_upgrade`.
- Original textures (drawn by `scripts/gen_textures.py`), blockstates, block
  and item models, English and French translations, a creative tab, crafting
  recipes for every tier and upgrade, and block loot tables.

### Native (`mixins/sophisticated_backpacks/`, `horizon-native.json`)

- Every backpack item is an item container (`item_container`, API 370): its
  contents live in the stack's `minecraft:container`, so they are saved and
  sent with the item. (Backpacks themselves don't stack: the item is a
  `leather_chestplate` template, so its stack size is 1.)
- Right-clicking a backpack in the hand (or off hand) opens an interactive
  menu (`use_item` + `open_menu`/`set_menu_items`): left click moves a whole
  stack, right click half a stack, from and to the player's 36 inventory
  slots. Every click is checked and applied on the server on the next
  inventory tick.
- The **backpack key** (default `B`, rebindable in Options > Mods) opens the
  backpack worn in the chest slot: the client sends a message on
  `sophisticated_backpacks:open` (`key` + `net_send_to_server`) and the server
  opens the same menu on the worn item (`net_from_client`, `server_players`,
  `living`/`equipment`, `open_menu`). Clicks are applied on that slot's
  inventory tick exactly as for a carried backpack.
- The placed form gives the block a mod container with exactly the tier's
  slots and a vanilla chest menu (`block_entity_container`), so hoppers and
  vanilla clients see it as an ordinary storage block.
- A placed backpack takes the contents of the item it was placed from
  (`place_block` + `block_entity_tick`).
- Upgrades work while the backpack is carried (inventory or chest slot):
  - **Pickup** pulls dropped items within 4 blocks into the backpack
    (`item_entities` + `remove_entity`);
  - **Restock** tops up the held hotbar stack from the backpack;
  - **Compacting** turns 9 of a material into its block (ingots, gems,
    redstone, coal, copper, netherite, wheat).

## Missing / differences from the original

- **Menus are chest-style rows**, not a custom backpack screen: no colour
  swatches, no upgrade slots panel, no sort/search buttons. Upgrades are
  simply kept in the backpack's own inventory.
- **The worn model and armour values are the leather chestplate's**: the item
  copies `minecraft:leather_chestplate` for the chest slot, so on a player it
  renders as a chestplate rather than a backpack. Its icon is the backpack
  texture.
- **Breaking the placed block spills its contents** at the block, as a chest
  does, and drops an empty backpack item. The item's contents are moved into
  the placed block, not back into the dropped item.
- No colour customisation, no tanks/tool/feeding/magnet/void upgrades, no
  stack upgrades beyond the five tiers, no filters, no JEI/recipe-viewer
  integration, no upgrading an item in place (tiers are crafted).
- Shift-click, drags, double-click and number-key swaps are not implemented;
  a click moves one stack (or half with the right button).
- Contents can only be changed while the backpack is in a slot that the
  server ticks (the 36 inventory slots, chest armour and off hand), which is
  where upgrades run too.

## Building

The VPS workflow used for this port:

```sh
# content only
cargo horizon-mod build                 # -> target/horizon-mod/sophisticated_backpacks.hmod
cargo horizon-mod check target/horizon-mod/sophisticated_backpacks.hmod

# native bundle staging (see scripts/stage_vps.sh) then:
CARGO_TARGET_DIR=/opt/build/target cargo build -p horizon-native-game --no-default-features
# client flavour
CARGO_TARGET_DIR=/opt/build/target cargo check -p horizon-client

# combined file
python3 scripts/pack.py                 # -> dist/sophisticated_backpacks.hmod
cargo horizon-mod check dist/sophisticated_backpacks.hmod
```

`horizon-native.json` and `horizon-mixin.toml` pin Horizon commit
`a85af177` and horizon-mixins `5a64af93`, the versions this port was built
against. `hooks` is 398, the highest hook point it uses (the worn-backpack key
reads `NativeServerPlayer.entity_id`, API 398).

## Licensing

The original Sophisticated Backpacks is **All Rights Reserved** (P3pp3rF1y).
No code, data, model or texture from it is included or translated here: this
is a clean-room reimplementation from public behaviour descriptions (its
README/CurseForge page and wiki), with original Rust code and textures
generated by `scripts/gen_textures.py`. The mod's name is used only to
describe what the port is. If the original author objects, this port should be
removed.

This port's own code and assets are original work for this private server
project and are not offered under the original mod's licence.
