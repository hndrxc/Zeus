"""Dead Letter Vault landmarks and lights. bake_assets.py injects its globals (math,
bpy and the primitive helpers); the shared floor and key suns come from the arena baker."""


def _vault_door(M):
    # Toe the pressure hatch toward the camera so the handwheel has open spaces
    # between its spokes. Depth runs toward the player; across runs up the wall.
    yaw = math.radians(-12)
    c, s = math.cos(yaw), math.sin(yaw)
    axis = (0, math.pi / 2, yaw)

    def p(depth, across=0, up=0):
        return (-5.14 + c * depth - s * across,
                -1.42 + s * depth + c * across, 1.58 + up)

    def disk(depth, radius, thickness, material, name, segments=64):
        cyl(p(depth), radius, thickness, M[material], rot=axis, seg=segments,
            smooth=False, name=name)

    # Stepped casting: the dark rebate remains visible between the machined
    # shoulder and the door leaf. No rectangular wall behind the crown.
    disk(-0.09, 1.70, 0.36, "teal_d", "vault_casting")
    disk(0.10, 1.62, 0.16, "steel", "vault_shoulder")
    disk(0.185, 1.46, 0.07, "black", "vault_rebate")
    disk(0.22, 1.34, 0.10, "teal_m", "vault_leaf")
    torus(p(0.265), 1.31, 0.033, M["dim_b"], rot=axis, maj=64, mnr=8)

    # Eight shot bolts visibly bridge the leaf and its seat. Leave wide gaps
    # between them: a crowded perimeter reads like a gear at gameplay scale.
    for i in range(8):
        a = (i + 0.5) * math.tau / 8
        dy, dz = math.cos(a), math.sin(a)
        box(p(0.285, 1.39 * dy, 1.39 * dz), (0.15, 0.42, 0.19), M["steel"],
            rot=(a, 0, yaw), bev=0.038, name="vault_shot_bolt")
        cyl(p(0.37, 1.51 * dy, 1.51 * dz), 0.079, 0.05, M["brass_d"],
            rot=axis, seg=6, smooth=False, name="vault_bolt_head")

    # Separate hinge straps on the near edge break the circular outline.
    for up in (-0.52, 0.52):
        box(p(0.26, -1.35, up), (0.18, 0.56, 0.19), M["steel"],
            rot=(0, 0, yaw), bev=0.035, name="vault_hinge_strap")
        cyl(p(0.26, -1.61, up), 0.14, 0.36, M["brass_d"], seg=20, name="vault_knuckle")
        cyl(p(0.26, -1.61, up + 0.195), 0.15, 0.055, M["steel"], seg=20,
            name="vault_hinge_cap")

    # A dished five-spoke wheel stands off the door on a separate spindle.
    # Bronze grips sit against a dark uninterrupted face, not another dial.
    disk(0.33, 0.30, 0.14, "black", "vault_gland", 32)
    disk(0.425, 0.16, 0.18, "steel", "vault_spindle", 24)
    for i in range(5):
        a = i * math.tau / 5 + math.radians(12)
        dy, dz = math.cos(a), math.sin(a)
        beam(p(0.48, 0.12 * dy, 0.12 * dz), p(0.59, 0.72 * dy, 0.72 * dz),
             0.06, M["brass_d"], seg=10, name="vault_wheel_spoke")
    torus(p(0.59), 0.72, 0.084, M["brass_d"], rot=axis, maj=48, mnr=10)
    disk(0.515, 0.19, 0.12, "steel", "vault_hub_nut", 6)
    disk(0.58, 0.095, 0.035, "cyan_l", "vault_sealed_indicator", 20)


def _deposit_bank(M, x, y, width, height, cols, rows):
    depth = 0.52
    face = x + depth / 2
    box((x, y, height / 2), (depth, width, height), M["teal_d"],
        bev=0.045, name="deposit_case")
    box((x, y, height), (depth + 0.06, width + 0.06, 0.09), M["steel"],
        bev=0.025, name="deposit_cornice")
    pitch_y, pitch_z = (width - 0.14) / cols, (height - 0.18) / rows
    for col in range(cols):
        cy = y + (col + 0.5 - cols / 2) * pitch_y
        for row in range(rows):
            cz = 0.09 + (row + 0.5) * pitch_z
            box((face, cy, cz), (0.08, pitch_y - 0.035, pitch_z - 0.035), M["steel"],
                bev=0.02, name="deposit_drawer")
            box((face + 0.048, cy, cz), (0.06, pitch_y * 0.43, 0.092), M["brass_d"],
                bev=0.018, name="deposit_card_pull")
            box((face + 0.08, cy, cz + 0.01), (0.012, pitch_y * 0.30, 0.038), M["paper"],
                name="deposit_index_card")


def build(M):
    _vault_door(M)
    _deposit_bank(M, -4.99, -3.75, 0.95, 0.85, 1, 2)
    _deposit_bank(M, -4.74, 1.25, 1.80, 1.35, 2, 3)
