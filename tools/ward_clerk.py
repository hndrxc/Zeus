"""Ward Clerk: a walking postal reliquary. +X is forward; dimensions are in baker units."""
import math


HIP_Z = .54
LINK = .24
ANKLE_Z = HIP_Z - 2 * LINK  # Straight, unposed linkage used to bind the three pivots.
AXLE = (math.pi / 2, 0, 0)
FACING = (0, math.pi / 2, 0)


def make_leg(pelvis, side, m):
    y = side * .125
    knee_z = HIP_Z - LINK
    label = 'Seal side' if side < 0 else 'Register side'
    hip = pivot(label + ' hip', (0, y, HIP_Z), pelvis)
    knee = pivot(label + ' knee', (0, y, knee_z), hip)
    ankle = pivot(label + ' ankle', (0, y, ANKLE_Z), knee)
    with attached(hip):
        cyl((0, y, HIP_Z), .083, .115, m['brass_d'], AXLE, name='Hip bearing')
        box((0, y, HIP_Z - LINK / 2), (.117, .109, LINK), m['gunmetal'],
            bev=.029, name='Thigh casing')
    with attached(knee):
        cyl((0, y, knee_z), .073, .146, m['steel'], AXLE, name='Knee knuckle')
        cyl((0, y + side * .074, knee_z), .044, .021, m['brass'], AXLE, name='Axle cap')
        beam((0, y, ANKLE_Z), (0, y, knee_z), .045, m['brass_d'], name='Shin piston')
        cyl((0, y, knee_z - .091), .064, .125, m['gunmetal'], name='Piston sleeve')
    with attached(ankle):
        box((.037, y, .04), (.259, .152, .080), m['black'], bev=.019, name='Flat boot sole')
        box((.061, y, .083), (.208, .152, .084), m['gunmetal'], bev=.028, name='Steel toe')
        cyl((0, y, ANKLE_Z + .035), .045, .144, m['brass_d'], AXLE, name='Boot pin')
    return hip, knee, ankle


def make_chassis(body, m):
    with attached(body):
        cyl((0, 0, .566), .155, .124, m['brass_d'], seg=12, name='Escapement housing')
        cyl((0, 0, .672), .123, .132, m['steel'], name='Waist swivel')
        box((.015, 0, .835), (.32, .405, .33), m['gunmetal'], bev=.053, name='Counting chest')
        box((.179, 0, .854), (.041, .267, .194), m['pearl'], bev=.018, name='Enamel docket')
        box((.198, 0, .895), (.020, .195, .038), m['black'], bev=.006, name='Postage slot')
        cyl((.212, .067, .80), .038, .018, m['brass_d'], FACING, name='Tariff stamp')
        cyl((.038, 0, 1.017), .097, .11, m['brass'], name='Collar bearing')
        # A projecting hood over a recessed amber reading slit, not a Censor screen.
        box((.072, 0, 1.12), (.254, .306, .172), m['gunmetal'], bev=.048, name='Reader head')
        box((.194, 0, 1.125), (.036, .231, .065), m['black'], bev=.013, name='Sight recess')
        box((.214, 0, 1.13), (.018, .164, .031), m['dial'], bev=.007, name='Reading slit')
        box((.107, 0, 1.202), (.309, .345, .058), m['pearl'], bev=.022, name='Porcelain visor')

    # The sloping register is legible from the rear: pale page block inside a dark
    # metal folio, a heavy brass clasp, and two broad thumb-index tabs on one side.
    rack = pivot('Tilted postal register', (-.167, .084, .725), body)
    with attached(rack):
        box((-.192, .084, .919), (.157, .306, .51), m['gunmetal'], bev=.031, name='Folio case')
        box((-.277, .084, .948), (.031, .248, .362), m['paper'], bev=.012, name='Address ledger')
        box((-.298, .084, .889), (.028, .287, .069), m['brass_d'], bev=.01, name='Ledger clasp')
        for z in (1.012, 1.098):
            box((-.242, .227, z), (.077, .081, .047), m['brass'], bev=.009, name='Index tab')
    rack.rotation_euler.y = -.29


def make_arms(body, m):
    arms = []
    for side in (-1, 1):
        y = side * .228
        arm = pivot('Reliquary arm' if side < 0 else 'Balance arm', (.016, y, .927), body)
        elbow = (.012, side * .304, .742)
        hand = (.155, -.317, .960) if side < 0 else (.121, .298, .624)
        with attached(arm):
            cyl((.016, y, .927), .089, .128, m['brass_d'], AXLE, name='Shoulder barrel')
            beam((.016, y, .927), elbow, .057, m['gunmetal'], name='Upper link')
            cyl(elbow, .064, .106, m['brass'], AXLE, name='Elbow ratchet')
            beam(elbow, hand, .043, m['steel'], name='Lower link')
            box(hand, (.108, .118, .093), m['gunmetal'], bev=.024, name='Gripper')
        arms.append(arm)
    return arms


def make_reliquary(arm, m):
    # One-handed processional seal. A bent support returns above the visor; no
    # long free tip or barrel. The open cage surrounds the light from every view.
    center = (.010, -.076, 1.345)
    x, y, z = center
    bottom = (x, y, z - .110)
    top = (x, y, z + .110)
    mount = pivot('Ward suspension', (.155, -.317, .96), arm)
    with attached(mount):
        hose([(.155, -.317, .923), (.104, -.324, 1.147), (.035, -.268, 1.216), bottom],
             .038, m['brass_d'], 'Bent carrying yoke')
        cyl(bottom, .074, .062, m['brass'], seg=8, name='Lower seal ferrule')
        for dx, dy in ((.190, 0), (0, .190), (-.190, 0), (0, -.190)):
            corner = (x + dx, y + dy, z)
            beam(bottom, corner, .018, m['brass_d'], name='Lower cage spar')
            beam(corner, top, .021, m['amethyst'], name='Violet enamel spar')
            sph(corner, .029, m['brass'], seg=12, name='Spar hinge')
        # Stacked square-cut lenses read as a split diamond, rather than a bulb.
        cyl((x, y, z - .033), .023, .064, m['amethyst'], r2=.112, seg=4,
            smooth=False, name='Lower seal crystal')
        cyl((x, y, z + .032), .115, .056, m['lilac'], r2=.025, seg=4,
            smooth=False, name='Luminous seal crystal')
        cyl(center, .064, .052, m['brass'], seg=8, name='Crystal lock ring')
        cyl(top, .052, .034, m['brass'], seg=8, name='Crown ferrule')
    return mount


def build_rig(m):
    root = pivot('Ward Clerk', (0, 0, 0))
    body = pivot('Chassis', (0, 0, 0), root)
    pelvis = pivot('Pelvis', (0, 0, 0), body)
    triples = [make_leg(pelvis, side, m) for side in (-1, 1)]
    make_chassis(body, m)
    arms = make_arms(body, m)
    make_reliquary(arms[0], m)
    return root, body, arms, [leg[0] for leg in triples], root, [leg[1] for leg in triples], [leg[2] for leg in triples]
