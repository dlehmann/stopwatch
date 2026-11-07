import math, sys
from PIL import Image, ImageDraw

S = 1024                      # draw large, downsample for anti-aliasing
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

ORANGE = (245, 124, 0, 255)
DARK = (38, 42, 48, 255)
FACE = (250, 250, 250, 255)
cx, cy, r = S / 2, S * 0.57, S * 0.38

# crown (top button) and stem
d.rounded_rectangle([cx - 70, cy - r - 150, cx + 70, cy - r - 80], radius=22, fill=DARK)
d.rectangle([cx - 34, cy - r - 90, cx + 34, cy - r + 10], fill=DARK)
# side button at ~1:30
a = math.radians(-45)
bx, by = cx + math.cos(a) * (r + 40), cy + math.sin(a) * (r + 40)
side = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(side).rounded_rectangle([bx - 38, by - 60, bx + 38, by + 60], radius=16, fill=DARK)
img.alpha_composite(side.rotate(-45, center=(bx, by)))
d = ImageDraw.Draw(img)

# case ring and face
d.ellipse([cx - r - 40, cy - r - 40, cx + r + 40, cy + r + 40], fill=ORANGE)
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=FACE)

# ticks
for i in range(12):
    ang = math.radians(i * 30 - 90)
    long = i % 3 == 0
    r1, r2 = r * (0.74 if long else 0.82), r * 0.92
    w = 26 if long else 14
    d.line([cx + math.cos(ang) * r1, cy + math.sin(ang) * r1,
            cx + math.cos(ang) * r2, cy + math.sin(ang) * r2], fill=DARK, width=w)

# elapsed sector (0 -> 2 o'clock)
d.pieslice([cx - r * 0.62, cy - r * 0.62, cx + r * 0.62, cy + r * 0.62], -90, -30, fill=(245, 124, 0, 90))

# hand pointing to ~2 o'clock
ang = math.radians(-30)
d.line([cx, cy, cx + math.cos(ang) * r * 0.78, cy + math.sin(ang) * r * 0.78], fill=ORANGE, width=30)
d.ellipse([cx - 44, cy - 44, cx + 44, cy + 44], fill=DARK)

out = sys.argv[1]
icon = img.resize((256, 256), Image.LANCZOS)
icon.save(f"{out}/icon.png")
icon.save(f"{out}/icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
