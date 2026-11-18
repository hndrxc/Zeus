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


def desk(M):
    # Twin stepped pedestals, separated by a real knee opening under the apron.
    for x in (-1.64, 2.28):
        box((x, 4.84, 0.055), (1.28, 1.24, 0.23), M["walnut"], bev=0.045, name="desk_plinth")
        box((x, 4.86, 0.55), (1.14, 1.14, 0.78), M["lacquer"], bev=0.045, name="desk_pedestal")
        for z in (0.36, 0.70):
            box((x, 4.275, z), (0.94, 0.065, 0.30), M["veneer"], bev=0.035, name="drawer_front")
            box((x, 4.237, z + 0.025), (0.33, 0.05, 0.065), M["brass_d"], bev=0.024, name="drawer_pull")
    box((0.32, 5.20, 0.72), (3.06, 0.17, 0.57), M["lacquer"], bev=0.04, name="modesty_panel")
    box((0.32, 4.91, 0.96), (5.14, 1.16, 0.19), M["walnut"], bev=0.035, name="desk_apron")
    box((0.32, 4.88, 1.055), (5.28, 1.25, 0.05), M["brass_d"], bev=0.013, name="desk_stringing")
    box((0.32, 4.88, 1.14), (5.34, 1.30, 0.14), M["veneer"], bev=0.047, name="writing_top")
    box((0.16, 4.73, 1.218), (2.36, 0.78, 0.024), M["leather"], bev=0.01, name="writing_blotter")

    # Low shoulders leave the lamps free to break the skyline. The broad,
    # off-centre high bay carries the seal instead of repeating a wall of cubbies.
    for x0, x1, top in ((-0.84, -0.36, 1.65), (-0.36, 0.10, 1.98),
                        (0.10, 1.96, 2.54), (1.96, 2.29, 1.87)):
        box(((x0 + x1) / 2, 5.35, (1.21 + top) / 2),
            (x1 - x0, 0.24, top - 1.21), M["lacquer"], name="stepped_escutcheon")
        box(((x0 + x1) / 2, 5.35, top + 0.035), (x1 - x0 + 0.05, 0.29, 0.07),
            M["veneer"], bev=0.02, name="escutcheon_coping")
    box((1.03, 5.35, 2.675), (1.39, 0.24, 0.13), M["walnut"], bev=0.035, name="crest_crown")
    box((1.03, 5.35, 2.77), (0.87, 0.21, 0.07), M["brass_d"], bev=0.02, name="crest_cap")

    # Broad brass letter with a dark chased fold, two ribbon ends and a red wax boss.
    ex, y, ez = 1.03, 5.17, 2.03
    box((ex, y + 0.035, ez), (1.44, 0.08, 0.76), M["brass_d"], bev=0.035, name="letter_reveal")
    box((ex, y - 0.012, ez), (1.32, 0.04, 0.65), M["brass"], bev=0.018, name="brass_envelope")
    for side in (-1, 1):
        beam((ex + side * 0.63, y - 0.04, ez + 0.295), (ex, y - 0.04, ez - 0.06),
             0.028, M["walnut"], seg=8, name="engraved_fold")
        box((ex + side * 0.07, y - 0.059, ez - 0.21), (0.095, 0.026, 0.28),
            M["wax_d"], rot=(0, -side * 0.30, 0), name="seal_ribbon")
    sph((ex, y - 0.081, ez - 0.06), 0.15, M["wax"], scale=(1.0, 0.40, 0.94), seg=16, name="wax_seal")

    for x in (-1.76, 2.52):
        banker_lamp(M, x, 4.78, 1.21)
    # Bound dispatch ledger: just one bright fore-edge against the dark blotter.
    ledger_turn = (0, 0, -0.15)
    for z, width, depth, height, material in ((1.249, 0.73, 0.50, 0.04, "lacquer"),
                                               (1.303, 0.69, 0.46, 0.07, "paper"),
                                               (1.359, 0.73, 0.50, 0.045, "lacquer")):
        box((-0.47, 4.70, z), (width, depth, height), M[material], rot=ledger_turn, bev=0.012, name="ledger")


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
