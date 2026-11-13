#!/usr/bin/env python3
"""Write the mod's content JSON (items, blocks, models, recipes, loot, lang).

Clean-room port: every id, name and recipe here is original.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NS = "sophisticated_backpacks"
A = ROOT / "assets" / NS
D = ROOT / "data" / NS

TIERS = [
    ("leather", 9, 1, "Leather"),
    ("iron", 18, 2, "Iron"),
    ("gold", 27, 3, "Gold"),
    ("diamond", 36, 4, "Diamond"),
    ("netherite", 54, 6, "Netherite"),
]
MENU = {1: "minecraft:generic_9x1", 2: "minecraft:generic_9x2", 3: "minecraft:generic_9x3",
        4: "minecraft:generic_9x4", 5: "minecraft:generic_9x5", 6: "minecraft:generic_9x6"}
UPGRADES = ["pickup", "restock", "compacting"]


def write(path: Path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def main():
    # Items: wearable (chestplate template) and placeable as the block beside them.
    for name, slots, rows, _ in TIERS:
        write(D / "horizon/item" / f"{name}_backpack.json", {
            "template": "minecraft:leather_chestplate",
            "place_block": f"{NS}:{name}_backpack_block",
        })
        # Blocks copy a barrel so vanilla opens a container screen for them; the
        # native half gives them their slots and menu.
        write(D / "horizon/block" / f"{name}_backpack_block.json", {
            "template": "minecraft:barrel",
            "item": False,
        })
        write(A / "models/item" / f"{name}_backpack.json", {
            "parent": "minecraft:item/generated",
            "textures": {"layer0": f"{NS}:item/{name}_backpack"},
        })
        write(A / "models/block" / f"{name}_backpack_block.json", {
            "parent": "minecraft:block/cube_all",
            "textures": {"all": f"{NS}:block/{name}_backpack"},
        })
        variants = {}
        for facing in ("down", "east", "north", "south", "up", "west"):
            for opened in ("false", "true"):
                variants[f"facing={facing},open={opened}"] = {
                    "model": f"{NS}:block/{name}_backpack_block"}
        write(A / "blockstates" / f"{name}_backpack_block.json", {"variants": variants})
        write(D / "loot_table/blocks" / f"{name}_backpack_block.json", {
            "type": "minecraft:block",
            "pools": [{
                "rolls": 1,
                "entries": [{"type": "minecraft:item", "name": f"{NS}:{name}_backpack"}],
                "conditions": [{"condition": "minecraft:survives_explosion"}],
            }],
        })

    for kind in UPGRADES:
        write(D / "horizon/item" / f"{kind}_upgrade.json", {
            "template": "minecraft:paper",
            "max_stack_size": 16,
            "rarity": "uncommon",
        })
        write(A / "models/item" / f"{kind}_upgrade.json", {
            "parent": "minecraft:item/generated",
            "textures": {"layer0": f"{NS}:item/{kind}_upgrade"},
        })

    write(D / "horizon/creative_tab/backpacks.json", {
        "title": f"itemGroup.{NS}.backpacks",
        "icon": f"{NS}:leather_backpack",
        "items": [f"{NS}:{n}_backpack" for n, *_ in TIERS]
        + [f"{NS}:{k}_upgrade" for k in UPGRADES],
    })

    def shaped(name, pattern, key, result, count=1, category="misc"):
        write(D / "recipe" / f"{name}.json", {
            "type": "minecraft:crafting_shaped",
            "category": category,
            "pattern": pattern,
            "key": key,
            "result": {"id": result, "count": count},
        })

    shaped("leather_backpack", ["LLL", "LSL", "LLL"],
           {"L": "minecraft:leather", "S": "minecraft:string"},
           f"{NS}:leather_backpack")
    shaped("iron_backpack", ["III", "IBI", "III"],
           {"I": "minecraft:iron_ingot", "B": f"{NS}:leather_backpack"},
           f"{NS}:iron_backpack")
    shaped("gold_backpack", ["GGG", "GBG", "GGG"],
           {"G": "minecraft:gold_ingot", "B": f"{NS}:iron_backpack"},
           f"{NS}:gold_backpack")
    shaped("diamond_backpack", ["DDD", "DBD", "DDD"],
           {"D": "minecraft:diamond", "B": f"{NS}:gold_backpack"},
           f"{NS}:diamond_backpack")
    shaped("netherite_backpack", ["NNN", "NBN", "NNN"],
           {"N": "minecraft:netherite_ingot", "B": f"{NS}:diamond_backpack"},
           f"{NS}:netherite_backpack")
    shaped("pickup_upgrade", ["PPP", "PHP", "PPP"],
           {"P": "minecraft:paper", "H": "minecraft:hopper"},
           f"{NS}:pickup_upgrade")
    shaped("restock_upgrade", ["PPP", "PCP", "PPP"],
           {"P": "minecraft:paper", "C": "minecraft:chest"},
           f"{NS}:restock_upgrade")
    shaped("compacting_upgrade", ["PPP", "PTP", "PPP"],
           {"P": "minecraft:paper", "T": "minecraft:crafting_table"},
           f"{NS}:compacting_upgrade")

    en = {
        "itemGroup.sophisticated_backpacks.backpacks": "Sophisticated Backpacks",
        "item.sophisticated_backpacks.pickup_upgrade": "Pickup Upgrade",
        "item.sophisticated_backpacks.restock_upgrade": "Restock Upgrade",
        "item.sophisticated_backpacks.compacting_upgrade": "Compacting Upgrade",
    }
    fr = {
        "itemGroup.sophisticated_backpacks.backpacks": "Sacs \u00e0 dos sophistiqu\u00e9s",
        "item.sophisticated_backpacks.pickup_upgrade": "Am\u00e9lioration de ramassage",
        "item.sophisticated_backpacks.restock_upgrade": "Am\u00e9lioration de r\u00e9approvisionnement",
        "item.sophisticated_backpacks.compacting_upgrade": "Am\u00e9lioration de compactage",
    }
    names_en = {"leather": "Leather", "iron": "Iron", "gold": "Gold",
                "diamond": "Diamond", "netherite": "Netherite"}
    names_fr = {"leather": "En cuir", "iron": "En fer", "gold": "En or",
                "diamond": "En diamant", "netherite": "En netherite"}
    for tier, *_ in TIERS:
        en[f"item.sophisticated_backpacks.{tier}_backpack"] = f"{names_en[tier]} Backpack"
        fr[f"item.sophisticated_backpacks.{tier}_backpack"] = f"Sac \u00e0 dos {names_fr[tier]}"
    write(A / "lang/en_us.json", en)
    write(A / "lang/fr_fr.json", fr)
    print("wrote content")


if __name__ == "__main__":
    main()
