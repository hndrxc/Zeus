"""Dead Letter / The Undertow Post - canonical asset baker.

Usage (from repo root):
  "C:/Program Files/Blender Foundation/Blender 5.2/blender.exe" -b -P tools/bake_assets.py -- [--only arena courier ...] [--engine EEVEE|CYCLES]

The arena is framed in a 900x600 layout with ground origin (450,308).
Sprites use a 256x256 layout with foot anchor (128,176). SS controls output
pixels per layout pixel; the orthographic projection uses PX layout pixels per unit.
"""
import bpy, bmesh, sys, os, math, random, time
from mathutils import Vector, Matrix
from contextlib import contextmanager
from pathlib import Path
from types import SimpleNamespace

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets")
PREVIEW = os.path.join(ROOT, "target", "art_preview")
ZOOM = 1
# Preserve the cardinal file indices, then add the four intervening views.
VIEW_ANGLES = (0, 90, 180, 270, 45, 135, 225, 315)
# Output pixels per design unit: the scene lays out in a 900x600 design space and
# stretches it to the window (and to HiDPI), so assets are baked well above 1x.
SS = 3
PX = 62.225
S2 = math.sqrt(2.0)
CAM_DIR = Vector((1.0, -1.0, math.tan(math.asin(0.55)) * S2))

MATS = {}
OBJS = []


# --------------------------------------------------------------- scene setup
def fresh():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    MATS.clear()
    OBJS.clear()
    return bpy.context.scene


def mat(name, color, metal=0.0, rough=0.5, emit=None, strength=0.0, coat=0.0):
    if name in MATS:
        return MATS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*color, 1.0)
    b.inputs["Metallic"].default_value = metal
    b.inputs["Roughness"].default_value = rough
    if coat:
        for k in ("Coat Weight", "Clearcoat"):
            if k in b.inputs:
                b.inputs[k].default_value = coat
                break
    if emit is not None:
        b.inputs["Emission Color"].default_value = (*emit, 1.0)
        b.inputs["Emission Strength"].default_value = strength
    MATS[name] = m
    return m


def P():
    """palette"""
    return dict(
        teal_d=mat("teal_d", (0.010, 0.050, 0.060), 0.2, 0.28, coat=0.6),
        teal_m=mat("teal_m", (0.020, 0.100, 0.115), 0.2, 0.25, coat=0.6),
        teal_l=mat("teal_l", (0.045, 0.19, 0.20), 0.3, 0.22, coat=0.7),
        black=mat("black", (0.012, 0.014, 0.018), 0.3, 0.3, coat=0.5),
        copper=mat("copper", (0.70, 0.30, 0.14), 1.0, 0.32),
        brass=mat("brass", (0.85, 0.58, 0.14), 1.0, 0.28),
        brass_d=mat("brass_d", (0.45, 0.30, 0.08), 1.0, 0.4),
        pearl=mat("pearl", (0.80, 0.77, 0.66), 0.0, 0.35, coat=0.5),
        paper=mat("paper", (0.75, 0.70, 0.55), 0.0, 0.8),
        wax=mat("wax", (0.60, 0.025, 0.02), 0.0, 0.18, coat=1.0),
        wax_d=mat("wax_d", (0.30, 0.012, 0.012), 0.0, 0.25, coat=0.8),
        sulfur=mat("sulfur", (0.90, 0.72, 0.04), 0.0, 0.4),
        rust=mat("rust", (0.20, 0.09, 0.05), 0.7, 0.6),
        steel=mat("steel", (0.16, 0.22, 0.24), 0.9, 0.4),
        glass=mat("glass", (0.02, 0.10, 0.12), 0.0, 0.05, emit=(0.05, 0.4, 0.45), strength=0.6),
        gold_l=mat("gold_l", (1.0, 0.7, 0.15), 0.0, 0.3, emit=(1.0, 0.72, 0.18), strength=6.0),
        cyan_l=mat("cyan_l", (0.1, 0.8, 1.0), 0.0, 0.3, emit=(0.15, 0.85, 1.0), strength=9.0),
        amber_l=mat("amber_l", (1.0, 0.5, 0.1), 0.0, 0.3, emit=(1.0, 0.45, 0.08), strength=8.0),
        red_l=mat("red_l", (1.0, 0.1, 0.05), 0.0, 0.3, emit=(1.0, 0.08, 0.04), strength=7.0),
        dim_b=mat("dim_b", (0.2, 0.5, 0.5), 0.0, 0.5, emit=(0.2, 0.55, 0.55), strength=0.7),
        dial=mat("dial", (0.8, 0.8, 0.6), 0.0, 0.4, emit=(0.8, 0.85, 0.6), strength=1.8),
        gunmetal=mat("gunmetal", (0.055, 0.065, 0.075), 0.9, 0.42),
        amethyst=mat("amethyst", (0.16, 0.05, 0.22), 0.05, 0.26, coat=0.8),
        lilac=mat("lilac", (0.62, 0.42, 1.0), 0.0, 0.35, emit=(0.58, 0.34, 1.0), strength=5.0),
    )


def use_gpu(sc):
    """Render Cycles on the first GPU backend with a device; keep the CPU otherwise."""
    prefs = bpy.context.preferences.addons["cycles"].preferences
    for backend in ("OPTIX", "CUDA", "HIP", "ONEAPI", "METAL"):
        try:
            prefs.compute_device_type = backend
        except TypeError:
            continue
        devices = prefs.get_devices_for_type(backend)
        gpus = [d for d in devices if d.type != "CPU"]
        if gpus:
            for d in devices:
                d.use = d.type != "CPU"
            sc.cycles.device = "GPU"
            sc.cycles.denoising_use_gpu = True
            print(f"Cycles: {backend} on {', '.join(d.name for d in gpus)}")
            return
    prefs.compute_device_type = "NONE"
    sc.cycles.device = "CPU"
    print("Cycles: no GPU backend found, rendering on the CPU")


def setup(res_x, res_y, ortho, shift_y_px, transparent, engine="EEVEE", samples=48, gpu=True):
    sc = bpy.context.scene
    r = sc.render
    r.resolution_x, r.resolution_y, r.resolution_percentage = res_x, res_y, 100
    r.film_transparent = transparent
    r.image_settings.file_format = "PNG"
    r.image_settings.color_mode = "RGBA" if transparent else "RGB"
    r.image_settings.color_depth = "8"
    r.filter_size = 1.2
    ok = False
    if engine == "EEVEE":
        for name in ("BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"):
            try:
                r.engine = name
                ok = True
                break
            except TypeError:
                pass
        if ok:
            try:
                sc.eevee.taa_render_samples = samples
            except Exception:
                pass
            for a, v in (("use_gtao", True), ("use_shadows", True), ("use_raytracing", True)):
                if hasattr(sc.eevee, a):
                    try:
                        setattr(sc.eevee, a, v)
                    except Exception:
                        pass
    if not ok:
        r.engine = "CYCLES"
        sc.cycles.samples = 64
        sc.cycles.use_denoising = True
        if gpu:
            use_gpu(sc)
        else:
            sc.cycles.device = "CPU"
    try:
        sc.view_settings.view_transform = "Standard"
        sc.view_settings.look = "None"
    except Exception:
        pass
    cam = bpy.data.cameras.new("cam")
    cam.type = "ORTHO"
    cam.ortho_scale = ortho
    cam.clip_start, cam.clip_end = 0.1, 400
    cam.sensor_fit = "HORIZONTAL"
    cam.shift_y = shift_y_px / res_x
    co = bpy.data.objects.new("cam", cam)
    bpy.context.collection.objects.link(co)
    co.location = CAM_DIR.normalized() * 60
    co.rotation_euler = (-co.location).to_track_quat("-Z", "Y").to_euler()
    sc.camera = co
    return sc


def world_color(col, strength=1.0):
    w = bpy.data.worlds.new("w")
    w.use_nodes = True
    bg = w.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (*col, 1)
    bg.inputs["Strength"].default_value = strength
    bpy.context.scene.world = w


def light(kind, pos, energy, color, size=1.0):
    l = bpy.data.lights.new("l", kind)
    l.energy = energy
    l.color = color
    if kind == "SUN":
        l.angle = math.radians(8)
    elif kind == "POINT":
        l.shadow_soft_size = size
    o = bpy.data.objects.new("l", l)
    bpy.context.collection.objects.link(o)
    o.location = pos
    if kind == "SUN":
        o.rotation_euler = (-Vector(pos)).to_track_quat("-Z", "Y").to_euler()
    return o


# --------------------------------------------------------------- primitives
def new_obj(bm, name, loc, rot, m, smooth=False):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.location = loc
    ob.rotation_euler = rot
    if m is not None:
        me.materials.append(m)
    if smooth:
        for p in me.polygons:
            p.use_smooth = True
    OBJS.append(ob)
    return ob


def box(loc, size, m, rot=(0, 0, 0), bev=0.0, name="box"):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=size, verts=bm.verts)
    if bev > 0:
        bmesh.ops.bevel(bm, geom=bm.edges[:], offset=min(bev, min(size) * 0.45), segments=2, affect="EDGES")
    return new_obj(bm, name, loc, rot, m, smooth=False)


def cyl(loc, r, h, m, rot=(0, 0, 0), r2=None, seg=24, smooth=True, name="cyl"):
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=seg,
                          radius1=r, radius2=r if r2 is None else r2, depth=h)
    return new_obj(bm, name, loc, rot, m, smooth=smooth)


def sph(loc, r, m, scale=(1, 1, 1), seg=24, name="sph"):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=max(6, seg // 2), radius=r)
    bmesh.ops.scale(bm, vec=scale, verts=bm.verts)
    return new_obj(bm, name, loc, (0, 0, 0), m, smooth=True)


def torus(loc, R, r, m, rot=(0, 0, 0), maj=40, mnr=10):
    bm = bmesh.new()
    vs = []
    for i in range(maj):
        a = 2 * math.pi * i / maj
        row = []
        for j in range(mnr):
            b = 2 * math.pi * j / mnr
            rr = R + r * math.cos(b)
            row.append(bm.verts.new((rr * math.cos(a), rr * math.sin(a), r * math.sin(b))))
        vs.append(row)
    for i in range(maj):
        for j in range(mnr):
            bm.faces.new((vs[i][j], vs[(i + 1) % maj][j], vs[(i + 1) % maj][(j + 1) % mnr], vs[i][(j + 1) % mnr]))
    return new_obj(bm, "torus", loc, rot, m, smooth=True)


def text(loc, s, size, m, rotz=0.0, extrude=0.006, name="txt"):
    c = bpy.data.curves.new(name, "FONT")
    c.body = s
    c.size = size
    c.align_x = "CENTER"
    c.align_y = "CENTER"
    c.extrude = extrude
    ob = bpy.data.objects.new(name, c)
    bpy.context.collection.objects.link(ob)
    ob.location = loc
    ob.rotation_euler = (0, 0, rotz)
    c.materials.append(m)
    OBJS.append(ob)
    return ob


def beam(a, b, r, m, seg=12, name="beam"):
    a, b = Vector(a), Vector(b)
    d = b - a
    q = d.to_track_quat("Z", "Y")
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=d.length)
    ob = new_obj(bm, name, (a + b) / 2, (0, 0, 0), m, smooth=True)
    ob.rotation_euler = q.to_euler()
    return ob


def group_rotate(rotz_deg=45.0, scale=1.0):
    e = bpy.data.objects.new("grp", None)
    bpy.context.collection.objects.link(e)
    e.rotation_euler = (0, 0, math.radians(rotz_deg))
    e.scale = (scale, scale, scale)
    for o in OBJS:
        o.parent = e
    OBJS.clear()
    return e


# floor (u,v) screen axes -> world xy
def uv(u, v):
    return ((u - v) / S2, (u + v) / S2)


# --------------------------------------------------------------- arena
def arena_base(M, rnd):
    """Shared floor slab, tiles, border and side walls (all chambers)."""
    H = 4.2
    # slab with chamfered vertical edges
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(2 * H, 2 * H, 0.9), verts=bm.verts)
    vert = [e for e in bm.edges if abs(e.verts[0].co.z - e.verts[1].co.z) > 0.5]
    bmesh.ops.bevel(bm, geom=vert, offset=0.25, segments=1, affect="EDGES")
    new_obj(bm, "slab", (0, 0, -0.03 - 0.45), (0, 0, 0), M["black"])
    # copper grid base visible in the seams
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(8.12, 8.12, 0.02), verts=bm.verts)
    new_obj(bm, "seams", (0, 0, -0.02), (0, 0, 0), M["copper"])

    # tiles
    n = 8
    pitch = 8.1 / n
    for i in range(n):
        for j in range(n):
            cx = -4.05 + pitch * (i + 0.5)
            cy = -4.05 + pitch * (j + 0.5)
            k = (i + j) % 2
            m = M["teal_m"] if k else M["teal_d"]
            if rnd.random() < 0.08:
                m = M["black"]
            box((cx, cy, -0.02), (pitch - 0.05, pitch - 0.05, 0.04), m, bev=0.03, name="tile")
            pm = M["teal_l"] if (i * 3 + j * 5) % 7 == 0 else (M["teal_d"] if k else M["teal_m"])
            box((cx, cy, 0.0), (pitch - 0.34, pitch - 0.34, 0.016), pm, bev=0.012, name="plate")
            # corner rivets on every tile
            for sx in (-1, 1):
                for sy in (-1, 1):
                    cyl((cx + sx * 0.4, cy + sy * 0.4, 0.004), 0.018, 0.012, M["brass_d"], seg=8, smooth=False)

    # copper seam lines (grid) + glowing nodes at some junctions
    for i in range(n + 1):
        c = -4.05 + pitch * i
        box((c, 0, -0.004), (0.012, 8.1, 0.008), M["copper"], name="seam")
        box((0, c, -0.004), (8.1, 0.012, 0.008), M["copper"], name="seam")
    for i in range(1, n):
        for j in range(1, n):
            if rnd.random() < 0.22:
                c = M["cyan_l"] if rnd.random() < 0.6 else M["amber_l"]
                box((-4.05 + pitch * i, -4.05 + pitch * j, 0.0), (0.07, 0.07, 0.014), c, name="node")

    # hazard border
    for side in range(4):
        ax = [(1, 0), (0, 1)][side % 2]
        sgn = 1 if side < 2 else -1
        for k in range(-13, 13):
            t = (k + 0.5) * 0.29
            if abs(t) > 3.75:
                continue
            m = M["sulfur"] if k % 2 == 0 else M["black"]
            if ax == (1, 0):
                box((sgn * 4.125, t, 0.0), (0.1, 0.27, 0.012), m, rot=(0, 0, 0.5), name="haz")
            else:
                box((t, sgn * 4.125, 0.0), (0.27, 0.1, 0.012), m, rot=(0, 0, 0.5), name="haz")
    # edge lip
    for s in (-1, 1):
        box((s * 4.17, 0, -0.03), (0.06, 8.0, 0.06), M["copper"], name="lip")
        box((0, s * 4.17, -0.03), (8.0, 0.06, 0.06), M["copper"], name="lip")

    # side walls: copper bands, rivets, riveted plates (faces +x and -y are visible)
    for z in (-0.12, -0.78):
        box((4.2, 0, z), (0.05, 8.0, 0.1), M["copper"], name="band")
        box((0, -4.2, z), (8.0, 0.05, 0.1), M["copper"], name="band")
    for t in range(-15, 16):
        p = t * 0.26
        for z in (-0.12, -0.78):
            cyl((4.23, p, z), 0.03, 0.03, M["brass"], rot=(0, math.pi / 2, 0), seg=8)
            cyl((p, -4.23, z), 0.03, 0.03, M["brass"], rot=(math.pi / 2, 0, 0), seg=8)
    for t in range(-3, 4):
        box((4.21, t * 1.1, -0.45), (0.03, 0.8, 0.36), M["teal_d"], bev=0.02, name="panel")
        box((t * 1.1, -4.21, -0.45), (0.8, 0.03, 0.36), M["teal_d"], bev=0.02, name="panel")


def floor_type(u, v, words, size, material, tracking=1.07):
    x, y = uv(u, v)
    lettering = text((x, y, .043), words, size, material, rotz=math.pi / 4,
                     extrude=.001, name="Engraved inscription")
    face = lettering.data
    face.font = bpy.data.fonts.load(os.path.join(ROOT, "tools", "fonts", "DejaVuSerif.ttf"),
                                    check_existing=True)
    face.space_character = tracking
    # Restore some cap height lost to the oblique ground-plane projection.
    lettering.scale.y = 1.35
    return lettering


def floor_seal(M, title, legend, register):
    gold = mat("satin gold inlay", (.64, .43, .19), metal=.63, rough=.43)
    shell = mat("aged shell inlay", (.53, .49, .36), metal=.23, rough=.55)
    # A flush enamel cartouche masks the tile joints behind the letterforms.
    # Scale its die to the actual title width, including side bearings.
    title_obj = floor_type(0, .15, title, .54, gold)
    bpy.context.view_layer.update()
    width = max(1.55, title_obj.dimensions.x / 2 + .24)
    enamel = mat("seal enamel", (.022, .070, .073), metal=.22, rough=.48)
    for z, margin, material in ((.029, .032, M["brass_d"]), (.032, 0, enamel)):
        die = cyl((0, 0, z), 1, .004, material, rot=(0, 0, math.pi / 4),
                  seg=96, smooth=False, name="Oval postmark die")
        die.scale = (width + margin, 1.48 + margin, 1)
    floor_type(0, .78, register, .18, shell, 1.22)
    floor_type(0, -.53, legend, .19, shell, 1.12)
    # Paired hairlines and a lozenge make a small printer's ornament.
    for v in (-.94, 1.16):
        for side in (-1, 1):
            x, y = uv(side * .52, v)
            box((x, y, .043), (.82, .022, .008), gold, rot=(0, 0, math.pi / 4))
        x, y = uv(0, v)
        box((x, y, .043), (.081, .081, .008), gold)


def arena_sorting(M, rnd):
    """Sorting Floor only: inscriptions, markings, back/front props and its five
    point lights (three lamp posts, two machinery glows). The Vault and Office
    are lit by the shared suns in arena_backdrop plus their own lights."""
    # stencilled hull numbers
    for t in range(-3, 4, 2):
        text((4.24, t * 1.1, -0.45), "%02d" % (t + 4), 0.22, M["sulfur"], rotz=math.pi / 2, extrude=0.004).rotation_euler = (math.pi / 2, 0, math.pi / 2)
        text((t * 1.1, -4.24, -0.45), "DL-%d" % (t + 4), 0.2, M["pearl"], extrude=0.004).rotation_euler = (math.pi / 2, 0, 0)

    floor_seal(M, "Dead Letter", "THE UNDERTOW POST", "OFFICE  ·  4417")
    for u, v, words, size in (
        (0, 3.7, "Sort by Depth of Loss", .27), (0, -3.7, "Return to Sender", .30),
        (-3.9, .1, "Dispatch 04", .22), (3.9, .1, "Undeliverable", .22),
        (-2.2, -2.2, "Postage Due", .19), (2.2, 2.2, "No Living Addressee", .19),
    ):
        floor_type(u, v, words, size, M["paper"])
    # centre postmark rings and cancellation ticks
    for R, r in ((2.35, 0.035), (1.7, 0.02)):
        torus((0, 0, 0.014), R, r, M["copper"])
    x0, y0 = 0, 0
    for k in range(48):
        a = 2 * math.pi * k / 48
        L = 0.16 if k % 4 == 0 else 0.07
        rr = 2.35 + 0.1
        box((rr * math.cos(a), rr * math.sin(a), 0.012), (L, 0.02, 0.01), M["brass_d"], rot=(0, 0, a), name="tick")
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        box((2.0 * math.cos(a), 2.0 * math.sin(a), 0.014), (0.75, 0.05, 0.012), M["brass_d"], rot=(0, 0, a), name="spoke")
    # safety triangles
    for (u, v) in ((-3.0, 2.0), (3.0, -2.0), (3.0, 2.0), (-3.0, -2.0)):
        x, y = uv(u, v)
        cyl((x, y, 0.012), 0.34, 0.012, M["sulfur"], rot=(0, 0, math.radians(135)), seg=3, smooth=False)
        cyl((x, y, 0.016), 0.24, 0.012, M["black"], rot=(0, 0, math.radians(135)), seg=3, smooth=False)
        xx, yy = uv(u, v - 0.06)
        text((xx, yy, 0.024), "!", 0.28, M["sulfur"], rotz=math.pi / 4)
    # arrows pointing toward centre
    for (u, v, a) in ((-4.7, 0.0, 0), (4.7, 0.0, 180), (0.0, 4.9, 270), (0.0, -4.9, 90)):
        x, y = uv(u, v)
        cyl((x, y, 0.012), 0.2, 0.01, M["copper"], rot=(0, 0, math.radians(135 + a - 90 + 90)), seg=3, smooth=False)

    # scattered undeliverable letters
    for _ in range(26):
        while True:
            x, y = rnd.uniform(-3.9, 3.9), rnd.uniform(-3.9, 3.9)
            if math.hypot(x, y) > 2.6:
                break
        a = rnd.uniform(0, math.pi)
        box((x, y, 0.014), (0.26, 0.17, 0.008), M["paper"], rot=(0, 0, a), name="letter")
        dx, dy = 0.0, 0.0
        cyl((x, y, 0.02), 0.03, 0.006, M["wax"], rot=(0, 0, a), seg=10, smooth=False)

    # ---------------- back-left rack (x<-4.3) : sorting pigeonholes
    rx = -4.85
    box((rx - 0.05, -0.8, 1.45), (0.5, 4.2, 2.9), M["black"], bev=0.04, name="rack_back")
    ys = [-2.9 + i * 0.6 for i in range(8)]
    for zi in range(0, 7):
        z = 0.35 + zi * 0.42
        box((rx + 0.12, -0.8, z), (0.5, 4.2, 0.05), M["brass_d"], name="shelf")
    for y in ys:
        box((rx + 0.12, y, 1.55), (0.5, 0.05, 2.7), M["brass_d"], name="div")
    for yi in range(7):
        for zi in range(6):
            if rnd.random() < 0.6:
                a = rnd.uniform(-0.2, 0.2)
                box((rx + 0.2, ys[yi] + 0.3, 0.35 + zi * 0.42 + 0.17), (0.2, 0.3, 0.26), M["paper"], rot=(0, 0, a), name="mail")
            if rnd.random() < 0.18:
                box((rx + 0.4, ys[yi] + 0.3, 0.35 + zi * 0.42 + 0.36), (0.04, 0.12, 0.03), rnd.choice([M["amber_l"], M["cyan_l"], M["red_l"]]), name="led")
    box((rx + 0.38, -0.8, 3.0), (0.1, 4.2, 0.22), M["brass"], name="rack_head")
    text((rx + 0.44, -0.8, 3.0), "SORTING  HALL  -  LETTERS TO THE DROWNED", 0.14, M["black"], rotz=math.pi / 2).rotation_euler = (math.pi / 2, 0, math.pi / 2)
    # rack stands in water: legs
    for y in (-2.8, 1.2):
        box((rx + 0.1, y, -0.2), (0.4, 0.2, 0.5), M["rust"], name="leg")

    # ---------------- back-right chute machine (y>4.3)
    my = 4.95
    box((-1.2, my, 1.1), (4.2, 1.1, 2.2), M["teal_d"], bev=0.06, name="machine")
    box((-1.2, my - 0.57, 1.5), (3.8, 0.06, 0.9), M["black"], name="panel")
    # chute mouths
    for k, x in enumerate((-2.4, -1.2, 0.0)):
        box((x, my - 0.62, 1.5), (0.8, 0.06, 0.16), M["amber_l"], name="slot")
        box((x, my - 0.6, 1.5), (0.95, 0.07, 0.3), M["brass"], name="slot_f").hide_render = False
        box((x, my - 0.66, 1.5), (0.78, 0.04, 0.14), M["black"], name="slot_in")
        text((x, my - 0.68, 1.05), ["A-F", "G-N", "O-Z"][k], 0.2, M["brass"], extrude=0.01).rotation_euler = (math.pi / 2, 0, 0)
    # dials
    for k, x in enumerate((-2.9, -0.55, 0.5)):
        cyl((x, my - 0.6, 0.55), 0.17, 0.05, M["dial"], rot=(math.pi / 2, 0, 0), seg=24)
        torus((x, my - 0.62, 0.55), 0.17, 0.025, M["brass"], rot=(math.pi / 2, 0, 0), maj=24, mnr=6)
        box((x + 0.04, my - 0.64, 0.58), (0.01, 0.01, 0.13), M["red_l"], rot=(0, 0.6 * k, 0), name="needle")
    # hopper
    cyl((-1.2, my, 2.55), 0.9, 0.7, M["brass"], r2=0.35, seg=4, rot=(0, 0, math.pi / 4), smooth=False)
    cyl((-1.2, my, 3.1), 0.4, 0.5, M["copper"], seg=16)
    # Keep the conveyor against the chute, outside the reachable floor.
    belt_y = my - 0.65
    box((-1.2, belt_y, 0.12), (4.0, 0.28, 0.16), M["steel"], name="belt")
    for t in range(15):
        box((-3.0 + t * 0.28, belt_y, 0.22), (0.05, 0.26, 0.04), M["black"], name="slat")
    for t in range(3):
        box((-2.4 + t * 1.3, belt_y, 0.32), (0.3, 0.2, 0.1), M["paper"], rot=(0, 0, 0.2 * t), name="parcel")
    # pneumatic tubes
    for k, (z, yo) in enumerate(((2.3, 0.0), (2.0, 0.0), (2.6, 0.0))):
        beam((-3.3 + 0.3 * k, my + 0.1, z + 0.6), (1.3 - 0.2 * k, my + 0.1, z + 0.6), 0.1, M["copper"])
        for t in range(6):
            cyl((-3.0 + t * 0.8 + 0.2 * k, my + 0.1, z + 0.6), 0.14, 0.07, M["brass"], rot=(0, math.pi / 2, 0), seg=14)

    # asymmetrical pipe ruins along back-left
    px = -4.4
    beam((px, 3.8, 0.0), (px, 3.8, 3.4), 0.18, M["copper"])
    beam((px, 3.1, 0.0), (px, 3.1, 2.2), 0.12, M["steel"])
    sph((px, 3.1, 2.25), 0.14, M["steel"])
    beam((px, 3.1, 2.25), (px, 2.4, 2.6), 0.12, M["steel"])  # broken kinked
    cyl((px, 2.35, 2.62), 0.15, 0.16, M["rust"], r2=0.1, rot=(math.radians(-70), 0, 0))
    beam((px - 0.1, 3.8, 3.0), (px - 0.1, 0.2, 3.0), 0.12, M["copper"])
    for yy in (3.2, 2.2, 1.2):
        cyl((px - 0.1, yy, 3.0), 0.19, 0.08, M["brass"], rot=(math.pi / 2, 0, 0))
    beam((px, -3.5, 0.0), (px, -3.5, 2.8), 0.15, M["steel"])
    sph((px, -3.5, 2.8), 0.17, M["brass"])
    beam((px, -3.5, 2.8), (px - 0.4, -3.9, 3.1), 0.15, M["steel"])  # torn pipe end hanging out
    # leaning wreck pipe
    beam((px - 0.2, -3.9, 0.0), (px - 0.9, -3.4, 1.7), 0.2, M["rust"])
    for z in (0.8, 1.3):
        pass
    # gauge on pipe
    cyl((px + 0.3, 3.8, 1.5), 0.2, 0.06, M["dial"], rot=(0, math.pi / 2, 0))
    torus((px + 0.34, 3.8, 1.5), 0.2, 0.03, M["brass"], rot=(0, math.pi / 2, 0), maj=24, mnr=6)
    box((px + 0.38, 3.8, 1.55), (0.01, 0.01, 0.15), M["red_l"], rot=(0.5, 0, 0), name="needle")
    # vertical pipes at back-right
    for x, z in ((1.9, 3.2), (2.6, 2.4), (3.4, 3.0)):
        beam((x, 4.55 + 0.1 * x / 3, 0.0), (x, 4.55 + 0.1 * x / 3, z), 0.16, rnd.choice([M["copper"], M["steel"], M["brass_d"]]))
        cyl((x, 4.55 + 0.1 * x / 3, 0.35), 0.24, 0.1, M["brass"])
        cyl((x, 4.55 + 0.1 * x / 3, z * 0.6), 0.24, 0.1, M["brass"])
    beam((1.9, 4.55, 2.9), (3.4, 4.7, 2.9), 0.12, M["copper"])
    sph((3.4, 4.7, 2.9), 0.15, M["steel"])
    cyl((4.2, 4.9, 1.4), 0.28, 2.8, M["steel"], seg=16)
    cyl((4.2, 4.9, 2.9), 0.1, 0.5, M["rust"], r2=0.3, rot=(0.3, 0.1, 0))
    # corner pillar (back corner)
    box((-4.55, 4.55, 1.4), (0.6, 0.6, 2.8), M["teal_d"], bev=0.05, name="pillar")
    for z in (0.3, 1.2, 2.2):
        box((-4.55, 4.55, z), (0.7, 0.7, 0.1), M["brass"], name="pillar_band")
    sph((-4.55, 4.55, 3.0), 0.22, M["gold_l"])
    # lamps
    for (x, y) in ((-4.6, -3.2), (-4.6, 2.0), (3.6, 4.6)):
        beam((x, y, 0), (x, y, 1.6), 0.05, M["steel"])
        sph((x, y, 1.7), 0.14, M["amber_l"])
        light("POINT", (x + 0.3, y, 2.6), 25, (1.0, 0.55, 0.2), 0.3)
    # glow lights near machinery
    light("POINT", (-1.2, 3.6, 2.6), 40, (1.0, 0.55, 0.2), 0.4)
    light("POINT", (-4.0, -0.8, 3.2), 35, (0.3, 0.9, 1.0), 0.4)

    # low front-edge props (keep <=0.25 tall, outside arena)
    for k, y in enumerate((-3.2, -1.0, 1.4, 3.3)):
        cyl((4.5, y, 0.1), 0.12, 0.25, M["brass_d"], seg=10)
        box((4.5, y + 0.3, 0.08), (0.35, 0.22, 0.16), M["paper"], rot=(0, 0, 0.2 * k), name="sack")
    for x in (-3.3, -0.8, 1.9, 3.5):
        cyl((x, -4.5, 0.1), 0.12, 0.25, M["brass_d"], seg=10)
        box((x + 0.4, -4.5, 0.07), (0.3, 0.3, 0.14), M["rust"], name="crate")
    # chain between low posts (front)
    for k in range(10):
        box((-3.3 + k * 0.28, -4.5, 0.17 - 0.03 * math.sin(k / 9 * math.pi) * 2), (0.1, 0.03, 0.03), M["steel"], name="chain")


def arena_under(M):
    """Shared supports below the floor."""

    # underside supports in black water
    for (x, y) in ((-3.5, -3.5), (3.5, -3.5), (3.5, 3.5), (-3.5, 3.5), (0, -3.5), (3.5, 0), (-3.5, 0), (0, 3.5)):
        box((x, y, -2.6), (0.5, 0.5, 4.2), M["rust"], name="strut")
        box((x, y, -1.0), (0.7, 0.7, 0.14), M["copper"], name="strut_cap")
    # hanging cables below front
    for x in (-2.5, 1.3, 3.7):
        beam((x, -4.25, -0.7), (x + 0.2, -4.4, -3.5), 0.03, M["black"])


def arena_backdrop(M, rnd):
    """Shared water backdrop, bubbles and key lighting."""

    # water backdrop: a distant dark plane with gradient, bubbles and ghost pylons
    bpy.ops.mesh.primitive_plane_add(size=120, location=(0, 0, -14))
    pl = bpy.context.object
    wm = bpy.data.materials.new("water")
    wm.use_nodes = True
    nt = wm.node_tree
    nt.nodes.clear()
    tc = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(tc.outputs["Window"], sep.inputs[0])
    vm = nt.nodes.new("ShaderNodeVectorMath")
    vm.operation = "DISTANCE"
    vm.inputs[1].default_value = (0.5, 0.45, 0)
    nt.links.new(tc.outputs["Window"], vm.inputs[0])
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.0
    ramp.color_ramp.elements[0].color = (0.010, 0.060, 0.070, 1)
    ramp.color_ramp.elements[1].position = 0.75
    ramp.color_ramp.elements[1].color = (0.0015, 0.004, 0.006, 1)
    nt.links.new(vm.outputs["Value"], ramp.inputs[0])
    em = nt.nodes.new("ShaderNodeEmission")
    nt.links.new(ramp.outputs[0], em.inputs[0])
    outn = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(em.outputs[0], outn.inputs[0])
    pl.data.materials.append(wm)
    for _ in range(110):
        x, y, z = rnd.uniform(-10, 10), rnd.uniform(-10, 10), rnd.uniform(-9, -1.5)
        s = rnd.uniform(0.015, 0.045)
        sph((x, y, z), s, M["dim_b"], seg=8)

    # key lights
    light("SUN", (-0.7, -1.0, 1.6), 3.2, (1.0, 0.92, 0.8))
    light("SUN", (1.0, 0.6, 0.9), 1.2, (0.35, 0.85, 1.0))
    world_color((0.03, 0.07, 0.08), 1.0)


# --------------------------------------------------------------- characters
def lights_char():
    light("SUN", (0.4, -1.0, 1.5), 3.6, (1.0, 0.93, 0.82))
    light("SUN", (-1.0, 1.0, 0.8), 3.0, (0.35, 0.9, 1.0))
    light("SUN", (1.0, 0.5, 0.4), 1.5, (0.8, 0.9, 1.0))
    world_color((0.16, 0.22, 0.24), 1.0)


def pivot(name, position, parent=None):
    joint = bpy.data.objects.new(name, None)
    bpy.context.collection.objects.link(joint)
    joint.empty_display_size = 0.07
    joint.parent = parent
    joint.location = Vector(position) - (parent.matrix_world.translation if parent else Vector((0, 0, 0)))
    bpy.context.view_layer.update()
    return joint


@contextmanager
def attached(joint):
    first = len(OBJS)
    yield
    for obj in OBJS[first:]:
        obj.parent = joint
        obj.location -= joint.matrix_world.translation


def hose(points, radius, material, name):
    curve = bpy.data.curves.new(name, 'CURVE')
    curve.dimensions = '3D'
    curve.bevel_depth = radius
    curve.bevel_resolution = 3
    spline = curve.splines.new('BEZIER')
    spline.bezier_points.add(len(points) - 1)
    for knot, point in zip(spline.bezier_points, points):
        knot.co = point
        knot.handle_left_type = knot.handle_right_type = 'AUTO'
    obj = bpy.data.objects.new(name, curve)
    bpy.context.collection.objects.link(obj)
    curve.materials.append(material)
    OBJS.append(obj)


def build_bailiff(M):
    wax, wd = M["wax"], M["wax_d"]
    for s in (-1, 1):
        box((0.04, s * 0.22, 0.12), (0.4, 0.24, 0.24), M["black"], bev=0.06, name="tread")
        cyl((0.0, s * 0.22, 0.32), 0.09, 0.2, M["brass_d"], seg=12)
    sph((0, 0, 0.75), 0.5, wax, scale=(0.95, 1.0, 1.0), seg=32)
    # drips
    for (a, z, L) in ((0.9, 0.55, 0.22), (2.0, 0.65, 0.3), (3.5, 0.5, 0.18), (4.6, 0.6, 0.28), (5.6, 0.45, 0.2)):
        x, y = 0.5 * math.cos(a), 0.5 * math.sin(a)
        sph((x * 0.98, y * 0.98, z - L * 0.4), 0.06, wd, scale=(1, 1, L / 0.06 * 0.6), seg=10)
    # collar + brass face plate with slit eye
    torus((0.0, 0, 1.13), 0.3, 0.05, M["brass"], maj=30, mnr=8)
    box((0.41, 0, 1.0), (0.08, 0.36, 0.28), M["brass"], bev=0.03, name="faceplate")
    box((0.46, 0, 1.03), (0.02, 0.26, 0.05), M["amber_l"], name="eye")
    box((0.46, 0, 0.93), (0.02, 0.2, 0.03), M["black"], name="grille")
    # judge cap: stamp seal crown
    cyl((0.0, 0, 1.28), 0.28, 0.09, M["brass"], seg=24)
    cyl((0.0, 0, 1.36), 0.2, 0.08, wax, seg=24)
    sph((0.0, 0, 1.45), 0.07, M["gold_l"])
    # left arm tucked
    sph((0.0, -0.5, 0.9), 0.15, M["brass"])
    beam((0.0, -0.5, 0.9), (0.28, -0.52, 0.6), 0.1, wax)
    sph((0.3, -0.53, 0.55), 0.11, wd)
    # right arm raised with giant rubber stamp
    sph((0.0, 0.5, 0.9), 0.15, M["brass"])
    beam((0.0, 0.5, 0.9), (0.2, 0.62, 1.12), 0.11, wax)
    sph((0.22, 0.64, 1.15), 0.12, wd)
    beam((0.22, 0.64, 1.15), (0.22, 0.64, 1.42), 0.045, M["brass_d"])
    cyl((0.22, 0.64, 1.45), 0.1, 0.1, M["black"], seg=12)
    box((0.22, 0.64, 1.18), (0.36, 0.36, 0.08), M["brass"], bev=0.02, name="stamp")
    box((0.22, 0.64, 1.12), (0.3, 0.3, 0.05), wax, name="ink")
    # detail: tread links, brass belly band, stamp handle ring, cheek rivets
    for s in (-1, 1):
        for k in range(4):
            box((0.04 + (k - 1.5) * 0.09, s * 0.22, 0.245), (0.04, 0.26, 0.03), M["brass_d"], name="link")
        sph((0.46, s * 0.15, 1.0), 0.02, M["brass_d"], seg=8)
    torus((0.0, 0, 0.62), 0.5, 0.035, M["brass_d"], maj=36, mnr=6)
    torus((0.22, 0.64, 1.5), 0.08, 0.02, M["brass"], maj=16, mnr=6)
    box((0.22, 0.64, 1.07), (0.3, 0.3, 0.02), M["black"], name="stampface")
    light("POINT", (0.9, 0, 1.0), 12, (1.0, 0.4, 0.2), 0.2)


def build_censor(M):
    navy = M["teal_d"]
    for s in (-1, 1):
        beam((0.0, s * 0.14, 0.0), (0.0, s * 0.14, 0.6), 0.05, M["steel"])
        box((0.06, s * 0.14, 0.04), (0.26, 0.14, 0.08), M["black"], name="foot")
        sph((0.0, s * 0.14, 0.45), 0.07, M["brass_d"])
    box((0, 0, 0.98), (0.44, 0.5, 0.82), navy, bev=0.06, name="body")
    box((0, 0, 0.58), (0.5, 0.56, 0.08), M["brass"], name="skirt")
    # screen face with censor bars
    box((0.23, 0, 1.15), (0.03, 0.4, 0.4), M["cyan_l"], name="screen")
    for z in (1.27, 1.12, 1.0):
        box((0.25, 0, z), (0.02, 0.4, 0.06), M["black"], name="bar")
    box((0.25, 0.04, 1.2), (0.02, 0.18, 0.03), M["black"], name="bar")
    # telegram ticker slot & paper tape
    box((0.23, 0, 0.78), (0.04, 0.34, 0.06), M["black"], name="slot")
    box((0.3, 0, 0.64), (0.02, 0.18, 0.26), M["paper"], rot=(0, 0.15, 0), name="tape")
    for k, y in enumerate((-0.3, 0.3)):
        box((0.0, y * 1.0, 0.7 - 0.03 * k), (0.12, 0.02, 0.3), M["paper"], rot=(0.0, 0.1 * (1 if k else -1), 0.2), name="tape")
    # side horns
    for s in (-1, 1):
        beam((0.0, s * 0.28, 1.3), (0.1, s * 0.5, 1.46), 0.04, M["brass"])
        cyl((0.14, s * 0.56, 1.5), 0.04, 0.2, M["brass"], r2=0.14, rot=(0, math.pi / 3, s * 0.4), seg=14)
        beam((0.0, s * 0.25, 0.95), (0.18, s * 0.42, 0.62), 0.05, M["steel"])
        sph((0.2, s * 0.43, 0.6), 0.06, M["cyan_l"])
    # antenna
    beam((0, 0, 1.4), (0, 0, 1.7), 0.03, M["steel"])
    box((0, 0, 1.58), (0.02, 0.5, 0.025), M["steel"], name="dipole")
    box((0, 0, 1.66), (0.02, 0.32, 0.025), M["steel"], name="dipole")
    sph((0, 0, 1.73), 0.06, M["cyan_l"])
    for s in (-1, 1):
        sph((0, s * 0.25, 1.58), 0.03, M["cyan_l"])
    torus((0, 0, 1.4), 0.12, 0.02, M["brass"], maj=20, mnr=6)
    # detail: screen bezel, side speaker grilles, shoulder rivets, foot dials
    for sy in (-1, 1):
        box((0.23, sy * 0.22, 1.15), (0.05, 0.03, 0.46), M["brass"], bev=0.01, name="bezel")
    for sz in (-1, 1):
        box((0.23, 0, 1.15 + sz * 0.23), (0.05, 0.46, 0.03), M["brass"], bev=0.01, name="bezel")
    for s in (-1, 1):
        for zz in (0.85, 0.95, 1.05):
            box((0.0, s * 0.225, zz), (0.3, 0.02, 0.03), M["black"], name="vent")
        sph((0.1, s * 0.23, 1.33), 0.035, M["brass"], seg=10)
    for k in range(5):
        sph((0.215, -0.16 + k * 0.08, 0.84), 0.018, M["amber_l"], seg=8)
    light("POINT", (0.6, 0, 1.2), 25, (0.2, 0.9, 1.0), 0.2)


def build_postmaster(M):
    wax, wd = M["wax"], M["wax_d"]
    # base skirt (robe) with brass hem
    cyl((0, 0, 0.55), 0.95, 1.1, wax, r2=0.55, seg=20)
    cyl((0, 0, 0.04), 1.0, 0.08, M["brass"], seg=32)
    torus((0, 0, 0.1), 0.96, 0.04, M["brass"], maj=40, mnr=8)
    for k in range(6):
        a = k * math.pi / 3 + 0.3
        box((0.8 * math.cos(a), 0.8 * math.sin(a), 0.5), (0.04, 0.16, 0.75), wd, rot=(0, 0, a), name="fold")
    # torso cabinet
    box((0, 0, 1.45), (0.9, 1.0, 0.75), M["brass"], bev=0.08, name="torso")
    box((0.46, 0, 1.45), (0.04, 0.8, 0.6), M["black"], name="grille")
    for z in (1.25, 1.45, 1.65):
        box((0.49, 0, z), (0.02, 0.7, 0.04), M["brass_d"], name="slot")
    # chest seal
    cyl((0.5, 0, 1.4), 0.3, 0.08, wax, rot=(0, math.pi / 2, 0), seg=28)
    torus((0.54, 0, 1.4), 0.3, 0.04, M["gold_l"], rot=(0, math.pi / 2, 0), maj=28, mnr=8)
    cyl((0.56, 0, 1.4), 0.16, 0.03, wd, rot=(0, math.pi / 2, 0), seg=20)
    # sash
    box((0.0, 0, 1.2), (0.96, 1.04, 0.1), M["sulfur"], bev=0.02, name="sash")
    # shoulders
    for s in (-1, 1):
        sph((0, s * 0.62, 1.72), 0.26, M["brass"])
        torus((0, s * 0.62, 1.72), 0.26, 0.04, M["gold_l"], maj=24, mnr=6)
        # upper arms with stamps: one raised, one forward
    beam((0, 0.62, 1.72), (0.3, 0.9, 2.1), 0.13, wax)
    box((0.3, 0.9, 2.2), (0.5, 0.5, 0.1), M["brass"], bev=0.03, name="stamp")
    cyl((0.3, 0.9, 2.3), 0.08, 0.2, M["black"], seg=12)
    beam((0, -0.62, 1.72), (0.55, -0.85, 1.4), 0.13, wax)
    cyl((0.6, -0.9, 1.3), 0.2, 0.08, M["gold_l"], seg=20)
    box((0.6, -0.9, 1.22), (0.34, 0.34, 0.1), wax, bev=0.03, name="stamp")
    # head: clock face
    cyl((0.0, 0, 2.15), 0.42, 0.4, M["brass"], rot=(0, math.pi / 2, 0), seg=32)
    cyl((0.22, 0, 2.15), 0.36, 0.04, M["dial"], rot=(0, math.pi / 2, 0), seg=32)
    torus((0.22, 0, 2.15), 0.38, 0.04, M["gold_l"], rot=(0, math.pi / 2, 0), maj=36, mnr=8)
    for k in range(12):
        a = k * math.pi / 6
        box((0.25, 0.3 * math.sin(a), 2.15 + 0.3 * math.cos(a)), (0.02, 0.03, 0.06), M["black"], rot=(a, 0, 0), name="tick")
    box((0.26, 0.0, 2.25), (0.02, 0.03, 0.2), M["black"], name="hand")
    box((0.26, 0.09, 2.17), (0.02, 0.18, 0.03), M["black"], rot=(-0.5, 0, 0), name="hand")
    sph((0.27, 0, 2.15), 0.04, M["red_l"])
    # crown of mail spikes
    for k in range(9):
        a = math.pi * (k - 4) / 8
        cyl((0.0, 0.42 * math.sin(a) * 0.9, 2.15 + 0.42 * math.cos(a) * 0.95), 0.05, 0.22, M["gold_l"] if k % 4 == 0 else M["brass"],
            r2=0.0, rot=(-a, 0, 0), seg=8, smooth=False)
    # top lamp
    cyl((0, 0, 2.62), 0.05, 0.1, M["brass"], seg=10)
    # sorting chutes on the back
    for s in (-1, 1):
        cyl((-0.5, s * 0.35, 1.6), 0.16, 0.7, M["copper"], rot=(0, 0.6, 0), seg=14)
    # detail: hem studs, clock bezel bolts, shoulder pauldron plates, hand-bell lamp
    for k in range(16):
        a = k * math.pi / 8
        sph((0.99 * math.cos(a), 0.99 * math.sin(a), 0.1), 0.045, M["gold_l"] if k % 4 == 0 else M["brass_d"], seg=8)
    for s in (-1, 1):
        box((0, s * 0.62, 1.98), (0.34, 0.3, 0.06), M["brass_d"], bev=0.02, name="pauldron")
        cyl((0.1, s * 0.5, 1.2), 0.06, 0.3, M["copper"], seg=10)
    for k in range(12):
        a = k * math.pi / 6 + math.pi / 12
        sph((0.25, 0.4 * math.sin(a), 2.15 + 0.4 * math.cos(a)), 0.022, M["gold_l"], seg=8)
    box((0.5, 0, 1.78), (0.06, 0.5, 0.05), M["black"], name="brow")
    sph((0, 0, 2.7), 0.06, M["red_l"], seg=10)
    light("POINT", (1.2, 0, 1.6), 55, (1.0, 0.55, 0.25), 0.3)


# --------------------------------------------------------------- explorable floors
FLOOR_SS = 2


def load_floor(path):
    """Read the gameplay layout. Preserve block line order for the runtime atlas."""
    path = Path(path)
    layout = SimpleNamespace(blocks=[], doors={}, routes={}, views={})
    arity = {"floor": 1, "image": 4, "bounds": 4, "entrance": 4, "landmark": 2,
             "block": 6, "door": 5, "view": 3}
    named = {"block": ("wall", "counter", "table", "rack", "stack", "railing", "landmark"),
             "door": ("stop", "side"), "route": ("main", "loop", "side"),
             "view": ("arrival", "junction", "return")}
    singles = {"floor", "image", "bounds", "entrance", "landmark"}
    seen = set()
    line_no = 0

    def fail(message):
        raise ValueError(f"{path}:{line_no}: {message}")

    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        fields = line.partition("#")[0].split()
        if not fields:
            continue
        key, *args = fields
        if key == "route":
            if len(args) < 5 or len(args) % 2 != 1:
                fail("route needs a name and at least two (x, y) pairs")
        elif key not in arity:
            fail(f"unknown record {key!r}")
        elif len(args) != arity[key]:
            fail(f"{key} expects {arity[key]} fields, got {len(args)}")
        if key in singles:
            if key in seen:
                fail(f"duplicate {key}")
            seen.add(key)
        if key == "floor":
            if args[0] != path.stem:
                fail(f"floor id must match file stem {path.stem!r}")
            layout.id = args[0]
            continue
        if key in named:
            name = args.pop(0)
            if name not in named[key]:
                fail(f"unknown {key} kind {name!r}")
        try:
            values = tuple(map(float, args))
        except ValueError as exc:
            fail(str(exc))
        if not all(map(math.isfinite, values)):
            fail(f"{key} coordinates must be finite")
        if key in ("block", "door", "bounds"):
            x0, y0, x1, y1 = values[:4]
            if x0 >= x1 or y0 >= y1:
                fail(f"{key} rectangle must have positive extents")
        if key == "block":
            if values[4] <= 0:
                fail("block height must be positive")
            layout.blocks.append(SimpleNamespace(index=len(layout.blocks), kind=name,
                                                 x0=x0, y0=y0, x1=x1, y1=y1, h=values[4]))
        elif key in named:
            records = getattr(layout, key + "s")
            if name in records:
                fail(f"duplicate {key} {name}")
            records[name] = list(zip(values[::2], values[1::2])) if key == "route" else values
        else:
            if key == "image":
                w, h, ox, oy = values
                if not (w.is_integer() and h.is_integer() and w > 0 and h > 0):
                    fail("image needs positive integer width and height")
                values = (int(w), int(h), ox, oy)
            setattr(layout, key, values)

    line_no += 1
    for key in sorted(singles - seen):
        fail(f"missing {key}")
    for key in ("door", "route", "view"):
        for name in named[key]:
            if name not in getattr(layout, key + "s"):
                fail(f"missing {key} {name}")
    if not layout.blocks:
        fail("missing blocks")
    return layout


def floor_scene(engine, layout):
    """Fresh floor scene with the arena camera, shifted so world origin lands at (OX, OY).

    World (x, y, z) then lands at design px (OX + 44(x+y), OY + 24.2(x-y) - 52z),
    matching the contract. setup() only sets shift_y, as (OY - H/2)/W design px
    expressed against the 2W-pixel render width; shift_x is the same derivation
    along the horizontal axis, negated because a positive Blender shift_x moves
    the projected point left."""
    fresh()
    M = P()
    W, H, OX, OY = layout.image
    # CPU only, like arena_scene: coplanar floor/block faces must resolve the
    # same way on every bake.
    setup(2 * W, 2 * H, W / PX, (OY - H / 2) * FLOOR_SS, False, engine, samples=64, gpu=False)
    bpy.context.scene.camera.data.shift_x = -(OX - W / 2) / W
    return M


def floor_registration(layout):
    """Contract projection vs Blender camera for layout control points.

    Covers the entrance, both door rectangles and block corners at z=0 and z=h.
    Returns the max error in design px; raises at the 0.5 px contract limit."""
    from bpy_extras.object_utils import world_to_camera_view
    W, H, OX, OY = layout.image
    sc = bpy.context.scene
    bpy.context.view_layer.update()
    points = [tuple(layout.entrance[:2]) + (0.0,)]
    for x0, y0, x1, y1 in layout.doors.values():
        points += [(x0, y0, 0.0), (x1, y0, 0.0), (x0, y1, 0.0), (x1, y1, 0.0)]
    picked, seen = [], set()
    for b in (layout.blocks[0], max(layout.blocks, key=lambda b: b.h),
              layout.blocks[len(layout.blocks) // 2]):
        if b.index not in seen:
            seen.add(b.index)
            picked.append(b)
    for b in picked:
        for z in (0.0, b.h):
            points += [(b.x0, b.y0, z), (b.x1, b.y1, z)]
    worst = 0.0
    for x, y, z in points:
        v = world_to_camera_view(sc, sc.camera, Vector((x, y, z)))
        ax, ay = v.x * W, (1 - v.y) * H
        ex, ey = OX + 44 * (x + y), OY + 24.2 * (x - y) - 52 * z
        worst = max(worst, math.hypot(ax - ex, ay - ey))
    print(f"registration {layout.id}: {len(points)} points, max error {worst:.4f} design px")
    if worst >= 0.5:
        raise ValueError(f"{layout.id}: registration error {worst:.4f} design px >= 0.5")
    return worst


def graybox_floor(M, layout):
    """Plain-box stand-in for a floor: slab, cut edge, dark water, tagged blocks,
    door marks and neutral lights. Used with --graybox and before the floor script exists."""
    x0, y0, x1, y1 = layout.bounds
    cx, cy, dx, dy = (x0 + x1) / 2, (y0 + y1) / 2, x1 - x0, y1 - y0
    floor_m = mat("graybox floor", (0.045, 0.11, 0.125), 0.0, 0.8)
    edge_m = mat("graybox edge", (0.42, 0.30, 0.10), 0.6, 0.5)
    box((cx, cy, -0.25), (dx, dy, 0.5), floor_m, name="floor_slab")
    # Cut slab edge: a brass-dark trim frame on the slab perimeter.
    box((cx, y0 + 0.06, 0.01), (dx, 0.12, 0.04), edge_m, name="slab_edge")
    box((cx, y1 - 0.06, 0.01), (dx, 0.12, 0.04), edge_m, name="slab_edge")
    box((x0 + 0.06, cy, 0.01), (0.12, dy, 0.04), edge_m, name="slab_edge")
    box((x1 - 0.06, cy, 0.01), (0.12, dy, 0.04), edge_m, name="slab_edge")
    # Dark water below, like arena_under: margins fade to the INK abyss.
    ink_m = bpy.data.materials.new("graybox ink")
    ink_m.use_nodes = True
    ink_tree = ink_m.node_tree
    ink_tree.nodes.clear()
    ink_em = ink_tree.nodes.new("ShaderNodeEmission")
    ink_em.inputs["Color"].default_value = (0.002, 0.006, 0.007, 1.0)
    ink_em.inputs["Strength"].default_value = 1.0
    ink_out = ink_tree.nodes.new("ShaderNodeOutputMaterial")
    ink_tree.links.new(ink_em.outputs[0], ink_out.inputs[0])
    box((cx, cy, -3.2), (dx + 90, dy + 90, 0.4), ink_m, name="water")
    kind_colors = {
        "wall": (0.36, 0.39, 0.43), "counter": (0.46, 0.39, 0.30),
        "table": (0.38, 0.43, 0.33), "rack": (0.30, 0.37, 0.43),
        "stack": (0.43, 0.35, 0.39), "railing": (0.56, 0.56, 0.59),
        "landmark": (0.85, 0.62, 0.18),
    }
    for b in layout.blocks:
        ob = box(((b.x0 + b.x1) / 2, (b.y0 + b.y1) / 2, b.h / 2),
                 (b.x1 - b.x0, b.y1 - b.y0, b.h),
                 mat(f"graybox {b.kind}", kind_colors[b.kind], 0.0, 0.65),
                 name=f"block_{b.index}")
        ob["block"] = b.index
    for kind, (x0, y0, x1, y1) in layout.doors.items():
        if kind == "stop":
            m = mat("graybox stop", (0.25, 0.12, 0.03), 0.0, 0.6,
                    emit=(1.0, 0.55, 0.15), strength=0.9)
        else:
            m = mat("graybox side", (0.14, 0.07, 0.28), 0.0, 0.6,
                    emit=(0.55, 0.35, 1.0), strength=0.9)
        box(((x0 + x1) / 2, (y0 + y1) / 2, 0.01), (x1 - x0, y1 - y0, 0.02), m,
            name=f"door_{kind}")
    light("SUN", (-0.7, -1.0, 1.6), 3.0, (1.0, 0.92, 0.8))
    light("SUN", (1.0, 0.6, 0.9), 1.0, (0.35, 0.85, 1.0))
    world_color((0.004, 0.011, 0.013), 1.0)


def floor_occluders(layout, png_path):
    """Render the object-index pass and pack visible block pieces into the atlas.

    Piece RGB is copied from the encoded background PNG, so it matches the
    background byte for byte. Alpha is the hard visible-surface mask from the
    index pass (pass_index = block + 1 on every tagged object)."""
    import numpy as np
    import OpenImageIO as oiio
    sc = bpy.context.scene
    tagged = {}
    for ob in sc.objects:
        if "block" not in ob:
            continue
        index = int(ob["block"])
        if not 0 <= index < len(layout.blocks):
            raise ValueError(f"{ob.name}: block tag {index} out of range")
        ob.pass_index = index + 1
        tagged.setdefault(index, []).append(ob.name)
    work = Path(ROOT) / "target" / "floors" / layout.id
    work.mkdir(parents=True, exist_ok=True)
    for stale in work.glob("*.exr"):
        stale.unlink()
    view_layer = sc.view_layers[0]
    view_layer.use_pass_object_index = True
    view_layer.update_render_passes()
    group = bpy.data.node_groups.new("floor_index", "CompositorNodeTree")
    sc.compositing_node_group = group
    layers = group.nodes.new("CompositorNodeRLayers")
    dump = group.nodes.new("CompositorNodeOutputFile")
    dump.directory = str(work) + os.sep
    dump.file_name = "index"
    dump.format.file_format = "OPEN_EXR_MULTILAYER"
    dump.file_output_items.new("FLOAT", "idx")
    group.links.new(layers.outputs["Object Index"], dump.inputs[0])
    bpy.ops.render.render(write_still=False)
    exrs = sorted(work.glob("*.exr"))
    if len(exrs) != 1:
        raise ValueError(f"{layout.id}: expected one index EXR, found {exrs}")
    index_buf = oiio.ImageBuf(str(exrs[0]))
    index = np.array(index_buf.get_pixels("float"), dtype=np.float32)
    bg_buf = oiio.ImageBuf(str(png_path))
    bg = np.array(bg_buf.get_pixels("uint8"), dtype=np.uint8)
    W, H = layout.image[0], layout.image[1]
    if index.shape[:2] != (2 * H, 2 * W) or bg.shape != (2 * H, 2 * W, 3):
        raise ValueError(f"{layout.id}: pass/background size mismatch "
                         f"{index.shape} vs {bg.shape}")
    index = index[..., 0] if index.ndim == 3 else index
    bx0, by0, bx1, by1 = layout.bounds
    pieces = []
    for b in layout.blocks:
        # Far boundary walls are never in front of walkable floor: no piece.
        if b.kind == "wall" and (b.x1 <= bx0 or b.y0 >= by1):
            continue
        rows, cols = np.where(index == b.index + 1)
        if rows.size == 0:
            raise ValueError(f"{layout.id}: block {b.index} ({b.kind}) has no visible pixels")
        r0, r1, c0, c1 = rows.min(), rows.max() + 1, cols.min(), cols.max() + 1
        rgba = np.zeros((r1 - r0, c1 - c0, 4), dtype=np.uint8)
        rgba[..., :3] = bg[r0:r1, c0:c1]
        rgba[..., 3] = np.where(index[r0:r1, c0:c1] == b.index + 1, 255, 0)
        pieces.append(SimpleNamespace(block=b.index, rgba=rgba, w=c1 - c0, h=r1 - r0,
                                      dx=c0 / FLOOR_SS, dy=r0 / FLOOR_SS))
    pad, max_w = 2, 4096
    x = y = row_h = atlas_w = 0
    for p in sorted(pieces, key=lambda p: (-p.h, p.block)):
        if x + p.w > max_w:
            x, y, row_h = 0, y + row_h + pad, 0
        p.ax, p.ay = x, y
        x += p.w + pad
        row_h = max(row_h, p.h)
        atlas_w = max(atlas_w, p.ax + p.w)
    atlas_h = y + row_h
    atlas = np.zeros((atlas_h, atlas_w, 4), dtype=np.uint8)
    for p in pieces:
        atlas[p.ay:p.ay + p.h, p.ax:p.ax + p.w] = p.rgba
    out_png = Path(png_path).with_name(f"{layout.id}_occ.png")
    atlas_buf = oiio.ImageBuf(oiio.ImageSpec(atlas_w, atlas_h, 4, "uint8"))
    atlas_buf.set_pixels(oiio.ROI(0, atlas_w, 0, atlas_h, 0, 1), atlas.tobytes())
    if not atlas_buf.write(str(out_png)):
        raise OSError(f"failed to write {out_png}: {atlas_buf.geterror()}")

    def num(value):
        value = float(value)
        return str(int(value)) if value.is_integer() else repr(value)

    table = ["# generated by tools/bake_assets.py; do not edit",
             f"atlas {atlas_w} {atlas_h}"]
    for p in sorted(pieces, key=lambda p: p.block):
        table.append(f"piece {p.block} {p.ax} {p.ay} {p.w} {p.h} "
                     f"{num(p.dx)} {num(p.dy)} {num(p.w / FLOOR_SS)} {num(p.h / FLOOR_SS)}")
    Path(png_path).with_name(f"{layout.id}.occ").write_text("\n".join(table) + "\n", encoding="utf-8")
    print(f"occluders {layout.id}: {len(pieces)} pieces, atlas {atlas_w}x{atlas_h}")
    print("WROTE", out_png)


FLOOR_SCRIPTS = {"sorting_hall": "floor_sorting.py", "vault_stacks": "floor_vault.py"}


def bake_floor(floor_id, engine, graybox, gpu_preview):
    import runpy
    started = time.perf_counter()
    layout = load_floor(os.path.join(ROOT, "assets", "floors", floor_id + ".floor"))
    M = floor_scene(engine, layout)
    floor_registration(layout)
    script = os.path.join(ROOT, "tools", FLOOR_SCRIPTS[floor_id])
    if not graybox and os.path.exists(script):
        # Same runpy pattern as the arena landmarks: the floor script sees this
        # module's globals (bpy, mat, box, light, ...) and builds the whole floor.
        runpy.run_path(script, init_globals=globals())["build"](M, layout, random.Random(7))
    else:
        graybox_floor(M, layout)
    if gpu_preview:
        # Dev-only GPU preview at reduced samples; never writes to assets/.
        sc = bpy.context.scene
        use_gpu(sc)
        sc.cycles.samples = 24
        os.makedirs(PREVIEW, exist_ok=True)
        render(os.path.join(PREVIEW, floor_id + "_preview.png"))
        print(f"floor preview {floor_id}: {time.perf_counter() - started:.1f}s (gpu)")
        return
    out_dir = os.path.join(OUT, "floors")
    os.makedirs(out_dir, exist_ok=True)
    png = os.path.join(out_dir, floor_id + ".png")
    render(png)
    floor_occluders(layout, png)
    print(f"floor {floor_id}: {time.perf_counter() - started:.1f}s "
          f"({'graybox' if graybox else 'scripted'})")


# --------------------------------------------------------------- driver
def render(path):
    bpy.context.scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    print("WROTE", path)


def arena_scene(engine):
    fresh()
    M = P()
    # CPU only: the platform lip and the Sorting Floor rack have coplanar faces
    # that OptiX resolves differently, so a GPU bake would change the committed art.
    setup(900 * SS, 600 * SS, 900 / PX, 8 * SS, False, engine, samples=64, gpu=False)
    return M


def bake_arena(engine):
    import runpy
    tools = os.path.join(ROOT, "tools")
    # Sorting Floor keeps its original draw order (floor, sorting props, backdrop)
    chambers = (
        ("arena.png", arena_sorting),
        ("arena_vault.png", lambda M, rnd: runpy.run_path(os.path.join(tools, "arena_vault.py"), init_globals=globals())["build"](M)),
        ("arena_office.png", lambda M, rnd: runpy.run_path(os.path.join(tools, "arena_office.py"), init_globals=globals())["build"](M)),
    )
    for out, chamber in chambers:
        M = arena_scene(engine)
        rnd = random.Random(7)
        arena_base(M, rnd)
        chamber(M, rnd)
        arena_under(M)
        arena_backdrop(M, rnd)
        render(os.path.join(OUT, out))


def bake_enemy(name, builder, engine):
    import runpy
    script = os.path.join(ROOT, "tools", "enemies.py")
    runpy.run_path(script, init_globals=globals())["bake"](name, builder, engine)

def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    global ZOOM
    engine = "CYCLES"
    if "--zoom" in argv:
        ZOOM = int(argv[argv.index("--zoom") + 1])
        os.makedirs(PREVIEW, exist_ok=True)
    if "--engine" in argv:
        engine = argv[argv.index("--engine") + 1].upper()
    graybox = "--graybox" in argv
    gpu_preview = "--gpu-preview" in argv or "--dev" in argv
    only = []
    if "--only" in argv:
        i = argv.index("--only") + 1
        while i < len(argv) and not argv[i].startswith("--"):
            only.append(argv[i])
            i += 1
    os.makedirs(OUT, exist_ok=True)
    jobs = {
        "arena": lambda: bake_arena(engine),
        "courier": lambda: bake_courier(engine),
        "bailiff": lambda: bake_enemy("bailiff", build_bailiff, engine),
        "sorting_hall": lambda: bake_floor("sorting_hall", engine, graybox, gpu_preview),
        "vault_stacks": lambda: bake_floor("vault_stacks", engine, graybox, gpu_preview),
        "censor": lambda: bake_enemy("censor", build_censor, engine),
        "postmaster": lambda: bake_enemy("postmaster", build_postmaster, engine),
        "ward_clerk": lambda: bake_ward_clerk(engine),
    }
    for k, f in jobs.items():
        if not only or k in only:
            f()


def bake_courier(engine):
    import runpy
    script = os.path.join(ROOT, "tools", "courier.py")
    runpy.run_path(script, init_globals=globals())["bake"](engine)


def bake_ward_clerk(engine):
    import runpy
    script = os.path.join(ROOT, "tools", "ward_clerk.py")
    builder = runpy.run_path(script, init_globals=globals())["build_rig"]
    bake_enemy("ward_clerk", builder, engine)


if __name__ == "__main__":
    main()
