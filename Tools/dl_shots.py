#!/usr/bin/env python3
"""Composes the per-platform pictures on the download page: site/assets/shots/dl-<platform>.webp, 2400x1800 (2x of
the 1200x900 box, and large enough for the lightbox). Each platform's window sits on its own OS wallpaper; the
phones sit in a device frame (iPhone: Tools/frame_phone.py style; Android: a Pixel-style frame drawn here).

  dl_shots.py desktop <screenshot> <l,t,r,b> <wallpaper> <out> [window corner radius] [scale]
  dl_shots.py iphone <framed.png> <out>        (framed by ~/.tools/frame_phone.py)
  dl_shots.py android <raw_screen.png> <out>
"""
import sys
from PIL import Image, ImageDraw, ImageFilter
W, H = 2400, 1800

def cover(wp_path):
    """The wallpaper, cropped to fill the picture."""
    wp = Image.open(wp_path).convert("RGB")
    s = max(W / wp.width, H / wp.height)
    wp = wp.resize((int(wp.width * s) + 1, int(wp.height * s) + 1), Image.LANCZOS)
    x, y = (wp.width - W) // 2, (wp.height - H) // 2
    return wp.crop((x, y, x + W, y + H)).convert("RGBA")

def gradient(colors):
    """A soft wallpaper-like gradient from a few colour blobs (for the phones)."""
    bg = Image.new("RGB", (W // 4, H // 4), colors[0]); d = ImageDraw.Draw(bg)
    for (c, (x, y, r)) in colors[1:]: d.ellipse([x - r, y - r, x + r, y + r], fill=c)
    return bg.filter(ImageFilter.GaussianBlur(70)).resize((W, H), Image.LANCZOS).convert("RGBA")

def rounded_mask(size, radius):
    m = Image.new("L", size, 0); ImageDraw.Draw(m).rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255); return m

def drop(bg, pic, pos, alpha_mask=None, blur=50, offset=36, alpha=110):
    """Pastes `pic` with a soft shadow. alpha_mask: the picture's shape (defaults to its alpha)."""
    mask = alpha_mask or pic.split()[3]
    pad = blur * 3
    sh = Image.new("RGBA", (pic.width + pad * 2, pic.height + pad * 2), (0, 0, 0, 0))
    sh.paste((10, 10, 30, alpha), (pad, pad + offset), mask)
    sh = sh.filter(ImageFilter.GaussianBlur(blur))
    bg.alpha_composite(sh, (pos[0] - pad, pos[1] - pad))
    bg.alpha_composite(pic, pos)

def desktop(src, box, wallpaper, out, radius=14, scale=1.0):
    win = Image.open(src).convert("RGBA").crop(box)
    if scale != 1.0: win = win.resize((int(win.width * scale), int(win.height * scale)), Image.LANCZOS)
    maxw, maxh = int(W * 0.8), int(H * 0.84)
    s = min(1.0, maxw / win.width, maxh / win.height)
    if s < 1.0: win = win.resize((int(win.width * s), int(win.height * s)), Image.LANCZOS)
    mask = rounded_mask(win.size, int(radius * 2 * scale * s))
    pic = Image.new("RGBA", win.size, (0, 0, 0, 0)); pic.paste(win, (0, 0), mask)
    bg = cover(wallpaper)
    drop(bg, pic, ((W - win.width) // 2, (H - win.height) // 2), mask)
    bg.convert("RGB").save(out, quality=88, method=6); print(out, win.size)

def place_phone(bg, ph, out):
    target_h = int(H * 1.0)                      # the phone fills the height; its bottom bezel runs off
    s = target_h / ph.height
    ph = ph.resize((int(ph.width * s), target_h), Image.LANCZOS)
    drop(bg, ph, ((W - ph.width) // 2, int(H * 0.05)), blur=60, offset=40, alpha=120)
    bg.convert("RGB").save(out, quality=88, method=6); print(out, ph.size)

def iphone(framed, out):
    bg = gradient([(28, 38, 96), ((78, 96, 232), (120, 110, 90)), ((214, 120, 230), (470, 330, 150)), ((60, 190, 240), (300, 420, 160))])
    place_phone(bg, Image.open(framed).convert("RGBA"), out)

def android_frame(raw, target_h=3200):
    """A Pixel-style frame: even bezel, rounded corners, a centred punch-hole camera."""
    sc = Image.open(raw).convert("RGB")
    dh = target_h; dw = int(dh * sc.width / sc.height)
    screen = Image.new("RGBA", (dw, dh), (0, 0, 0, 0)); screen.paste(sc.resize((dw, dh), Image.LANCZOS), (0, 0), rounded_mask((dw, dh), int(dw * 0.11)))
    bez = int(dw * 0.032); rim = max(3, int(dw * 0.006))
    fw, fh = dw + bez * 2, dh + bez * 2
    out = Image.new("RGBA", (fw, fh), (0, 0, 0, 0)); d = ImageDraw.Draw(out)
    R = int(dw * 0.14)
    d.rounded_rectangle([0, 0, fw - 1, fh - 1], radius=R, fill=(74, 76, 82, 255))            # polished rim
    d.rounded_rectangle([rim, rim, fw - 1 - rim, fh - 1 - rim], radius=R - rim, fill=(16, 16, 18, 255))   # black bezel
    out.alpha_composite(screen, (bez, bez))
    cr = int(dw * 0.018); cx, cy = fw // 2, bez + int(dh * 0.02)
    d.ellipse([cx - cr, cy - cr, cx + cr, cy + cr], fill=(10, 10, 12, 255))             # punch-hole camera
    d.ellipse([cx - cr // 2, cy - cr // 2, cx + cr // 2, cy + cr // 2], fill=(28, 30, 44, 255))
    # side buttons
    bw = max(4, int(dw * 0.012)); d.rounded_rectangle([fw - bw, int(fh * 0.22), fw + bw, int(fh * 0.30)], radius=bw, fill=(74, 76, 82, 255))
    d.rounded_rectangle([fw - bw, int(fh * 0.33), fw + bw, int(fh * 0.45)], radius=bw, fill=(74, 76, 82, 255))
    return out

def android(raw, out):
    bg = gradient([(120, 150, 140), ((190, 214, 196), (120, 110, 100)), ((236, 206, 180), (470, 330, 150)), ((150, 180, 210), (300, 420, 160))])
    place_phone(bg, android_frame(raw), out)

def scene(src, box, out):
    """A region of a real desktop screenshot (window + bars + dock), scaled to the picture."""
    im = Image.open(src).convert("RGB").crop(box)
    im = im.resize((W, H), Image.LANCZOS)
    im.save(out, quality=88, method=6); print(out, box)

def macdesk(full_shot, bar_h, dock_box, wallpaper, window, out):
    """macOS: the real menu bar (left and right ends, 2x) and the real dock from a 1x screenshot of the Mac,
    the window (2x) on the wallpaper. dock_box = l,t,r,b of the dock in the screenshot."""
    shot = Image.open(full_shot).convert("RGBA")
    bg = cover(wallpaper)
    half = W // 4   # each end of the bar: 600 px at 1x -> 1200 px at 2x
    left = shot.crop((0, 0, half, bar_h)).resize((half * 2, bar_h * 2), Image.LANCZOS)
    right = shot.crop((shot.width - half, 0, shot.width, bar_h)).resize((half * 2, bar_h * 2), Image.LANCZOS)
    dock = shot.crop(dock_box); k = min(1.6, (W - 40) / dock.width)
    dock = dock.resize((int(dock.width * k), int(dock.height * k)), Image.LANCZOS)
    win = Image.open(window).convert("RGBA")
    maxh = H - bar_h * 2 - dock.height - 140
    s = min(1.0, maxh / win.height); win = win.resize((int(win.width * s), int(win.height * s)), Image.LANCZOS)
    mask = rounded_mask(win.size, int(22 * s)); pic = Image.new("RGBA", win.size, (0, 0, 0, 0)); pic.paste(win, (0, 0), mask)
    drop(bg, pic, ((W - win.width) // 2, bar_h * 2 + (maxh - win.height) // 2 + 60), mask)
    bg.alpha_composite(left, (0, 0)); bg.alpha_composite(right, (W - half * 2, 0))
    bg.alpha_composite(dock, ((W - dock.width) // 2, H - dock.height - 8))
    bg.convert("RGB").save(out, quality=88, method=6); print(out, win.size, dock.size)

if __name__ == "__main__":
    k = sys.argv[1]
    if k == "desktop": desktop(sys.argv[2], tuple(map(int, sys.argv[3].split(","))), sys.argv[4], sys.argv[5], int(sys.argv[6]) if len(sys.argv) > 6 else 14, float(sys.argv[7]) if len(sys.argv) > 7 else 1.0)
    elif k == "iphone": iphone(sys.argv[2], sys.argv[3])
    elif k == "android": android(sys.argv[2], sys.argv[3])
    elif k == "scene": scene(sys.argv[2], tuple(map(int, sys.argv[3].split(","))), sys.argv[4])
    elif k == "macdesk": macdesk(sys.argv[2], int(sys.argv[3]), tuple(map(int, sys.argv[4].split(","))), sys.argv[5], sys.argv[6], sys.argv[7])
