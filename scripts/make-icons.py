"""Regenerates the icon files in resources/ from resources/icon.png (1024x1024 RGBA).

    python scripts/make-icons.py        (needs Pillow)

resources/icon.ico   16, 24, 32, 48, 64 and 256 px: the icon of the Windows .exe and .scr (build.rs embeds it)
resources/icon.rgba  64x64 raw RGBA: the window icon of every platform (src/main.rs includes it)
"""

from pathlib import Path

from PIL import Image

res = Path(__file__).resolve().parent.parent / "resources"
master = Image.open(res / "icon.png").convert("RGBA")
assert master.size == (1024, 1024), master.size

master.save(res / "icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 256)])
(res / "icon.rgba").write_bytes(master.resize((64, 64), Image.LANCZOS).tobytes())
