#!/usr/bin/env python3
"""Render annotated floor plans for every assets/floors/*.floor layout.

Run from the repository root (needs Pillow):
    python tools/floor_plan.py

Writes docs/floors/<id>_plan.png in the game's screen orientation. The plan is
drawn over the baked background assets/floors/<id>.png when it exists and over
a schematic of the layout otherwise.
"""
import math
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FLOORS = os.path.join(ROOT, "assets", "floors")
OUT = os.path.join(ROOT, "docs", "floors")
FONT_DIR = os.path.join(ROOT, "tools", "fonts")

# Projection constants (design px, see the floor contract).
PU, PV, PZ = 44.0, 24.2, 52.0
VIEW_W, VIEW_H = 900.0, 600.0
VIEW_UP, VIEW_DOWN = 308.0, 292.0
TARGET_W = 1500
RADIUS = 0.28

TITLES = {
    "sorting_hall": "Sorting Floor \u2014 public hall",
    "vault_stacks": "Dead Letter Vault \u2014 archive stacks",
}
VIEW_NAMES = {"arrival": ("A", "arrival"), "junction": ("J", "junction"), "return": ("R", "return")}
VIEW_COLORS = {"arrival": (90, 200, 255), "junction": (255, 150, 60), "return": (110, 230, 130)}

PEARL = (233, 228, 210)
BRASS = (230, 196, 58)
VIOLET = (176, 140, 240)
WAX = (210, 48, 31)
INK = (6, 15, 18)
FLOOR_FILL = (38, 52, 56)
KIND_COLORS = {
    "wall": (92, 100, 104),
    "counter": (140, 110, 70),
    "table": (120, 96, 66),
    "rack": (86, 110, 128),
    "stack": (84, 100, 120),
    "railing": (170, 150, 90),
    "landmark": (80, 170, 160),
}


def font(size, mono=False):
    names = ["consola.ttf", "C:/Windows/Fonts/consola.ttf"] if mono else []
    names.append(os.path.join(FONT_DIR, "DejaVuSerif.ttf"))  # has the arrow glyphs
    for n in names:
        try:
            return ImageFont.truetype(n, size)
        except OSError:
            continue
    return ImageFont.load_default()


def parse(path):
    f = {"blocks": [], "doors": {}, "routes": {}, "views": {}}
    with open(path, encoding="utf-8") as fh:
        for raw in fh:
            line = raw.split("#", 1)[0].split()
            if not line:
                continue
            key, a = line[0], line[1:]
            if key == "floor":
                f["id"] = a[0]
            elif key == "image":
                f["image"] = tuple(float(v) for v in a[:4])
            elif key == "bounds":
                f["bounds"] = tuple(float(v) for v in a[:4])
            elif key == "entrance":
                f["entrance"] = tuple(float(v) for v in a[:4])
            elif key == "landmark":
                f["landmark"] = (float(a[0]), float(a[1]))
            elif key == "block":
                f["blocks"].append((a[0],) + tuple(float(v) for v in a[1:6]))
            elif key == "door":
                f["doors"][a[0]] = tuple(float(v) for v in a[1:5])
            elif key == "route":
                v = [float(x) for x in a[1:]]
                f["routes"][a[0]] = list(zip(v[0::2], v[1::2]))
            elif key == "view":
                f["views"][a[0]] = (float(a[1]), float(a[2]))
    return f


def camera_frame(f, view):
    """Top-left corner of the 900x600 frame in baked-image design px (engine clamp)."""
    w, h, ox, oy = f["image"]
    cu = PU * (view[0] + view[1])
    cv = PV * (view[0] - view[1])
    cu = min(max(cu, VIEW_W / 2 - ox), w - ox - VIEW_W / 2)
    cv = min(max(cv, VIEW_UP - oy), h - oy - VIEW_DOWN)
    return ox + cu - VIEW_W / 2, oy + cv - VIEW_UP


class Plan:
    def __init__(self, f):
        self.f = f
        w, h, self.ox, self.oy = f["image"]
        self.s = TARGET_W / w
        self.size = (round(w * self.s), round(h * self.s))

    def pt(self, x, y, z=0.0):
        return ((self.ox + PU * (x + y)) * self.s,
                (self.oy + PV * (x - y) - PZ * z) * self.s)

    def draw_schematic(self, d):
        x0, y0, x1, y1 = self.f["bounds"]
        d.rectangle((0, 0) + self.size, fill=INK)
        d.polygon([self.pt(x0, y0), self.pt(x1, y0), self.pt(x1, y1), self.pt(x0, y1)], fill=FLOOR_FILL)
        for (kind, bx0, by0, bx1, by1, h) in sorted(self.f["blocks"], key=lambda b: (b[1] + b[3]) - (b[2] + b[4])):
            c = KIND_COLORS.get(kind, (120, 120, 120))
            top = tuple(min(255, int(v * 1.25)) for v in c)
            side_x = tuple(int(v * 0.8) for v in c)
            side_y = tuple(int(v * 0.6) for v in c)
            d.polygon([self.pt(bx1, by0, 0), self.pt(bx1, by1, 0), self.pt(bx1, by1, h), self.pt(bx1, by0, h)], fill=side_x)
            d.polygon([self.pt(bx0, by0, 0), self.pt(bx1, by0, 0), self.pt(bx1, by0, h), self.pt(bx0, by0, h)], fill=side_y)
            d.polygon([self.pt(bx0, by0, h), self.pt(bx1, by0, h), self.pt(bx1, by1, h), self.pt(bx0, by1, h)], fill=top)

    def background(self):
        path = os.path.join(FLOORS, self.f["id"] + ".png")
        if os.path.exists(path):
            bg = Image.open(path).convert("RGB").resize(self.size, Image.LANCZOS)
            return Image.eval(bg, lambda v: int(v * 0.62)), True
        img = Image.new("RGB", self.size, INK)
        self.draw_schematic(ImageDraw.Draw(img))
        return img, False


def label(d, xy, text, fnt, fill, anchor="la", pad=3):
    box = d.textbbox(xy, text, font=fnt, anchor=anchor)
    d.rectangle((box[0] - pad, box[1] - pad, box[2] + pad, box[3] + pad), fill=(8, 14, 16, 215))
    d.text(xy, text, font=fnt, fill=fill, anchor=anchor)


def polyline(d, pts, color, style, width):
    """Draw a polyline solid, dashed or dotted by arc length."""
    on, off = {"solid": (1e9, 0), "dash": (16, 9), "dot": (2.5, 8)}[style]
    phase, drawing = 0.0, True
    for a, b in zip(pts, pts[1:]):
        L = math.dist(a, b)
        if L == 0:
            continue
        t = 0.0
        while t < L:
            seg = (on if drawing else off) - phase
            end = min(L, t + seg)
            if drawing:
                p = (a[0] + (b[0] - a[0]) * t / L, a[1] + (b[1] - a[1]) * t / L)
                q = (a[0] + (b[0] - a[0]) * end / L, a[1] + (b[1] - a[1]) * end / L)
                d.line([p, q], fill=color, width=width)
                if style == "dot":
                    r = width * 0.9
                    d.ellipse((q[0] - r, q[1] - r, q[0] + r, q[1] + r), fill=color)
            if t + seg <= L:
                drawing, phase = (not drawing), 0.0
            else:
                phase += end - t
            t = end


def arrow(d, a, b, color, size=11):
    ang = math.atan2(b[1] - a[1], b[0] - a[0])
    m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
    tip = (m[0] + math.cos(ang) * size * 0.6, m[1] + math.sin(ang) * size * 0.6)
    pts = [tip]
    for da in (2.6, -2.6):
        pts.append((tip[0] + math.cos(ang + da) * size, tip[1] + math.sin(ang + da) * size))
    d.polygon(pts, fill=color, outline=INK)


def marker(d, p, color, r=7):
    d.ellipse((p[0] - r - 2, p[1] - r - 2, p[0] + r + 2, p[1] + r + 2), fill=INK)
    d.ellipse((p[0] - r, p[1] - r, p[0] + r, p[1] + r), fill=color)


def render(f):
    plan = Plan(f)
    base, baked = plan.background()
    img = base.convert("RGBA")
    over = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(over, "RGBA")
    pt, s = plan.pt, plan.s
    small, mid, big = font(15), font(18), font(30)
    mono = font(14, True)

    # Block footprints, faint.
    for (kind, x0, y0, x1, y1, h) in f["blocks"]:
        poly = [pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1), pt(x0, y0)]
        d.line(poly, fill=(255, 255, 255, 90), width=1)
    # Walkable bounds.
    x0, y0, x1, y1 = f["bounds"]
    d.line([pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1), pt(x0, y0)], fill=(255, 255, 255, 230), width=3)

    # Camera frames (A/J/R).
    fw, fh = VIEW_W * s, VIEW_H * s
    for name in ("arrival", "junction", "return"):
        if name not in f["views"]:
            continue
        col = VIEW_COLORS[name]
        fx, fy = camera_frame(f, f["views"][name])
        fx, fy = fx * s, fy * s
        d.rectangle((fx, fy, fx + fw, fy + fh), outline=col + (255,), width=3)
        tag, nm = VIEW_NAMES[name]
        label(d, (fx + 8, fy + 8), f"{tag}  {nm} view", mid, col)
        vx, vy = pt(*f["views"][name])
        d.line([(vx - 8, vy), (vx + 8, vy)], fill=col, width=2)
        d.line([(vx, vy - 8), (vx, vy + 8)], fill=col, width=2)

    # Doors.
    for kind, color, text in (("stop", WAX, "STOP \u2192 chamber"), ("side", VIOLET, "SIDE (encounter in #23)")):
        if kind not in f["doors"]:
            continue
        a, b, c, e = f["doors"][kind]
        poly = [pt(a, b), pt(c, b), pt(c, e), pt(a, e)]
        d.polygon(poly, fill=color + (110,), outline=color + (255,))
        d.line(poly + [poly[0]], fill=color, width=3)
        cx = sum(p[0] for p in poly) / 4
        cy = sum(p[1] for p in poly) / 4
        if kind == "stop":
            label(d, (cx, cy + 22), text, mid, (255, 120, 100), anchor="ma")
        else:
            label(d, (cx, cy - 22), text, mid, color, anchor="md")

    # Routes.
    styles = (("main", PEARL, "solid", 5), ("loop", BRASS, "dash", 4), ("side", VIOLET, "dot", 3))
    for name, color, style, width in styles:
        pts = [pt(*p) for p in f["routes"].get(name, [])]
        if len(pts) < 2:
            continue
        polyline(d, pts, color + (255,), style, width)
        for a, b in zip(pts, pts[1:]):
            if math.dist(a, b) > 40:
                arrow(d, a, b, color + (255,))

    # Entrance, landmark, junction, return.
    ex, ey, fx_, fy_ = f["entrance"]
    p = pt(ex, ey)
    marker(d, p, (255, 255, 255), 8)
    tip = pt(ex + fx_ * 1.2, ey + fy_ * 1.2)
    d.line([p, tip], fill=(255, 255, 255, 255), width=3)
    label(d, (p[0], p[1] + 14), "ENTRANCE", mid, (255, 255, 255), anchor="ma")

    lp = pt(*f["landmark"], z=2.0)
    d.polygon([(lp[0], lp[1] - 12), (lp[0] + 12, lp[1]), (lp[0], lp[1] + 12), (lp[0] - 12, lp[1])],
              fill=(255, 215, 90, 255), outline=INK)
    label(d, (lp[0], lp[1] - 18), "LANDMARK", mid, (255, 215, 90), anchor="md")

    loop = f["routes"].get("loop", [])
    if loop:
        jp, rp = pt(*loop[0]), pt(*loop[-1])
        marker(d, jp, VIEW_COLORS["junction"], 9)
        label(d, (jp[0] + 14, jp[1] - 4), "JUNCTION (loop starts)", mid, VIEW_COLORS["junction"], anchor="lm")
        marker(d, rp, VIEW_COLORS["return"], 9)
        label(d, (rp[0] + 14, rp[1] - 4), "RETURN (loop rejoins main)", mid, VIEW_COLORS["return"], anchor="lm")

    img = Image.alpha_composite(img, over)
    d = ImageDraw.Draw(img, "RGBA")

    # Title and legend.
    title = TITLES.get(f["id"], f["id"])
    label(d, (16, 12), title, big, (245, 240, 225), pad=8)
    src = "baked background" if baked else "schematic (no baked background)"
    label(d, (18, 60), f"floor plan over {src}; screen orientation, frames are 900x600 design px",
          small, (190, 200, 200))
    lx, ly = 18, img.size[1] - 215
    d.rectangle((lx - 8, ly - 8, lx + 360, ly + 200), fill=(8, 14, 16, 225), outline=(255, 255, 255, 120))
    rows = [("main route (entrance \u2192 stop door)", PEARL, "solid"),
            ("loop (leaves and rejoins main)", BRASS, "dash"),
            ("side route (to side door)", VIOLET, "dot")]
    for i, (text, col, style) in enumerate(rows):
        y = ly + 14 + i * 26
        polyline(d, [(lx, y), (lx + 60, y)], col, style, 4)
        d.text((lx + 72, y), text, font=small, fill=col, anchor="lm")
    y = ly + 14 + 3 * 26
    d.rectangle((lx, y - 7, lx + 60, y + 7), outline=WAX, width=3)
    d.text((lx + 72, y), "stop door (red) / side door (violet)", font=small, fill=(230, 230, 230), anchor="lm")
    for i, name in enumerate(("arrival", "junction", "return")):
        y = ly + 14 + (4 + i) * 26 - (0 if i == 0 else 0)
        col = VIEW_COLORS[name]
        d.rectangle((lx, y - 7, lx + 60, y + 7), outline=col, width=3)
        tag, nm = VIEW_NAMES[name]
        d.text((lx + 72, y), f"{tag}: camera frame at {nm}", font=small, fill=col, anchor="lm")
    d.text((img.size[0] - 14, img.size[1] - 12),
           f"{f['id']}  image {int(f['image'][0])}x{int(f['image'][1])}", font=mono,
           fill=(150, 160, 160), anchor="rd")
    return img.convert("RGB"), baked


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sorted(n for n in os.listdir(FLOORS) if n.endswith(".floor"))
    if not names:
        sys.exit("no .floor layouts in " + FLOORS)
    for n in names:
        f = parse(os.path.join(FLOORS, n))
        img, baked = render(f)
        out = os.path.join(OUT, f["id"] + "_plan.png")
        img.save(out, optimize=True)
        print(f"{out}: {img.size[0]}x{img.size[1]}, {os.path.getsize(out) // 1024} KiB, "
              f"{'baked' if baked else 'schematic'} background")


if __name__ == "__main__":
    main()
