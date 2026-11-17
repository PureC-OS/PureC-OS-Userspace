#!/usr/bin/env python3
import struct
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

FONT = "/usr/share/fonts/TTF/JetBrainsMonoNerdFontPropo-Regular.ttf"
SIZE = 96
ICONS = {
    "start": "apps",
    "wifi-1": "wifi_strength_1",
    "wifi-2": "wifi_strength_2",
    "wifi-3": "wifi_strength_3",
    "wifi-4": "wifi_strength_4",
    "wifi-none": "wifi_strength_outline",
    "wifi-off": "wifi_off",
    "wifi-lock": "wifi_lock",
    "ethernet": "ethernet",
    "net-off": "lan_disconnect",
    "power": "power",
    "restart": "restart",
    "logout": "logout",
    "account": "account_circle",
    "refresh": "refresh",
}


def render(font, ch):
    big = Image.new("L", (SIZE * 3, SIZE * 3), 0)
    ImageDraw.Draw(big).text((SIZE, SIZE), ch, font=font, fill=255)
    box = big.getbbox()
    if not box:
        sys.exit("empty glyph")
    crop = big.crop(box)
    limit = SIZE - 8
    if crop.width > limit or crop.height > limit:
        crop.thumbnail((limit, limit), Image.LANCZOS)
    out = Image.new("L", (SIZE, SIZE), 0)
    out.paste(crop, ((SIZE - crop.width) // 2, (SIZE - crop.height) // 2))
    return out.tobytes()


def main():
    root = Path(__file__).resolve().parent.parent / "assets" / "icons"
    cmap = {}
    for cp, name in TTFont(FONT).getBestCmap().items():
        cmap.setdefault(name, cp)
    font = ImageFont.truetype(FONT, SIZE - 8)
    for out, glyph in ICONS.items():
        cp = cmap.get("md-" + glyph)
        if cp is None:
            sys.exit("missing md-" + glyph)
        data = render(font, chr(cp))
        (root / (out + ".icn")).write_bytes(b"PCIC" + struct.pack("<HH", SIZE, SIZE) + data)
        print(out + ".icn")
    return 0


if __name__ == "__main__":
    sys.exit(main())
