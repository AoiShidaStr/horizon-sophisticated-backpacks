#!/usr/bin/env python3
"""Generate the mod's original backpack textures (16x16 pixel art).

All art here is drawn from scratch for this clean-room port; no texture from
the original mod is used.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ITEM = ROOT / "assets/sophisticated_backpacks/textures/item"
BLOCK = ROOT / "assets/sophisticated_backpacks/textures/block"

TIERS = {
    "leather": ((146, 92, 51), (184, 126, 79), (92, 56, 28), (222, 184, 133)),
    "iron": ((176, 176, 176), (208, 208, 208), (104, 104, 104), (238, 238, 238)),
    "gold": ((214, 168, 42), (244, 206, 88), (138, 100, 18), (255, 236, 150)),
    "diamond": ((74, 190, 176), (116, 226, 210), (38, 118, 110), (190, 255, 244)),
    "netherite": ((74, 62, 62), (108, 92, 92), (40, 32, 32), (156, 136, 136)),
}


def shade(color, amount):
    return tuple(max(0, min(255, c + amount)) for c in color)


def draw_item(base, light, dark, accent):
    """A backpack seen from the front: body, flap, buckle, straps."""
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()

    def rect(x0, y0, x1, y1, color):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if 0 <= x < 16 and 0 <= y < 16:
                    px[x, y] = (*color, 255)

    # Shoulder straps peeking out at the top.
    rect(3, 1, 4, 4, dark)
    rect(11, 1, 12, 4, dark)
    # Main body.
    rect(2, 4, 13, 14, base)
    # Left/right shading.
    rect(2, 4, 11, 14, base)
    rect(12, 4, 13, 14, dark)
    rect(2, 13, 13, 14, dark)
    # Top flap.
    rect(2, 4, 13, 7, light)
    rect(2, 7, 13, 7, dark)
    # Buckle and strap.
    rect(7, 7, 8, 10, accent)
    rect(7, 11, 8, 11, dark)
    # Pocket.
    rect(4, 9, 11, 12, light)
    rect(4, 12, 11, 12, dark)
    # Highlights.
    rect(3, 5, 3, 6, shade(light, 24))
    rect(4, 10, 4, 11, shade(light, 18))
    # Corners rounded.
    for x, y in ((2, 4), (13, 4), (2, 14), (13, 14)):
        px[x, y] = (0, 0, 0, 0)
    return img


def draw_block(base, light, dark, accent):
    """The placed form: a stitched leather crate with a buckle."""
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()

    def rect(x0, y0, x1, y1, color):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                px[x, y] = (*color, 255)

    rect(0, 0, 15, 15, base)
    # Edge frame.
    rect(0, 0, 15, 1, dark)
    rect(0, 14, 15, 15, dark)
    rect(0, 0, 1, 15, dark)
    rect(14, 0, 15, 15, dark)
    # Lid seam and stitched flap.
    rect(2, 3, 13, 5, light)
    rect(2, 6, 13, 6, dark)
    # Stitches.
    for x in range(3, 13, 2):
        px[x, 2] = (*accent, 255)
        px[x, 7] = (*accent, 255)
    # Central buckle.
    rect(7, 8, 8, 11, accent)
    rect(7, 11, 8, 11, dark)
    # Grain: a few lighter pixels, deterministic.
    for x, y in ((3, 9), (6, 12), (12, 9), (4, 13), (11, 13), (9, 3)):
        px[x, y] = (*light, 255)
    return img


def draw_upgrade(kind):
    """A small module card: a frame coloured by the upgrade's job."""
    colors = {
        "pickup": ((90, 170, 90), (140, 220, 140), (40, 90, 40)),
        "restock": ((90, 120, 200), (150, 180, 240), (40, 60, 110)),
        "compacting": ((190, 140, 60), (240, 200, 120), (110, 70, 20)),
    }
    base, light, dark = colors[kind]
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()

    def rect(x0, y0, x1, y1, color):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                px[x, y] = (*color, 255)

    rect(3, 2, 12, 13, dark)
    rect(4, 3, 11, 12, base)
    rect(4, 3, 11, 4, light)
    # A socket pattern, different per kind.
    if kind == "pickup":
        rect(6, 7, 9, 8, light)
        rect(7, 6, 8, 9, light)
    elif kind == "restock":
        rect(6, 6, 9, 6, light)
        rect(8, 6, 8, 10, light)
        rect(6, 10, 9, 10, light)
    else:
        rect(6, 6, 6, 9, light)
        rect(9, 6, 9, 9, light)
        rect(6, 9, 9, 9, light)
    # Contact pins.
    for x in (5, 7, 9, 11):
        px[x, 14] = (*dark, 255)
    return img


def main():
    ITEM.mkdir(parents=True, exist_ok=True)
    BLOCK.mkdir(parents=True, exist_ok=True)
    for tier, (base, light, dark, accent) in TIERS.items():
        draw_item(base, light, dark, accent).save(ITEM / f"{tier}_backpack.png")
        draw_block(base, light, dark, accent).save(BLOCK / f"{tier}_backpack.png")
    for kind in ("pickup", "restock", "compacting"):
        draw_upgrade(kind).save(ITEM / f"{kind}_upgrade.png")
    print("wrote textures")


if __name__ == "__main__":
    main()
