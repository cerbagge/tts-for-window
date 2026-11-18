import sys
from PIL import Image, ImageDraw

out_dir = sys.argv[1]
S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# Rounded-square background with vertical gradient (indigo -> violet)
grad = Image.new("RGBA", (S, S))
top, bot = (79, 70, 229), (147, 51, 234)
gd = ImageDraw.Draw(grad)
for y in range(S):
    t = y / (S - 1)
    gd.line([(0, y), (S, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bot)) + (255,))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([40, 40, S - 40, S - 40], radius=220, fill=255)
img.paste(grad, (0, 0), mask)

d = ImageDraw.Draw(img)
white = (255, 255, 255, 255)
# Speaker body + cone
d.rounded_rectangle([210, 400, 360, 624], radius=28, fill=white)
d.polygon([(330, 400), (560, 230), (560, 794), (330, 624)], fill=white)
# Sound waves
w = 56
for r in (150, 270):
    cx, cy = 560, 512
    d.arc([cx - r, cy - r, cx + r, cy + r], start=-50, end=50, fill=white, width=w)
    # round caps
    import math
    for ang in (-50, 50):
        a = math.radians(ang)
        rc = r - w / 2  # PIL draws arc width inward from the bounding box
        x, y = cx + rc * math.cos(a), cy + rc * math.sin(a)
        d.ellipse([x - w / 2 + 1, y - w / 2 + 1, x + w / 2 - 1, y + w / 2 - 1], fill=white)

sizes = [16, 24, 32, 48, 64, 128, 256]
img.resize((256, 256), Image.LANCZOS).save(f"{out_dir}/icon.png")
img.save(f"{out_dir}/icon.ico", sizes=[(s, s) for s in sizes])
print("ok")
