"""Enemy rigging, mechanical fittings and directional animation bakes."""
from pathlib import Path
import shutil

FRONT = (0, math.pi / 2, 0)
SIDE = (math.pi / 2, 0, 0)
SIGNS = (-1, 1)


def brass_dial(x, y, z, radius, m):
    cyl((x, y, z), radius, .045, m["brass_d"], FRONT, seg=40)
    cyl((x+.026, y, z), radius*.84, .016, m["pearl"], FRONT, seg=40)
    for i in range(9):
        a = math.radians(-130 + 32.5*i)
        box((x+.04, y+radius*.65*math.sin(a), z+radius*.65*math.cos(a)),
            (.012,.012,radius*.21), m["black"], rot=(-a,0,0), name="Dial index")
    beam((x+.049,y,z),(x+.049,y-radius*.43,z+radius*.38),radius*.06,m["wax"])
    sph((x+.05,y,z),radius*.10,m["brass"])


def reel(x,y,z,r,m):
    cyl((x,y,z),r,.067,m["brass_d"],SIDE,seg=42)
    face=y+math.copysign(.04,y)
    cyl((x,face,z),r*.87,.022,m["paper"],SIDE,seg=42)
    torus((x,face+math.copysign(.018,y),z),r*.60,.012,m["copper"],SIDE)
    for k in range(5):
        a=k*math.tau/5
        cyl((x+r*.57*math.cos(a),face+math.copysign(.023,y),z+r*.57*math.sin(a)),
            r*.14,.012,m["black"],SIDE,seg=16)
    cyl((x,face+math.copysign(.028,y),z),r*.20,.048,m["brass"],SIDE)


def bailiff_details(body, arms, legs, m):
    with attached(body):
        # A segmented judge's ruff frames the face and the enamel breastplate.
        for i in range(18):
            a=i*math.tau/18
            box((.32*math.cos(a),.32*math.sin(a),1.13),
                (.17,.095,.064),m["pearl"],rot=(0,-.32,a),bev=.018,name="Ruff pleat")
        for s in SIGNS:
            hose([(.22,s*.27,1.11),(.39,s*.25,.95),(.455,s*.11,.81)],.021,m["brass"],"Chain of office")
        brass_dial(.468,0,.785,.118,m)
        box((-.435,0,.87),(.16,.48,.29),m["teal_d"],bev=.04,name="Warrant case")
        for y in (-.15,0,.15):
            cyl((-.49,y,1.04),.065,.27,m["paper"],rot=(0,-.19,0),name="Rolled warrant")
            torus((-.49,y,1.08),.068,.012,m["wax"])
        for i in range(8):
            a=i*math.tau/8
            box((.209*math.cos(a),.209*math.sin(a),1.37),(.035,.062,.095),
                m["brass"],rot=(0,0,a),bev=.01,name="Crown fluting")
    for s,arm,leg in zip(SIGNS,arms,legs):
        with attached(arm):
            sph((0,s*.5,.945),.168,m["teal_d"],(1,1,.75))
            torus((0,s*.5,.925),.16,.018,m["brass"])
            for x in (-.075,0,.075):
                sph((x,s*.63,.964),.016,m["pearl"])
            if s==1:
                box((.22,.64,1.085),(.425,.403,.088),m["brass_d"],bev=.032,name="Seal die")
                cyl((.22,.64,1.024),.188,.047,m["wax_d"],seg=8,name="Octagonal stamp pad")
                torus((.22,.64,.996),.133,.012,m["copper"])
        with attached(leg):
            for x in (-.086,.08,.178):
                cyl((x,s*.348,.12),.063,.025,m["copper"],SIDE)
                cyl((x,s*.366,.12),.025,.016,m["brass"],SIDE,seg=6)
            box((.068,s*.22,.236),(.41,.27,.056),m["teal_d"],bev=.025,name="Tread fender")


def censor_details(body,arms,legs,m):
    with attached(body):
        for s in SIGNS:
            reel(-.13,s*.286,1.17,.182,m)
            hose([(-.14,s*.28,1.37),(-.27,s*.26,1.4),(-.29,s*.21,.77)],
                 .026,m["copper"],"Signal conduit")
        box((-.235,0,1.02),(.09,.411,.537),m["brass_d"],bev=.03,name="Receiver casing")
        for k in range(7):
            box((-.288,0,.81+k*.062),(.035,.356,.023),m["steel"],bev=.009,name="Cooling fin")
        for i in range(2):
            for j in range(5):
                cyl((.263,-.159+j*.079,.91+i*.053),.026,.03,
                    m["pearl"] if j!=4 else m["wax"],FRONT,seg=12,name="Telegraph key")
        for k in range(4):
            box((.325,0,.57+.043*k),(.01,.115-.015*(k%2),.012),m["black"],name="Redaction")
    for s,arm,leg in zip(SIGNS,arms,legs):
        with attached(arm):
            beam((.12,s*.39,.74),(.35,s*.43,.68),.084,m["teal_l"],name="Transmitter stock")
            cyl((.37,s*.43,.69),.091,.27,m["copper"],FRONT)
            for i in range(4):
                torus((.29+i*.05,s*.43,.69),.093,.015,m["brass"],FRONT)
            cyl((.519,s*.43,.69),.09,.037,m["black"],FRONT)
            cyl((.545,s*.43,.69),.054,.015,m["cyan_l"],FRONT)
        with attached(leg):
            beam((-.047,s*.14,.49),(-.064,s*.14,.14),.036,m["copper"])
            cyl((.01,s*.14,.29),.079,.16,m["teal_l"])
            torus((.01,s*.14,.35),.078,.013,m["brass"])
            box((.105,s*.14,.079),(.197,.163,.065),m["steel"],bev=.021,name="Toe armor")


def post_details(body,arms,base,m):
    with attached(base):
        # Embroidered panels taper to the slope of the revolving robe.
        for i in range(12):
            a=i*math.tau/12
            for side in SIGNS:
                points=[]
                for z,offset in ((.19,0),(.50,side*.14),(.92,side*.055)):
                    r=.966-.36*z
                    points.append((r*math.cos(a+offset),r*math.sin(a+offset),z))
                hose(points,.021,m["brass"],"Robe embroidery")
            sph((.883*math.cos(a),.883*math.sin(a),.23),.041,m["pearl"])
    with attached(body):
        # A pipe-organ sorting rack gives the reverse view a distinct silhouette.
        for k in range(7):
            y=(k-3)*.13
            top=2.14-.092*abs(k-3)
            cyl((-.52,y,(1.08+top)/2),.067,top-1.08,m["copper"])
            torus((-.52,y,top),.07,.016,m["brass"])
            cyl((-.52,y,top+.006),.050,.009,m["black"])
        box((-.556,0,1.33),(.105,.99,.135),m["teal_d"],bev=.035,name="Manifold")
        for s in SIGNS:
            hose([(.32,s*.45,1.78),(.54,s*.39,1.71),(.56,s*.19,1.57)],
                 .025,m["pearl"],"Draped aiguillette")
            reel(-.062,s*.519,1.44,.17,m)
        brass_dial(.584,0,1.40,.162,m)
        for k in range(9):
            a=math.pi*(k-4)/8
            y,z=.47*math.sin(a),2.15+.47*math.cos(a)
            sph((-.032,y,z),.057,m["teal_l"],(.75,1,1.26))
    for s,arm in zip(SIGNS,arms):
        with attached(arm):
            sph((0,s*.62,1.84),.278,m["teal_d"],(1,1,.62))
            torus((0,s*.62,1.82),.274,.023,m["brass"])
            for k in range(5):
                x=(k-2)*.078
                beam((x,s*.826,1.82),(x,s*.856,1.635+.015*abs(k-2)),.022,m["brass"])
            x,y,z=(.30,.9,2.23) if s>0 else (.60,-.9,1.27)
            for side in SIGNS:
                box((x,y+side*.136,z),(.34,.036,.115),m["teal_l"],bev=.014,name="Stamp shoe")
            cyl((x,y,z-.105),.172,.055,m["wax_d"],seg=8,name="Cancellation die")


def build_rig(name,builder,m):
    builder(m)
    original=list(OBJS)
    boss=name=="postmaster"
    bailiff=name=="bailiff"
    root=pivot(name.title(),(0,0,0))
    body=pivot("Chassis",(0,0,1.05 if boss else .56 if bailiff else .59),root)
    base=pivot("Revolving hem",(0,0,0),root) if boss else root
    shoulder_y,shoulder_z=(.62,1.72) if boss else (.5,.9) if bailiff else (.25,.95)
    arms=[pivot("Shoulder "+str(s),(0,s*shoulder_y,shoulder_z),body) for s in SIGNS]
    legs=[] if boss else [pivot("Drive "+str(s),(0,s*(.22 if bailiff else .14),.41 if bailiff else .59),root) for s in SIGNS]
    for obj in original:
        x,y,z=obj.location
        side=int(y>0)
        joint=body
        if boss:
            if z<1.1: joint=base
            elif abs(y)>=.60: joint=arms[side]
        elif bailiff:
            if z<.43 and abs(y)<.36: joint=legs[side]
            elif abs(y)>=.49 and x>=-.01: joint=arms[side]
        else:
            if z<.56 and abs(y)<.20: joint=legs[side]
            elif .33<abs(y)<.47 and z<1.0: joint=arms[side]
        obj.parent=joint
        obj.location-=joint.matrix_world.translation
    if boss: post_details(body,arms,base,m)
    elif bailiff: bailiff_details(body,arms,legs,m)
    else: censor_details(body,arms,legs,m)
    return root,body,arms,legs,base

# For each attack, the two poses are the fully wound and released mechanisms.
# Channels: chassis pitch, chassis yaw, body lift, left/right shoulder pitch,
# left/right shoulder spread, drum yaw, and leg brace.
ATTACKS = {
    "bailiff":[("Warrant slam",(-14,-23,-.055,28,-58,-13,18,0,18),
                             (25,23,-.086,-36,76,12,-10,0,-34))],
    "censor":[("Transmit", (14,3,-.060,15,15,-8,8,0,17),
                          (-13,-3,.025,12,12,-5,5,0,-12))],
    "postmaster":[
        ("Circular dispatch",(-9,-24,-.025,-33,20,-31,24,-22,0),
                             (8,32,.065,-46,-60,38,-38,32,0)),
        ("Express charge",(-17,-17,-.057,48,-24,-10,10,-13,0),
                          (26,10,-.105,-55,46,15,-15,16,0)),
        ("Fan of letters",(1,28,-.03,12,-57,-12,26,19,0),
                          (-12,-28,.025,-78,34,26,-17,-23,0)),
        ("Summon clerks",(-13,3,.085,-110,-72,22,-22,-21,0),
                         (20,-8,-.105,39,68,-12,12,24,0)),
        ("Second shift", (15,-28,-.12,41,38,-10,10,-38,0),
                         (-14,31,.11,-89,-62,35,-39,47,0)),
    ],
}


def animate(name,rig,scene):
    if name == "ward_clerk":
        root, body, arms, legs, base, knees, ankles = rig
        origin = body.location.copy()
        samples = []
        def ward_pose(frame, swing, lift):
            body.location = origin + Vector((0, 0, lift))
            body.rotation_euler = (0, math.radians(2.0 + 1.5 * lift * 20.0),
                                   math.radians(-6.0 * swing))
            body.keyframe_insert(data_path="location", frame=frame)
            body.keyframe_insert(data_path="rotation_euler", frame=frame)
            # Seal-side arm holds the reliquary nearly still; the other arm
            # counterbalances the stride. Hip pitch is about Blender Y because
            # the Ward Clerk's legs are built downward from the pelvis.
            arm_pitch = (-6.0 * swing, 16.0 * swing)
            for j, arm in enumerate(arms):
                arm.rotation_euler = (math.radians(4.0 if j == 0 else -4.0),
                                      math.radians(arm_pitch[j]), 0)
                arm.keyframe_insert(data_path="rotation_euler", frame=frame)
            for j, (hip, knee, ankle) in enumerate(zip(legs, knees, ankles)):
                leg_swing = swing if j == 0 else -swing
                knee_angle = 4.0 + 24.0 * max(0.0, -leg_swing)
                hip.rotation_euler = (0, math.radians(-21.0 * leg_swing), 0)
                knee.rotation_euler = (0, math.radians(knee_angle), 0)
                ankle.rotation_euler = (0, math.radians(-0.7 * knee_angle), 0)
                hip.keyframe_insert(data_path="rotation_euler", frame=frame)
                knee.keyframe_insert(data_path="rotation_euler", frame=frame)
                ankle.keyframe_insert(data_path="rotation_euler", frame=frame)
        ward_pose(1, 0.0, 0.0)
        samples.append(1)
        scene.timeline_markers.new("Ready", frame=1)
        scene.timeline_markers.new("Ward stride", frame=5)
        for k in range(8):
            phase = math.tau * k / 8.0
            swing = math.cos(phase)
            lift = .008 * math.sin(phase) ** 2 - .012 if abs(swing) > .95 else .006 * math.sin(phase) ** 2
            frame = 5 + 2 * k
            ward_pose(frame, swing, lift)
            samples.append(frame)
        scene.frame_end = 20
        scene.render.fps = 30
        return samples
    root,body,arms,legs,base=rig
    origin=body.location.copy()
    samples=[]
    def pose(frame,channels,step=0):
        pitch,yaw,lift,arm_l,arm_r,spread_l,spread_r,hem,brace=channels
        body.location=origin+Vector((0,0,lift))
        body.rotation_euler=(0,math.radians(pitch),math.radians(yaw))
        body.keyframe_insert(data_path="location",frame=frame)
        joints=[body,*arms,*legs]
        for j,arm in enumerate(arms):
            arm.rotation_euler=(math.radians((spread_l,spread_r)[j]),
                                math.radians((arm_l,arm_r)[j]),0)
        for j,leg in enumerate(legs):
            leg.rotation_euler.y=math.radians((step+brace)*SIGNS[j])
        if base!=root:
            base.rotation_euler.z=math.radians(hem)
            joints.append(base)
        for joint in joints:
            joint.keyframe_insert(data_path="rotation_euler",frame=frame)

    neutral=(0,)*9
    pose(1,neutral)
    samples.append(1)
    scene.timeline_markers.new("Ready",frame=1)
    scene.timeline_markers.new("Advance",frame=5)
    for k in range(9):
        swing=math.cos(math.tau*k/8)
        bounce=math.sin(math.tau*k/8)**2
        f=5+k*2
        travel=22 if name=="bailiff" else 33
        pose(f,(3+3*bounce,7*swing,.033*bounce,19*swing,-21*swing,
                -3*bounce,3*bounce,13*swing,0),travel*swing)
        if k<8: samples.append(f)
    for i,(label,wind,release) in enumerate(ATTACKS[name]):
        start=25+24*i
        scene.timeline_markers.new(label,frame=start)
        for offset,channels in ((0,[v*.42 for v in wind]),(5,wind),(8,release),
                                (11,[v*.82 for v in release]),(16,[v*.36 for v in release])):
            frame=start+offset
            pose(frame,channels)
            samples.append(frame)
        pose(start+20,neutral)
    scene.frame_end=start+20
    scene.render.fps=30
    return samples


def bake(name,builder,engine):
    fresh()
    m=P()
    size=256*SS*ZOOM
    scene=setup(size,size,256/PX,size*48/256,True,engine,samples=16)
    scene.view_settings.view_transform="AgX"
    rig=builder(m) if name=="ward_clerk" else build_rig(name,builder,m)
    if name=="postmaster": rig[0].scale=(.8,.8,.8)
    samples=animate(name,rig,scene)
    lights_char()
    for area in bpy.context.screen.areas:
        if area.type=="VIEW_3D":
            area.spaces.active.region_3d.view_perspective="CAMERA"
            area.spaces.active.overlay.show_overlays=False
            area.spaces.active.shading.color_type="MATERIAL"
    for material in MATS.values():
        shader=next(n for n in material.node_tree.nodes if n.type=="BSDF_PRINCIPLED")
        material.diffuse_color=shader.inputs["Base Color"].default_value
    folder=Path(OUT if ZOOM==1 else PREVIEW)
    dest=folder/name
    dest.mkdir(parents=True,exist_ok=True)
    scene.frame_set(1)
    bpy.ops.wm.save_as_mainfile(filepath=str(folder/(name+".blend")),compress=True)
    for direction, angle in enumerate(VIEW_ANGLES):
        rig[0].rotation_euler.z=math.radians(angle)
        for i,frame in enumerate(samples):
            scene.frame_set(frame)
            path=dest/f"{direction}_{i:02}.png"
            render(str(path))
            if direction==0 and i==0: shutil.copyfile(path,folder/(name+".png"))
