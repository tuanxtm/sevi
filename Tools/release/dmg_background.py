#!/usr/bin/env python3
"""DMG window background: a soft arrow between the app and the Applications link. Writes a 2x PNG
sized to the create-dmg window (660x420 points)."""
import sys
from PIL import Image, ImageDraw
W, H, S = 660, 420, 2
im = Image.new("RGB", (W * S, H * S), (246, 247, 250))
d = ImageDraw.Draw(im)
# arrow from app icon (170,190) to Applications (490,190); icons are 128pt so the arrow sits between them
y = 190 * S; x0, x1 = 262 * S, 398 * S
d.line([(x0, y), (x1, y)], fill=(165, 170, 185), width=6 * S)
d.polygon([(x1 + 4 * S, y), (x1 - 18 * S, y - 16 * S), (x1 - 18 * S, y + 16 * S)], fill=(165, 170, 185))
im.save(sys.argv[1], dpi=(144, 144))
