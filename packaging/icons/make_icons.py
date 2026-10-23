#!/usr/bin/env python3
"""Generates MusicCraft's app icon sizes from musiccraft-source.png.

The source is the owner's artwork (howling wolf on purple, 512x512, RGBA).
Writes next to this script:

- musiccraft-1024.png: macOS .icns input (build-dmg.sh); upscaled from the source.
- musiccraft-512.png, musiccraft-256.png, musiccraft-128.png: Linux hicolor icons
  (Flatpak, portable tarball); the 256 px one is also the window icon embedded
  in the app binary.
- musiccraft.ico: Windows installer and shortcut icon (16 to 256 px).

Requires Pillow. To change the icon, replace musiccraft-source.png (512 px or
larger, square) and run this script again.
"""
from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parent
SOURCE = OUT / "musiccraft-source.png"


def sized(icon: Image.Image, size: int) -> Image.Image:
    return icon.resize((size, size), Image.LANCZOS)


def main() -> None:
    icon = Image.open(SOURCE).convert("RGBA")
    sized(icon, 1024).save(OUT / "musiccraft-1024.png", optimize=True)
    for size in (512, 256, 128):
        sized(icon, size).save(OUT / f"musiccraft-{size}.png", optimize=True)
    icon.save(OUT / "musiccraft.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    print("wrote icons to", OUT)


if __name__ == "__main__":
    main()
