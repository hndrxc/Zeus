"""Postmaster's Office landmarks. The arena baker supplies primitives and lighting."""


def banker_lamp(M, x, y, z):
    """Arched enamel hood on a brass yoke; the lit opening faces the blotter."""
    cyl((x, y, z + 0.035), 0.15, 0.07, M["brass_d"], r2=0.12, seg=16, name="lamp_foot")
    beam((x, y, z + 0.07), (x, y, z + 0.38), 0.034, M["brass"], name="lamp_standard")
    for side in (-1, 1):
        beam((x, y, z + 0.37), (x + side * 0.32, y - 0.06, z + 0.52),
             0.026, M["brass"], name="lamp_yoke")

    # A half-barrel rather than a globe: straight lower lip and arched ends.
    bm = bmesh.new()
    ends = []
    for side in (-1, 1):
        end = []
        for i in range(13):
            angle = math.pi * i / 12
            end.append(bm.verts.new((side * 0.33, 0.16 * math.cos(angle), 0.17 * math.sin(angle))))
        ends.append(end)
    left, right = ends
    bm.faces.new(tuple(reversed(left)))
    bm.faces.new(right)
    for i in range(12):
        bm.faces.new((left[i], left[i + 1], right[i + 1], right[i])).smooth = True
    bm.faces.new((left[-1], left[0], right[0], right[-1]))
    new_obj(bm, "banker_hood", (x, y - 0.07, z + 0.50), (0, 0, 0), M["glass"])
    # The lit diffuser and the point light stay visible to the camera but not to
    # glossy rays: their mirror images in the clear-coated floor would read as a
    # hostile bolt in the boss chamber.
    box((x, y - 0.07, z + 0.493), (0.60, 0.27, 0.028), M["lamp"], bev=0.01, name="lamp_diffuser").visible_glossy = False
    light("POINT", (x, y - 0.16, z + 0.43), 12, (1.0, 0.71, 0.37), 0.15).visible_glossy = False


# The desk is a squat 0.8x so the crest sits low enough, and left of centre, to
# stay clear of the HUD cards that cover the top of the window (boss bar top
# centre, location and telemetry cards top right).
K = 0.8
TOP = 1.21 * K


def desk(M):
    # Twin stepped pedestals, separated by a real knee opening under the apron.
    for x in (-1.64, 2.28):
        box((x, 4.84, 0.055 * K), (1.28, 1.24, 0.23 * K), M["walnut"], bev=0.045, name="desk_plinth")
        box((x, 4.86, 0.55 * K), (1.14, 1.14, 0.78 * K), M["lacquer"], bev=0.045, name="desk_pedestal")
        for z in (0.36, 0.70):
            box((x, 4.275, z * K), (0.94, 0.065, 0.30 * K), M["veneer"], bev=0.035, name="drawer_front")
            box((x, 4.237, (z + 0.025) * K), (0.33, 0.05, 0.065), M["brass_d"], bev=0.024, name="drawer_pull")
    box((0.32, 5.20, 0.72 * K), (3.06, 0.17, 0.57 * K), M["lacquer"], bev=0.04, name="modesty_panel")
    box((0.32, 4.91, 0.96 * K), (5.14, 1.16, 0.19 * K), M["walnut"], bev=0.035, name="desk_apron")
    box((0.32, 4.88, 1.055 * K), (5.28, 1.25, 0.05 * K), M["brass_d"], bev=0.013, name="desk_stringing")
    box((0.32, 4.88, 1.14 * K), (5.34, 1.30, 0.14 * K), M["veneer"], bev=0.047, name="writing_top")
    box((0.16, 4.73, TOP + 0.008), (2.36, 0.78, 0.024), M["leather"], bev=0.01, name="writing_blotter")

    # Low shoulders leave the lamps free to break the skyline. The high bay sits
    # left of centre and carries the seal instead of repeating a wall of cubbies.
    for x0, x1, rise in ((-1.88, -1.40, 0.30), (-1.40, -0.25, 0.81),
                         (-0.25, 0.20, 0.50), (0.20, 0.62, 0.30)):
        top = TOP + rise
        box(((x0 + x1) / 2, 5.35, (TOP + top) / 2),
            (x1 - x0, 0.24, rise), M["lacquer"], name="stepped_escutcheon")
        box(((x0 + x1) / 2, 5.35, top + 0.035), (x1 - x0 + 0.05, 0.29, 0.07),
            M["veneer"], bev=0.02, name="escutcheon_coping")
    bay_top = TOP + 0.81 + 0.07
    ex = -0.825
    box((ex, 5.35, bay_top + 0.065), (0.95, 0.24, 0.13), M["walnut"], bev=0.035, name="crest_crown")
    box((ex, 5.35, bay_top + 0.165), (0.62, 0.21, 0.07), M["brass_d"], bev=0.02, name="crest_cap")

    # Brass letter with a dark chased fold, two ribbon ends and a red wax boss.
    y, ez = 5.17, 1.41
    box((ex, y + 0.035, ez), (1.0, 0.08, 0.58), M["brass_d"], bev=0.03, name="letter_reveal")
    box((ex, y - 0.012, ez), (0.92, 0.04, 0.49), M["brass"], bev=0.016, name="brass_envelope")
    for side in (-1, 1):
        beam((ex + side * 0.44, y - 0.04, ez + 0.225), (ex, y - 0.04, ez - 0.045),
             0.024, M["walnut"], seg=8, name="engraved_fold")
        box((ex + side * 0.055, y - 0.059, ez - 0.16), (0.08, 0.026, 0.21),
            M["wax_d"], rot=(0, -side * 0.30, 0), name="seal_ribbon")
    sph((ex, y - 0.081, ez - 0.045), 0.12, M["wax"], scale=(1.0, 0.40, 0.94), seg=16, name="wax_seal")

    for x in (-1.76, 2.52):
        banker_lamp(M, x, 4.78, TOP)
    # Bound dispatch ledger: just one bright fore-edge against the dark blotter.
    ledger_turn = (0, 0, -0.15)
    for dz, width, depth, height, material in ((0.039, 0.73, 0.50, 0.04, "lacquer"),
                                                (0.093, 0.69, 0.46, 0.07, "paper"),
                                                (0.149, 0.73, 0.50, 0.045, "lacquer")):
        box((-0.47, 4.70, TOP + dz), (width, depth, height), M[material], rot=ledger_turn, bev=0.012, name="ledger")


def visitors_chair(M):
    # A lone club chair across from the desk, facing in over the left rim.
    x, y = -4.86, -0.70
    for side in (-1, 1):
        box((x, y + side * 0.49, 0.105), (1.24, 0.15, 0.30), M["walnut"], bev=0.045, name="chair_sled")
    box((x, y, 0.33), (1.16, 1.29, 0.27), M["veneer"], bev=0.06, name="chair_frame")
    box((x + 0.07, y, 0.54), (0.94, 1.01, 0.24), M["leather"], bev=0.10, name="seat_cushion")
    rake = (0, -0.17, 0)
    box((x - 0.49, y, 1.01), (0.19, 1.27, 1.37), M["walnut"], rot=rake, bev=0.078, name="raked_chair_back")
    box((x - 0.37, y, 1.04), (0.23, 1.12, 1.14), M["leather"], rot=rake, bev=0.10, name="upholstered_back")
    for side in (-1, 1):
        ay = y + side * 0.55
        box((x, ay, 0.62), (1.02, 0.17, 0.42), M["walnut"], bev=0.055, name="arm_side")
        box((x + 0.02, ay, 0.81), (1.07, 0.25, 0.22), M["leather"], bev=0.095, name="rolled_arm")


def build(M):
    office = dict(M,
                  walnut=mat("of_walnut", (0.082, 0.032, 0.018), rough=0.44, coat=0.38),
                  veneer=mat("of_veneer", (0.24, 0.108, 0.048), rough=0.39, coat=0.48),
                  lacquer=mat("of_lacquer", (0.21, 0.045, 0.031), rough=0.33, coat=0.68),
                  leather=mat("of_leather", (0.036, 0.073, 0.053), rough=0.53, coat=0.18),
                  lamp=mat("of_opal", (0.79, 0.62, 0.36), rough=0.35, emit=(1.0, 0.72, 0.34), strength=1.4))
    desk(office)
    visitors_chair(office)
