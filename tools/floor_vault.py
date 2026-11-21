"""Dead Letter Vault archive stacks hall.

Geometry visible from the fixed game camera: a cutaway slab with a dark
brass cut edge, tall far walls with an entrance opening and two doorway
openings, low cut-down near walls, long ranks of tagged drawer stacks,
sectioned stacks with their cut tops showing drawers and swollen letters,
a chained register bay whose centrepiece is a brass card-index wheel, and
warm lamp pools against the INK abyss.
"""


def build(M, layout, rnd):
    def tag(i, ob):
        ob["block"] = i
        return ob

    def blk(i):
        return layout.blocks[i]

    def find(pred):
        for j, b in enumerate(layout.blocks):
            if pred(b):
                return j
        raise ValueError("layout block not found")

    def box_at(i, loc, size, m, bev=0.0, rot=(0.0, 0.0, 0.0), name="dress"):
        return tag(i, box(loc, size, m, rot=rot, bev=bev, name=name))

    # Pure black, no specular: what the CPU bake showed where two coplanar
    # faces shadowed each other.
    dead = mat("vault dead black", (0.0, 0.0, 0.0), 0.0, 1.0)
    dead.node_tree.nodes["Principled BSDF"].inputs["Specular IOR Level"].default_value = 0.0

    def top_shadow(cx, cy, z, sx, sy):
        """Keep a top face dark, as the CPU bake rendered it: a flat sheet 4 mm
        above the face that shadows it and is only hidden from the camera."""
        ob = box((cx, cy, z + 0.004), (sx, sy, 0.002), dead, name="top_shadow")
        ob.visible_camera = False
        return ob

    def face_shadow(loc, size):
        """Shadow-only sheet in front of a face: darkens it as the CPU bake did
        where coplanar faces shaded each other, leaving reflections alone."""
        ob = box(loc, size, dead, name="face_shadow")
        ob.visible_camera = False
        ob.visible_glossy = False
        ob.visible_transmission = False
        ob.visible_volume_scatter = False
        return ob

    def edge_band(i, bx0, by0, bx1, by1, h):
        """Dead-black band under the brass top edge on the camera-facing faces
        (-y and +x): the CPU bake showed the coplanar cap and wall faces there as
        a black line. Laid 1 mm proud of the face."""
        for loc, size in (
                (((bx0 + bx1) / 2.0, by0 - 0.001), (bx1 - bx0 - 0.10, 0.002)),
                ((bx1 + 0.001, (by0 + by1) / 2.0), (0.002, by1 - by0 - 0.10))):
            tag(i, box((loc[0], loc[1], h - 0.040), (size[0], size[1], 0.026), dead,
                       name="edge_band"))

    x0, y0, x1, y1 = layout.bounds
    cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
    width, depth = x1 - x0, y1 - y0

    # --- world and abyss -------------------------------------------------
    world_color((0.008, 0.02, 0.025), 1.0)
    ink = mat("vault ink", (0.002, 0.006, 0.007), emit=(0.0015, 0.005, 0.006),
              strength=1.0)
    box((cx, cy, -3.2), (width + 90.0, depth + 90.0, 0.5), ink, name="water")

    light("SUN", (-0.7, -1.0, 1.5), 1.25, (1.0, 0.88, 0.72), 2.0)
    light("SUN", (1.0, 0.5, 0.85), 0.38, (0.35, 0.85, 1.0), 2.0)
    light("POINT", (1.4, 2.0, 1.7), 7.0, (0.85, 0.72, 0.42), 0.65)
    light("POINT", (8.4, 4.9, 3.1), 15.0, (1.0, 0.55, 0.22), 0.85)
    light("POINT", (13.2, 9.6, 2.0), 5.0, (1.0, 0.66, 0.32), 0.7)
    light("POINT", (16.1, 15.0, 2.2), 12.0, (1.0, 0.58, 0.20), 0.9)
    light("POINT", (12.5, 15.0, 1.7), 3.5, (0.30, 0.80, 1.0), 0.7)

    # --- slab, floor tiles, cut edge and supports ------------------------
    box((cx, cy, -0.25), (width + 1.2, depth + 1.2, 0.5), M["copper"],
        bev=0.04, name="cut_slab")
    for ix in range(int(width)):
        for iy in range(int(depth)):
            px, py = x0 + 0.5 + ix, y0 + 0.5 + iy
            m = M["teal_m"] if (ix + iy) % 2 else M["teal_d"]
            if rnd.random() < 0.07:
                m = M["black"]
            elif rnd.random() < 0.06:
                m = M["teal_l"]
            box((px, py, 0.018), (0.96, 0.96, 0.036), m, bev=0.012, name="tile")
    # Cut edges end 1 mm inside the near walls that bury them.
    box((cx, y0 - 0.11, 0.025), (width + 1.198, 0.13, 0.05), M["brass_d"],
        bev=0.02, name="cut_edge")
    box((x1 + 0.11, cy, 0.025), (0.13, depth + 1.198, 0.05), M["brass_d"],
        bev=0.02, name="cut_edge")
    for px, py in ((-0.65, -0.65), (20.65, -0.65), (-0.65, 16.65),
                   (20.65, 16.65), (10.0, -0.65), (10.0, 16.65),
                   (-0.65, 8.0), (20.65, 8.0)):
        box((px, py, -1.7), (0.42, 0.42, 3.8), M["rust"], bev=0.04, name="support")

    # Flat thresholds and route furniture (nothing rises on walkable floor
    # except these flat plates).
    box((0.55, 2.0, 0.035), (1.1, 2.0, 0.07), M["teal_l"], bev=0.015,
        name="entrance_plate")
    # Thresholds sit 1 mm proud of the plate (CPU showed them) and stay black.
    for ty in (1.82, 2.18):
        box((0.55, ty, 0.056), (0.9, 0.10, 0.03), M["copper"], name="threshold")
        top_shadow(0.55, ty, 0.071, 0.9, 0.10)
    for px, py in ((3.8, 2.0), (7.0, 2.0), (10.5, 4.6), (10.5, 7.2),
                   (11.8, 9.9), (14.2, 11.3), (16.5, 9.9), (18.6, 6.2),
                   (18.6, 3.4), (15.2, 2.0), (12.0, 2.0)):
        box((px, py, 0.045), (0.20, 0.10, 0.02), M["brass_d"], name="route_tick")

    for px, py, pm in ((12.5, 15.55, M["teal_m"]), (16.1, 15.55, M["copper"])):
        box((px, py, 0.04), (1.25, 0.38, 0.025), pm, bev=0.012, name="door_plate")
    for i, b in enumerate(layout.blocks):
        if b.kind != "wall":
            continue
        size = (b.x1 - b.x0, b.y1 - b.y0, b.h)
        c = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0, b.h / 2.0)
        # The brass cap is the visible wall top. The body ends 35 mm below it,
        # inside the cap, so its top and flat sides never share a plane with the
        # cap's (Embree and OptiX break such ties differently) and the cap keeps
        # the exact silhouette the CPU bake had.
        tag(i, box((c[0], c[1], (b.h - 0.035) / 2.0),
                   (size[0], size[1], b.h - 0.035), M["teal_d"], bev=0.025,
                   name="wall"))
        tag(i, box((c[0], c[1], b.h - 0.035), (size[0], size[1], 0.07),
                   M["brass_d"], bev=0.012, name="wall_cap"))
        if b.h < 1.0 or i == 0:  # near-wall and entrance-wall tops render dark
            top_shadow(c[0], c[1], b.h, size[0] - 0.046, size[1] - 0.046)
        edge_band(i, b.x0, b.y0, b.x1, b.y1, b.h)
        dx, dy = size[0], size[1]
        if dx > dy and b.h > 1.0:  # far north wall: panel strip on -y face
            n = max(2, int(dx / 2.2))
            for k in range(n):
                px = b.x0 + (k + 0.5) * dx / n
                pw, ph = dx / n - 0.35, b.h - 0.75
                # The CPU showed some panels as darkened wall glass and hid the
                # rest; the shown ones stand 1 mm proud under a shadow-only sheet,
                # the others sit 1 mm inside the face.
                shown = (i, k) in ((2, 2), (2, 3), (2, 4), (3, 0), (3, 1), (4, 0))
                box_at(i, (px, b.y0 + (0.044 if shown else 0.046), b.h / 2.0),
                       (pw, 0.09, ph), M["gunmetal"], bev=0.015, name="wall_panel")
                if shown:
                    face_shadow((px, b.y0 - 0.006, b.h / 2.0), (pw, 0.002, ph))
        elif dy > dx and b.h > 1.0:  # west wall: panels on +x face
            n = max(2, int(dy / 2.4))
            for k in range(n):
                py = b.y0 + (k + 0.5) * dy / n
                if i == 0:  # the CPU bake showed these panels, pure black
                    box_at(i, (b.x1 - 0.044, py, b.h / 2.0),
                           (0.09, dy / n - 0.4, b.h - 0.75), dead,
                           bev=0.015, name="wall_panel")
                else:  # hidden in the wall face, as on the CPU
                    box_at(i, (b.x1 - 0.046, py, b.h / 2.0),
                           (0.09, dy / n - 0.4, b.h - 0.75), M["gunmetal"],
                           bev=0.015, name="wall_panel")

    # Entrance opening in the left wall (between y 1 and 3).
    box((-0.78, 2.0, 1.35), (0.28, 2.0, 2.7), M["black"], name="entrance_void")
    box_at(0, (-0.3, 1.07, 1.45), (0.66, 0.16, 2.9), M["brass_d"], bev=0.02,
           name="jamb")
    box_at(1, (-0.3, 2.93, 1.45), (0.66, 0.16, 2.9), M["brass_d"], bev=0.02,
           name="jamb")
    box_at(1, (-0.3, 2.0, 2.92), (0.66, 2.25, 0.38), M["teal_d"], bev=0.02,
           name="lintel")
    cyl((-0.62, 2.0, 1.35), 0.10, 0.08, M["gold_l"], rot=(0.0, math.pi / 2.0, 0.0),
        seg=12, smooth=False, name="entrance_lamp")

    # Round vault-door stop threshold (far-wall gap x 15.2..17) and the
    # shuttered side threshold (gap x 11.6..13.4). Both sit beyond y 16,
    # outside the walkable bounds.
    box((16.1, 16.42, 1.5), (1.78, 0.36, 3.0), M["black"], name="stop_recess")
    box((12.5, 16.42, 1.42), (1.72, 0.36, 2.84), M["black"], name="side_recess")
    # round steel vault door: leaf, rim, locking dogs and a spoked wheel
    zc, xc, yc = 1.55, 16.1, 16.20
    cyl((xc, yc, zc), 1.00, 0.20, M["steel"], rot=(math.pi / 2.0, 0.0, 0.0),
        seg=48, smooth=False, name="vault_leaf")
    torus((xc, yc - 0.13, zc), 0.98, 0.035, M["brass_d"],
          rot=(math.pi / 2.0, 0.0, 0.0), maj=48, mnr=8)
    cyl((xc, yc - 0.07, zc), 0.78, 0.12, M["teal_m"],
        rot=(math.pi / 2.0, 0.0, 0.0), seg=48, smooth=False, name="vault_inner")
    for k in range(8):
        a = k * math.tau / 8.0 + math.tau / 16.0
        dy, dz = math.cos(a), math.sin(a)
        px, pz = xc + 0.93 * dy, zc + 0.93 * dz
        box((px, yc - 0.12, pz), (0.11, 0.15, 0.27), M["gunmetal"],
            rot=(0.0, -a, 0.0), bev=0.018, name="locking_dog")
        cyl((px, yc - 0.21, pz), 0.050, 0.10, M["brass_d"],
            rot=(math.pi / 2.0, 0.0, 0.0), seg=8, smooth=False, name="dog_head")
    cyl((xc, yc - 0.18, zc), 0.14, 0.34, M["brass_d"],
        rot=(math.pi / 2.0, 0.0, 0.0), seg=16, smooth=False, name="wheel_hub")
    torus((xc, yc - 0.27, zc), 0.54, 0.048, M["brass_d"],
          rot=(math.pi / 2.0, 0.0, 0.0), maj=40, mnr=10)
    for k in range(5):
        a = k * math.tau / 5.0 + math.radians(10.0)
        beam((xc, yc - 0.27, zc),
             (xc + 0.54 * math.cos(a), yc - 0.27, zc + 0.54 * math.sin(a)),
             0.040, M["brass_d"], seg=8, name="wheel_spoke")
    cyl((xc, yc - 0.32, zc), 0.055, 0.04, M["cyan_l"],
        rot=(math.pi / 2.0, 0.0, 0.0), seg=12, smooth=False, name="sealed_lamp")
    # shuttered side door
    for k in range(7):
        pz = 0.22 + k * 0.39
        box((12.5, 16.10, pz), (1.50, 0.08, 0.24),
            M["brass_d"] if k % 2 else M["steel"], bev=0.015, name="shutter")
    box((12.5, 16.08, 1.42), (0.12, 0.10, 0.50), M["gunmetal"], name="latch")
    box((11.68, 16.30, 1.4), (0.14, 0.50, 2.80), M["teal_d"], bev=0.02, name="jamb")
    box((13.32, 16.30, 1.4), (0.14, 0.50, 2.80), M["teal_d"], bev=0.02, name="jamb")
    # brass pipework along the far wall, running past the doorway gaps
    for x0p, x1p, wi in ((-0.4, 11.6, 2), (13.4, 15.2, 3), (17.0, 20.5, 4)):
        beam((x0p, 16.28, 3.05), (x1p, 16.28, 3.05), 0.055, M["copper"], seg=8,
             name="wall_pipe")
        tag(wi, box((x1p, 16.28, 3.05), (0.16, 0.16, 0.16), M["brass_d"],
                    name="pipe_collar"))
    beam((-0.18, 3.0, 3.05), (-0.18, 16.6, 3.05), 0.05, M["steel"], seg=8,
         name="wall_pipe")

    # --- drawer-stack dressing ------------------------------------------
    def stack(i, cols=None, rows=3, south_rows=2, cut_top=False):
        b = blk(i)
        dx, dy = b.x1 - b.x0, b.y1 - b.y0
        c = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0, b.h / 2.0)
        body = M["teal_d"]
        if cut_top:
            body = M["steel"]
        # The body top sits 1 mm under the cornice top (which keeps the CPU
        # silhouette) so the two never share a plane.
        tag(i, box((c[0], c[1], (b.h - 0.001) / 2.0), (dx, dy, b.h - 0.001), body,
                   bev=0.025, name="stack_body"))
        tag(i, box((c[0], c[1], b.h - 0.03), (dx - 0.02, dy - 0.02, 0.06),
                   M["brass_d"], bev=0.012, name="stack_cornice"))
        top_shadow(c[0], c[1], b.h, dx - 0.046, dy - 0.046)  # tops render dark
        if not cut_top:  # the steel sectioned stacks show no band
            edge_band(i, b.x0, b.y0, b.x1, b.y1, b.h)
        cols = cols or max(3, min(6, int(dy / 0.9)))
        py, pz = dy / cols, b.h / (rows + 0.5)
        # Drawers stand 1 mm proud of the stack face and render pure black, as on
        # the CPU; the one sectioned stack at x 13.6 showed its body instead, so
        # its drawers sink 1 mm behind the face.
        lip = -0.036 if abs(b.x0 - 13.6) < 1e-6 else -0.0347
        for cc in range(cols):
            cy = b.y0 + (cc + 0.5) * py
            for rr in range(rows):
                cz = 0.20 + rr * pz
                box_at(i, (b.x1 + lip, cy, cz), (0.07, py - 0.06, pz - 0.07),
                       dead, bev=0.010, name="drawer")
                box_at(i, (b.x1 + 0.010, cy, cz), (0.035, py * 0.36, 0.075),
                       M["pearl"], name="label")
        if b.h > 2.0:
            sc_n = max(3, min(6, int(dx / 0.9)))
            sx = dx / sc_n
            for cc in range(sc_n):
                cx = b.x0 + (cc + 0.5) * sx
                for rr in range(south_rows):
                    cz = 0.35 + rr * pz
                    # the CPU showed the body, not these drawers, on the upper right
                    # of the 6.2 and 14.4 ranks
                    sink = (b.x0, b.y0) in ((0.0, 6.2), (0.0, 14.4)) and cc >= 4 and rr == 1
                    box_at(i, (cx, b.y0 + (0.036 if sink else 0.0347), cz),
                           (sx - 0.07, 0.07, pz - 0.08), dead, bev=0.010,
                           name="drawer")
                    box_at(i, (cx, b.y0 - 0.010, cz), (sx * 0.34, 0.035, 0.07),
                           M["brass_d"], name="pull")
        for k in range(3):  # copper ribs on the camera side
            t = 0.25 + 0.25 * k
            box_at(i, (b.x1 - 0.02, b.y0 + t * dy, b.h / 2.0),
                   (0.05, 0.10, b.h - 0.25), M["copper"], name="rib")
        if cut_top:
            tag(i, box((c[0], c[1], b.h + 0.005), (dx - 0.13, dy - 0.13, 0.045),
                       M["teal_m"], bev=0.012, name="cut_top"))
            letter_y = None  # box letters overlap their neighbours 5 cm in y
            for k in range(3):
                t = 0.25 + 0.25 * k
                if rnd.random() < 0.5:
                    tag(i, sph((c[0], b.y0 + t * dy, b.h + 0.075),
                               0.10, M["paper"], scale=(1.4, 1.3, 0.55), seg=12))
                    letter_y = None
                else:
                    ly = b.y0 + t * dy
                    box_at(i, (c[0], ly, b.h + 0.055),
                           (0.22, 0.30, 0.06), M["pearl"], bev=0.012, name="letter")
                    gap = 0.30 - 0.25 * dy  # overlap band; the CPU showed it black
                    if letter_y is not None and gap > 0.024:
                        top_shadow(c[0], (ly + letter_y) / 2.0, b.h + 0.085,
                                   0.196, gap - 0.024)
                    letter_y = ly

    # Long archive ranks (west and north), tall and enclosed.
    west = [find(lambda b: b.kind == "stack" and b.x0 == 0 and b.y0 == v)
            for v in (3.4, 6.2, 8.4, 11.8, 14.4)]
    north = [find(lambda b: b.kind == "stack" and b.x0 == v)
             for v in (6.0, 7.6)]
    for i in west + north:
        stack(i)
    # Sectioned, cut-down stacks whose tops show drawers and grown letters.
    low = [j for j, b in enumerate(layout.blocks)
           if b.kind == "stack" and b.h < 1.0]
    for i in low:
        stack(i, cols=4, rows=2, south_rows=1, cut_top=True)

    # --- railings and the chained register bay ---------------------------
    chain_idx = find(lambda b: b.kind == "railing" and b.x0 == 9.7)
    rail_idx = [chain_idx,
                find(lambda b: b.kind == "railing" and b.y0 == 10.8),
                find(lambda b: b.kind == "railing" and b.x0 == 12.2)]
    for i in rail_idx:
        b = blk(i)
        dx, dy = b.x1 - b.x0, b.y1 - b.y0
        c = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0, b.h / 2.0)
        # As on the walls, the body ends inside the cap so no faces are coplanar.
        tag(i, box((c[0], c[1], (b.h - 0.04) / 2.0), (dx, dy, b.h - 0.04),
                   M["steel"], bev=0.018, name="railing_body"))
        tag(i, box((c[0], c[1], b.h - 0.03), (dx, dy, 0.07), M["brass_d"],
                   bev=0.012, name="rail_cap"))
        edge_band(i, b.x0, b.y0, b.x1, b.y1, b.h + 0.005)
        long_x = dx >= dy
        spans = dx if long_x else dy
        n = max(2, int(spans / 0.8) + 1)
        for k in range(n):
            t = k / (n - 1) if n > 1 else 0.5
            px = b.x0 + t * dx if long_x else c[0]
            py = c[1] if long_x else b.y0 + t * dy
            box_at(i, (px, py, b.h / 2.0), (0.12, 0.12, b.h), M["copper"],
                   name="rail_post")
    # Chain slung across the register-bay railing.
    b = blk(chain_idx)
    ya, yb = b.y0 + 0.2, b.y1 - 0.2
    nlinks = 18
    for k in range(nlinks + 1):
        t = k / nlinks
        py = ya + t * (yb - ya)
        pz = b.h - 0.06 - 0.16 * math.sin(t * math.pi)
        tag(chain_idx, torus((b.x0 + 0.5 * (b.x1 - b.x0), py, pz), 0.052,
                             0.016, M["steel"],
                             rot=(0.0, math.pi / 2.0, 0.0), maj=10, mnr=6))
    box_at(chain_idx, (b.x0 + 0.1, ya, b.h / 2.0), (0.08, 0.08, b.h),
           M["copper"], name="post")
    box_at(chain_idx, (b.x0 + 0.1, yb, b.h / 2.0), (0.08, 0.08, b.h),
           M["copper"], name="post")

    # --- landmark: brass card-index drum (register wheel) ---------------
    i = find(lambda b: b.kind == "landmark")
    b = blk(i)
    dcx, dcy = (b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0
    c = (dcx, dcy, b.h / 2.0)
    tag(i, box((dcx, 6.76, b.h / 2.0), (b.x1 - b.x0, 2.48, b.h), M["teal_d"],
               bev=0.04, name="drum_housing"))
    tag(i, box((dcx, b.y0 + 0.08, 3.95), (b.x1 - b.x0 - 0.10, 0.14, 0.30),
               M["steel"], bev=0.015, name="wheel_hood"))
    # A-frame supports
    for px in (b.x0 + 0.42, b.x1 - 0.42):
        tag(i, box((px, b.y0 + 0.32, 0.85), (0.20, 0.24, 1.7), M["brass_d"],
                   bev=0.02, name="drum_leg"))
        tag(i, box((px, b.y0 + 0.32, 0.10), (0.24, 0.34, 0.20), M["gunmetal"],
                   bev=0.015, name="drum_foot"))
    tag(i, box((dcx, b.y1 - 0.33, 1.66), ((b.x1 - b.x0) - 0.65, 0.20, 0.14),
               M["copper"], name="drum_rail"))
    # central steel drum behind the wheel face
    tag(i, cyl((dcx, 6.85, 2.55), 0.82, 1.70, M["steel"],
               rot=(math.pi / 2.0, 0.0, 0.0), seg=32, smooth=False,
               name="index_drum"))
    # index wheel on the camera side of the housing
    wy, wz = b.y0 + 0.30, 2.62
    tag(i, cyl((dcx, wy, wz), 1.06, 0.22, M["teal_m"],
               rot=(math.pi / 2.0, 0.0, 0.0), seg=48, smooth=False,
               name="index_wheel"))
    tag(i, torus((dcx, wy - 0.10, wz), 1.04, 0.040, M["brass_d"],
                 rot=(math.pi / 2.0, 0.0, 0.0), maj=48, mnr=10))
    tag(i, torus((dcx, wy - 0.10, wz), 0.78, 0.026, M["copper"],
                 rot=(math.pi / 2.0, 0.0, 0.0), maj=48, mnr=8))
    nr = 14
    for k in range(3, nr + 3):
        a = k * math.tau / nr
        ox, oz = math.cos(a), math.sin(a)
        tag(i, box((dcx + 0.845 * ox, wy - 0.16, wz + 0.845 * oz),
                   (0.20, 0.045, 0.26), M["paper"], rot=(0.0, -a, 0.0),
                   bev=0.006, name="index_card"))
        tag(i, beam((dcx, wy - 0.14, wz),
                    (dcx + 0.86 * ox, wy - 0.14, wz + 0.86 * oz),
                    0.022, M["brass_d"], seg=6, name="card_spoke"))
    tag(i, cyl((dcx, wy - 0.16, wz), 0.17, 0.26, M["brass_d"],
               rot=(math.pi / 2.0, 0.0, 0.0), seg=20, smooth=False,
               name="drum_hub"))
    tag(i, cyl((dcx, wy - 0.30, wz), 0.055, 0.05, M["cyan_l"],
               rot=(math.pi / 2.0, 0.0, 0.0), seg=12, smooth=False,
               name="axle_lamp"))
    # pointer needle to the upper-right reading position
    tag(i, beam((dcx, wy - 0.30, wz),
                (dcx + 0.62, wy - 0.30, wz + 0.62), 0.032, M["pearl"], seg=8,
                name="pointer"))
    tag(i, sph((dcx + 0.68, wy - 0.30, wz + 0.68), 0.065, M["wax"], seg=12))
    # raised crown/index slot giving the register wheel its tall silhouette
    crown_y = b.y0 + 1.35
    tag(i, box((dcx, crown_y, 4.42), (0.44, 0.50, 0.95), M["steel"], bev=0.02,
               name="index_crown"))
    tag(i, box((dcx, crown_y - 0.26, 4.42), (0.28, 0.06, 0.70), M["dial"],
               name="crown_window"))
    tag(i, cyl((dcx, crown_y, 4.95), 0.10, 0.12, M["brass_d"], seg=12,
               smooth=False, name="crown_cap"))
