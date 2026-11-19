"""Articulated pressure-suit courier. Uses primitives from bake_assets.py."""


TURN = math.tau
SIDE = (math.pi / 2, 0, 0)
FRONT = (0, math.pi / 2, 0)


def build():
    m = P()
    ivory, brass, bronze = m['pearl'], m['brass'], m['brass_d']
    ink, enamel, copper = m['black'], m['teal_l'], m['copper']
    root = pivot('Courier', (0, 0, 0))
    body = pivot('Breastplate', (0, 0, .60), root)
    head = pivot('Helmet swivel', (0, 0, 1.11), body)
    with attached(body):
        sph((0, 0, .85), .35, ivory, (.83, 1, 1.04), seg=40)
        for z, radius in ((.58,.25),(.62,.275),(1.09,.27)):
            torus((0,0,z),radius,.032,bronze)
        box((.265,0,.86),(.075,.40,.28),enamel,bev=.035,name='Enamel dispatch badge')
        for s in (-1,1):
            beam((.313,s*.18,.965),(.313,0,.855),.014,ivory)
            beam((.312,s*.18,.75),(.312,s*.05,.84),.008,brass)
        cyl((.328,0,.845),.049,.025,m['wax'],FRONT,seg=18,name='Seal')
        box((-.32,0,.89),(.23,.58,.56),m['rust'],bev=.048,name='Dispatch case')
        for s in (-1,1):
            box((-.405,s*.19,1.17),(.16,.10,.17),ivory,rot=(0,-.15,.1*s),bev=.012,name='Mail bundle')
            cyl((-.28,s*.285,1.03),.092,.52,copper,name='Pressure bottle')
            sph((-.28,s*.285,1.29),.092,copper,(1,1,.72))
            for z in (.83,1.09,1.24):
                torus((-.28,s*.285,z),.094,.014,bronze)
            hose([(-.28,s*.285,1.32),(-.32,s*.36,1.39),(-.21,s*.38,1.51),(-.09,s*.29,1.48)],.032,ink,'Air supply')
        box((-.455,0,.92),(.045,.52,.24),enamel,bev=.018,name='Case flap')
        for y in (-.17,.17):
            box((-.48,y,.88),(.025,.052,.43),bronze,bev=.008,name='Leather keeper')
            box((-.498,y,.94),(.022,.082,.068),brass,bev=.008,name='Latch')
    with attached(head):
        sph((0,0,1.38),.314,copper,(1,1,.97),seg=48)
        torus((0,0,1.24),.28,.024,bronze)
        # Raised porthole clears the curvature of the helmet.
        cyl((.235,0,1.40),.228,.17,bronze,FRONT,seg=48)
        sph((.323,0,1.4),.20,m['glass'],(.30,1,1),seg=40)
        torus((.333,0,1.40),.204,.022,brass,FRONT)
        for y in (-.09,0,.09):
            span=math.sqrt(.184**2-y*y)
            beam((.366,y,1.40-span),(.366,y,1.40+span),.012,bronze)
        beam((.371,-.19,1.4),(.371,.19,1.4),.012,bronze)
        for k in range(10):
            a=TURN*k/10
            cyl((.339,.219*math.cos(a),1.4+.219*math.sin(a)),.012,.018,ivory,FRONT,seg=6)
        for s in (-1,1):
            cyl((-.025,s*.29,1.40),.111,.082,bronze,SIDE)
            cyl((-.025,s*.343,1.40),.087,.022,ivory,SIDE)
            beam((-.025,s*.359,1.4),(.019,s*.359,1.453),.009,m['wax'])
            for k in range(7):
                a=math.pi*k/6
                sph((-.025+.071*math.cos(a),s*.360,1.40+.071*math.sin(a)),.008,ink,seg=8)
        sph((-.042,0,1.622),.22,enamel,(1.12,1,.43))
        cyl((.04,0,1.716),.074,.055,bronze)
        cyl((.084,0,1.759),.081,.132,bronze,FRONT)
        sph((.153,0,1.759),.067,m['gold_l'],(.44,1,1))
    legs, knees, arms, elbows = [], [], [], []
    for s in (-1,1):
        y=s*.164
        hip=pivot('Hip '+str(s),(0,y,.62),root)
        knee=pivot('Knee '+str(s),(.008,y,.32),hip)
        legs.append(hip)
        knees.append(knee)
        with attached(hip):
            sph((0,y,.47),.123,ivory,(.9,1,1.47))
            for z in (.39,.43):
                torus((0,y,z),.105,.015,bronze)
        with attached(knee):
            cyl((.008,y,.32),.086,.21,copper,SIDE)
            sph((.025,y,.226),.107,enamel,(1,1,1.22))
            box((.07,y,.103),(.35,.226,.18),ink,bev=.061,name='Diving boot')
            box((.07,y,.052),(.365,.235,.067),bronze,bev=.024,name='Weighted sole')
            sph((.151,y,.117),.11,copper,(1.1,1,.69))
        y=s*.35
        arm=pivot('Shoulder '+str(s),(.0,y,1.045),body)
        elbow=pivot('Elbow '+str(s),(.045,s*.411,.802),arm)
        arms.append(arm)
        elbows.append(elbow)
        with attached(arm):
            beam((0,y,1.04),(.045,s*.411,.802),.090,ivory)
            sph((0,y,1.057),.149,enamel,(1,1,.75))
            torus((0,y,1.039),.143,.014,brass)
            for x in (-.08,0,.08):
                sph((x,y+s*.104,1.085),.017,brass,seg=12)
        with attached(elbow):
            sph((.045,s*.411,.802),.084,copper)
            beam((.045,s*.411,.802),(.19,s*.435,.66),.084,bronze)
            sph((.182,s*.435,.68),.092,enamel,(1.16,1,.85))
            sph((.227,s*.435,.638),.081,ink)
            if s==1:
                cyl((.241,s*.483,.66),.06,.065,m['cyan_l'],SIDE,name='Seal projector')
            else:
                y=s*.435
                beam((.14,y,.56),(.348,y,.746),.036,ink,name='Opener grip')
                beam((.277,y,.812),(.409,y,.672),.022,brass,name='Quillon')
                verts=[(.32,y,.762),(.381,y,.697),(.55,y,.824),(.911,y,1.186),(.50,y,.926),(.50,y-.035,.87),(.50,y+.035,.87)]
                mesh=bpy.data.meshes.new('Faceted blade')
                faces=[(k,(k+1)%5,5) for k in range(5)]+[((k+1)%5,k,6) for k in range(5)]
                mesh.from_pydata(verts,[],faces)
                obj=bpy.data.objects.new('Gilt letter opener',mesh)
                bpy.context.collection.objects.link(obj)
                mesh.materials.append(ivory)
                mesh.materials.append(brass)
                for i in (0,1,2,5,6,7): mesh.polygons[i].material_index=1
                OBJS.append(obj)
    return root,body,head,legs,knees,arms,elbows


# Pose channels use degrees. Legs and arms rotate around their mechanical joints.
def animate(rig):
    root,body,head,legs,knees,arms,elbows=rig
    scene=bpy.context.scene
    def pose(frame, lean, twist, lift, hips, shins, shoulders, bends, sweep=0):
        body.location.z=.60+lift
        body.rotation_euler=(0,math.radians(lean),math.radians(twist))
        head.rotation_euler.z=math.radians(-twist*.55)
        body.keyframe_insert(data_path='location',frame=frame)
        for joints,angles in ((legs,hips),(knees,shins),(arms,shoulders),(elbows,bends)):
            for joint,angle in zip(joints,angles):
                joint.rotation_euler.y=math.radians(angle)
        arms[0].rotation_euler.z=math.radians(sweep)
        for joint in [body,head,*legs,*knees,*arms,*elbows]:
            joint.keyframe_insert(data_path='rotation_euler',frame=frame)
    rest=(0,0,0,(0,0),(0,0),(0,0),(0,0))
    pose(1,*rest)
    for k in range(9):
        phase=TURN*k/8
        stride=math.cos(phase)
        pose(5+2*k,9,8*stride,.032*math.sin(phase)**2,
             (-36*stride,36*stride),(9+35*max(0,math.sin(phase)),9+35*max(0,-math.sin(phase))),
             (24*stride,-28*stride),(-11,-17))
    for f,values in [
        (25,(-8,-29,-.025,(22,-17),(12,8),(36,-22),(-86,-32),-30)),
        (27,(17,14,-.05,(-34,22),(14,18),(-64,18),(-27,-22),-8)),
        (29,(20,39,-.045,(-39,27),(18,17),(-81,30),(8,-16),32)),
        (32,(8,18,-.02,(-17,15),(9,8),(-28,18),(-13,-8),17)),
        (35,rest),
        (39,(-9,21,-.024,(13,-17),(9,13),(14,-30),(-22,-74))),
        (41,(12,-19,-.038,(20,-30),(12,16),(23,-81),(-18,-15))),
        (44,(-5,-10,-.015,(13,-18),(6,8),(12,-59),(-14,-26))),
        (47,rest),
        (51,(28,4,-.10,(-48,46),(34,48),(48,-44),(-25,-38))),
        (54,(31,8,-.083,(-57,54),(23,54),(55,-54),(-18,-29))),
        (57,(18,3,-.045,(-32,34),(16,26),(32,-31),(-12,-17))),
        # Backhand return: the blade comes back across the chest, elbow leading.
        (61,(12,46,-.035,(-24,22),(12,19),(-82,34),(-16,-28),56)),
        (63,(14,-18,-.055,(26,-28),(18,12),(-54,25),(-40,-32),-42)),
        (65,(3,-51,-.022,(23,-19),(12,14),(-28,4),(-67,-35),-66)),
        (68,(2,-15,-.012,(9,-10),(8,7),(-9,5),(-26,-16),-22)),
        # Finisher: a high guard breaks into a deep, planted diagonal chop.
        (72,(-14,-35,.018,(14,-18),(7,12),(-135,-31),(-56,-62),-28)),
        (74,(27,8,-.084,(-43,37),(24,26),(-76,45),(-10,-28),8)),
        (77,(35,54,-.115,(-48,41),(34,32),(-9,53),(10,-16),61)),
        (81,(16,25,-.047,(-24,19),(13,16),(5,18),(-24,-18),33)),
        (85,rest),
    ]: pose(f,*values)
    for f,name in ((1,'Ready'),(5,'Run'),(25,'Slash'),(39,'Dispatch seal'),(51,'Dash'),(61,'Backhand'),(72,'Finishing chop')):
        scene.timeline_markers.new(name,frame=f)
    scene.frame_end=85
    scene.render.fps=30


def bake(engine):
    fresh()
    scene=setup(768,768,256/PX,144,True,engine,samples=12)
    scene.view_settings.view_transform='AgX'
    rig=build()
    animate(rig)
    lights_char()
    scene.frame_set(1)
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.region_3d.view_perspective='CAMERA'
            area.spaces.active.overlay.show_overlays=False
            area.spaces.active.shading.color_type='MATERIAL'
    for material in MATS.values():
        shader=next(n for n in material.node_tree.nodes if n.type=='BSDF_PRINCIPLED')
        material.diffuse_color=shader.inputs['Base Color'].default_value
    folder=os.path.join(OUT,'courier')
    os.makedirs(folder,exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT,'courier.blend'),compress=True)
    samples=[1,5,7,9,11,13,15,17,19,25,27,29,32,39,41,44,51,54,57,61,63,65,68,72,74,77,81]
    for direction, angle in enumerate(VIEW_ANGLES):
        rig[0].rotation_euler.z=math.radians(angle)
        for index,frame in enumerate(samples):
            scene.frame_set(frame)
            path=os.path.join(folder,f'{direction}_{index:02}.png')
            render(path)
            if direction==0 and index==0:
                import shutil
                shutil.copyfile(path,os.path.join(OUT,'courier.png'))
