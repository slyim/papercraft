"""Build the macOS app icon from the square PaperCraft logo.

macOS does not mask application icons, so the artwork must carry its own
rounded-square shape and transparent margin: an 824-pixel continuous-corner
tile centered on a 1024-pixel canvas.

Usage: python3 packaging/macos/make_icon.py
Writes assets/papercraft-macos.png (512-pixel runtime icon) and assets/papercraft.icns.
"""

import math
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
CANVAS = 1024
TILE = 824
# Superellipse exponent approximating Apple's continuous-corner tile.
EXPONENT = 5.0
SUPERSAMPLE = 4


def tile_mask() -> Image.Image:
    size = CANVAS * SUPERSAMPLE
    half = TILE * SUPERSAMPLE / 2
    center = size / 2
    points = []
    steps = 2048
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = center + half * math.copysign(abs(c) ** (2 / EXPONENT), c)
        y = center + half * math.copysign(abs(s) ** (2 / EXPONENT), s)
        points.append((x, y))
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).polygon(points, fill=255)
    return mask.resize((CANVAS, CANVAS), Image.LANCZOS)


def main() -> None:
    logo = Image.open(ROOT / "assets/papercraft-logo.png").convert("RGB")
    art = logo.resize((TILE, TILE), Image.LANCZOS)
    icon = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    offset = (CANVAS - TILE) // 2
    icon.paste(art, (offset, offset))
    icon.putalpha(tile_mask())
    # Runtime Dock icon; 512 pixels covers the largest Retina Dock size.
    icon.resize((512, 512), Image.LANCZOS).save(
        ROOT / "assets/papercraft-macos.png", optimize=True
    )

    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "papercraft.iconset"
        iconset.mkdir()
        for points in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                px = points * scale
                suffix = "" if scale == 1 else "@2x"
                icon.resize((px, px), Image.LANCZOS).save(
                    iconset / f"icon_{points}x{points}{suffix}.png"
                )
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(Path(tmp) / "out.icns")],
            check=True,
        )
        shutil.copy2(Path(tmp) / "out.icns", ROOT / "assets/papercraft.icns")


if __name__ == "__main__":
    main()
