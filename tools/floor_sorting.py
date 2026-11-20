"""Public sorting hall. Run through bake_assets.py (its primitives use world units)."""
import math


class Bay:
    """A fixture's local x runs along its frontage; local -y faces the public."""
    def __init__(self, x, y, angle=0):
        self.x, self.y, self.angle = x, y, angle
        self.c, self.s = math.cos(angle), math.sin(angle)

    def p(self, u, v, z):
        return self.x + u*self.c - v*self.s, self.y + u*self.s + v*self.c, z

    def panel(self, u, v, z, w, d, h, material, bevel=0, name="Casting"):
        return box(self.p(u, v, z), (w, d, h), material, rot=(0, 0, self.angle),
                   bev=bevel, name=name)

    def rod(self, a, b, radius, material):
        return beam(self.p(*a), self.p(*b), radius, material)

    def word(self, u, v, z, words, size, material):
        ob = text(self.p(u, v, z), words, size, material, extrude=.001)
        ob.rotation_euler = (math.pi/2, 0, self.angle)
        ob.data.font = bpy.data.fonts.load(os.path.join(ROOT, 'tools', 'fonts', 'DejaVuSerif.ttf'), check_existing=True)
        return ob

    def disk(self, u, v, z, radius, depth, material, seg=36):
        return cyl(self.p(u, v, z), radius, depth, material, rot=(math.pi/2, 0, self.angle), seg=seg)

    def arch(self, u, v, spring, radius, gauge, material):
        points = [self.p(u + radius*math.cos(i*math.pi/32), v, spring + radius*math.sin(i*math.pi/32)) for i in range(33)]
        pipe_run(points, gauge, material, 'Rolled arch moulding')


def pipe_run(points, r, material, name):
    """Constant-radius sweep, including circular arcs without faceted elbows."""
    crv = bpy.data.curves.new(name, 'CURVE')
    crv.dimensions = '3D'
    crv.resolution_u = 1
    crv.bevel_depth, crv.bevel_resolution = r, 3
    crv.use_fill_caps = True
    line = crv.splines.new('POLY')
    line.points.add(len(points) - 1)
    for knot, p in zip(line.points, points):
        knot.co = (*p, 1)
    obj = bpy.data.objects.new(name, crv)
    bpy.context.collection.objects.link(obj)
    crv.materials.append(material)
    OBJS.append(obj)


def aligned(b):
    dx, dy = b.x1-b.x0, b.y1-b.y0
    return (Bay((b.x0+b.x1)/2, (b.y0+b.y1)/2, 0 if dx >= dy else math.pi/2),
            max(dx, dy), min(dx, dy))


def letter(f, u, v, z, m, angle=0, stack=.016):
    # Fold shadows, spine and the tiny oxblood seal make envelopes legible on a tray.
    x, y, _ = f.p(u, v, z)
    q = Bay(x, y, f.angle+angle)
    q.panel(0, 0, z+stack/2, .36, .245, stack, m['paper'], .007, 'Mail')
    for side in (-1, 1):
        q.rod((side*.17,.107,z+stack+.003),(0,-.023,z+stack+.003), .006, m['crease'])
    cyl(q.p(0,-.026,z+stack+.007), .029, .012, m['wax_d'], seg=12)


def water_and_section(m, layout, rnd):
    x0,y0,x1,y1 = layout.bounds
    # A section through the caisson: two steel flanges, copper facing plates,
    # web stiffeners and a foundation that disappears under dark water.
    walls = [b for b in layout.blocks if b.kind == 'wall']
    xa,xb = min(b.x0 for b in walls),max(b.x1 for b in walls)
    ya,yb = min(b.y0 for b in walls),max(b.y1 for b in walls)
    box(((xa+xb)/2,(ya+yb)/2,-.50),(xb-xa,yb-ya,.96),m['gunmetal'],bev=.065,name='Pressure hull')
    for f,L in ((Bay((xa+xb)/2,ya),xb-xa),(Bay(xb,(ya+yb)/2,math.pi/2),yb-ya)):
        for z in (-.11,-.86):
            f.panel(0,-.022,z,L,.095,.094,m['copper'],.02)
        n = math.ceil(L/1.7)
        for k in range(n):
            u = L*((k+.5)/n-.5)
            f.panel(u,-.039,-.46,L/n-.12,.061,.54,m['case'],.055)
            f.panel(u,-.080,-.45,L/n-.32,.025,.32,m['teal_d'],.035)
            f.panel(u-L/n/2+.035,-.075,-.49,.09,.12,.72,m['brass_d'],.016)
            if k%3==1:
                f.word(u,-.099,-.46,'DL · %02d'%(k+1),.21,m['address'])
        for k in range(math.ceil(L/3.6)):
            u = L*((k+.5)/math.ceil(L/3.6)-.5)
            f.panel(u,.13,-1.88,.35,.38,2.0,m['strut'],.04)
            f.panel(u,.13,-1.14,.49,.50,.14,m['rust'],.025)
            f.rod((u+.06,.14,-1.82),(u+.95,.14,-.9),.079,m['strut'])
    # Lit water directly below the hull; only emission reaches the margins.
    water = bpy.data.materials.new('Cold tidal basin')
    water.use_nodes = True
    nt = water.node_tree
    nt.nodes.clear()
    window = nt.nodes.new('ShaderNodeTexCoord')
    dist = nt.nodes.new('ShaderNodeVectorMath'); dist.operation = 'DISTANCE'
    dist.inputs[1].default_value = (.50,.47,0)
    nt.links.new(window.outputs['Window'],dist.inputs[0])
    ramp = nt.nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = .18
    ramp.color_ramp.elements[0].color = (.005,.026,.032,1)
    ramp.color_ramp.elements[1].position = .51
    ramp.color_ramp.elements[1].color = (.00182,.00478,.00605,1)
    nt.links.new(dist.outputs['Value'],ramp.inputs[0])
    em = nt.nodes.new('ShaderNodeEmission'); nt.links.new(ramp.outputs[0],em.inputs[0])
    out = nt.nodes.new('ShaderNodeOutputMaterial'); nt.links.new(em.outputs[0],out.inputs[0])
    box(((x0+x1)/2,(y0+y1)/2,-3.18),(100,100,.1),water,name='Tidal basin')
    for _ in range(70):
        x,y = rnd.uniform(x0-2,x1+4),rnd.uniform(y0-4,y1+2)
        if x > x1+.8 or y < y0-.8:
            sph((x,y,rnd.uniform(-3,-1.7)),rnd.uniform(.009,.024),m['patina'],seg=8)


def pavement(m, layout, rnd):
    x0,y0,x1,y1 = layout.bounds
    box(((x1+x0)/2,(y1+y0)/2,-.032),(x1-x0,y1-y0,.05),m['joint'],name='Copper grout')
    nx, ny = round((x1-x0)/1.30), round((y1-y0)/1.30)
    dx,dy = (x1-x0)/nx,(y1-y0)/ny
    for i in range(nx):
        for j in range(ny):
            x,y = x0+(i+.5)*dx,y0+(j+.5)*dy
            box((x,y,-.016),(dx-.021,dy-.021,.032),rnd.choice(m['tiles']),bev=.016,name='Glazed paver')
            if (i+2*j)%4==0:
                for s in (-1,1):
                    cyl((x+s*(dx*.5-.08),y+s*(dy*.5-.08),.003),.018,.007,m['joint'],seg=8)
    # Ceremonial brass inlay rather than stripes across the courier's feet.
    f = Bay(0,0)
    for y in (1.20,1.27,4.14):
        f.panel((x0+x1)/2,y,.008,x1-x0-.30,.019,.009,m['inlay'])
    for y in (5.99,8.04):
        f.panel(13.94,y,.008,7.04,.020,.009,m['inlay'])
    def cartouche(x,y,r,first,second):
        cyl((x,y,.010),r,.016,m['tiles'][0],seg=80)
        for rad in (r,r-.095):
            torus((x,y,.020),rad,.012,m['inlay'],maj=80,mnr=6)
        for k in range(16):
            a=k*math.pi/8
            box((x+(r-.18)*math.cos(a),y+(r-.18)*math.sin(a),.022),(.095 if k%4==0 else .048,.019,.008),m['inlay'],rot=(0,0,a))
        for z,textline,size in ((.11, first,.27),(-.26,second,.14)):
            u,v=(x+y)/math.sqrt(2),(-x+y)/math.sqrt(2)
            floor_type(u,v+z,textline,size,m['address'])
    cartouche(3.85,2.74,1.25,'Sorting Hall','UNDERTOW  POST')
    cartouche(18.72,7.63,1.22,'Dispatch','PUBLIC  CONCOURSE')
    for x,y in ((4.9,3.92),(8.7,1.47),(11.1,4.05),(16.96,5.87),(20.82,8.73),(15.82,6.09),(7.7,5.41)):
        letter(f,x,y,.008,m,rnd.uniform(-.6,.6))


def wall(m,b):
    f,L,D = aligned(b); h=b.h
    if h < 1:
        # Sawed-through enamel parapet, section cap with exposed brass channel.
        f.panel(0,0,h*.46,L,D,h*.92,m['case'],.035)
        f.panel(0,0,h-.055,L,D,.11,m['copper'],.018)
        f.panel(0,0,h+.005,L-.03,D-.15,.032,m['section'],.01)
        for v in (-D*.29,D*.29):
            f.panel(0,v,h+.024,L-.04,.027,.012,m['brass_d'])
        n=math.ceil(L/1.85)
        for i in range(n):
            u=((i+.5)/n-.5)*L
            f.panel(u,0,h+.026,.043,D-.05,.018,m['steel'])
        return
    # Layered cast-iron wall; long bays contain arched wired-glass clerestories.
    f.panel(0,.065,h/2,L,D-.13,h,m['case'],.04)
    for z,depth,thick,material in ((.21,D,.36,'gunmetal'),(.47,D,.075,'brass_d'),(2.73,D,.11,'copper'),(h-.18,D,.20,'case'),(h-.055,D,.09,'brass_d')):
        f.panel(0,0,z,L,depth,thick,m[material],.022)
    n=max(1,round(L/2.2)); pitch=L/n
    for i in range(n):
        u = (i+.5)*pitch-L/2; r=min(.65,pitch*.33)
        face=-D/2+.11
        f.panel(u,face,1.72,pitch-.20,.09,2.01,m['teal_d'],.042)
        f.disk(u,face-.059,1.87,r+.04,.05,m['brass_d'])
        f.disk(u,face-.091,1.87,r,.021,m['patina'])
        f.panel(u,face-.079,1.52,r*2,.06,.75,m['patina'])
        f.arch(u,face-.115,1.87,r,.033,m['brass_d'])
        for a in (0,math.pi/4,math.pi/2,3*math.pi/4):
            f.rod((u,face-.12,1.87),(u+r*math.cos(a),face-.12,1.87+r*math.sin(a)),.022,m['case'])
        for t in (-1,0,1):
            f.panel(u+t*r,face-.12,1.50,.051,.04,.78,m['case'])
        f.panel(u,face-.12,1.86,2*r,.04,.053,m['case'])
        f.panel(u,face-.11,1.10,2*r+.14,.11,.11,m['brass_d'],.018)
        f.panel(u,face,.79,pitch-.39,.10,.43,m['patina'],.035)
        for t in range(3):
            f.panel(u,face-.061,.69+t*.105,min(pitch*.48,.94),.018,.023,m['case'])
        f.panel(u,-D/2+.038,h-.46,pitch-.26,.07,.22,m['patina'],.04)
        if i==0 or i<n-1:
            a=u-pitch/2+.088
            f.panel(a,-D/2+.035,1.60,.149,.12,2.68,m['gunmetal'],.023)
            for z in (.40,2.68,3.13):
                f.panel(a,-D/2+.02,z,.198,.14,.101,m['brass_d'],.012)


def threshold(m,f,width,kind):
    # The jambs sit BEYOND the playable edge, even their bases. Real depth and
    # lit walls inside the stop opening distinguish a passage from a plaque.
    r=width/2-.10; spring=1.93
    dark=kind=='side'; metal=m['brass_d'] if dark else m['brass']
    f.panel(0,.61,-.023,width+.20,1.32,.06,m['gunmetal'])
    for s in (-1,1):
        u=s*(r+.08)
        f.panel(u,.37,spring*.5,.22,.66,spring,m['gunmetal'],.05)
        f.panel(u,.085,spring*.5,.12,.115,spring,m['copper'],.035)
        for z,d in ((.16,.25),(.39,.09),(1.82,.10),(spring,.16)):
            f.panel(u,.145,z,.25,.24,d,metal,.025)
        f.panel(u-s*.115,.115,1.25,.037,.073,1.01,m['dim_b'] if dark else m['lamp'])
    for depth,rad,gauge,ma in ((.39,r+.10,.18,m['gunmetal']),(.15,r+.09,.108,m['copper']),(.11,r,.049,metal)):
        f.arch(0,depth,spring,rad,gauge,ma)
    for i in range(1,8):
        a=i*math.pi/8
        f.rod((r*math.cos(a),.13,spring+r*math.sin(a)),((r+.19)*math.cos(a),.13,spring+(r+.19)*math.sin(a)),.035,metal)
    if dark:
        f.panel(0,.45,1.19,2*r,.2,2.38,m['case'],.04)
        f.disk(0,.41,spring,r,.11,m['case'])
        for i in range(13):
            z=.14+i*.19
            half=r if z<=spring else math.sqrt(max(0,r*r-(z-spring)**2))
            f.panel(0,.31,z,2*half,.12,.14,m['patina'],.018)
        f.panel(0,.228,1.11,.66,.072,.31,m['brass_d'],.047)
        f.word(0,.186,1.12,'II',.21,m['address'])
    else:
        for s in (-1,1):
            f.panel(s*(r+.11),1.52,1.46,.21,2.34,2.92,m['case'])
            for v in (.85,1.55,2.22):
                f.panel(s*(r-.017),v,1.42,.045,.13,2.63,m['brass_d'])
        f.panel(0,2.67,1.39,2*r+.2,.2,2.78,m['lamp'])
        f.panel(0,1.39,2.91,2*r+.3,2.62,.19,m['case'])
        if kind=='stop':
            # Conveyor disappearing into the next sorting chamber.
            for j in range(9):
                f.rod((-.54,.32+j*.24,.067),(.54,.32+j*.24,.067),.061,m['steel'])
            letter(f,.12,1.13,.135,m,.29)
            f.panel(0,2.48,.81,1.19,.18,1.05,m['gunmetal'],.06)
            f.panel(0,2.36,.81,.83,.055,.16,m['lamp'],.025)
            for s in (-1,1):
                for j in range(5):
                    f.panel(s*.67,-.10-j*.124,.020,.14,.069,.02,m['sulfur'] if j%2==0 else m['black'])
        else:
            for k in (-1,1):
                f.panel(k*.47,2.52,1.14,.75,.14,2.24,m['patina'],.05)
                f.panel(k*.47,2.41,1.47,.47,.045,.73,m['glass'],.05)
        light('POINT',f.p(0,.87,2.03),120 if kind=='stop' else 54,(1,.63,.29),.50)
    f.panel(0,-.23,.013,width-.06,.41,.019,m['joint'],.008)
    for i in (-1,0,1):
        f.panel(0,-.23+i*.116,.026,width-.18,.028,.008,metal)


def cordon(m,b):
    f,L,D=aligned(b); n=math.ceil(L/1.39); cap=min(D/2,.105)
    f.panel(0,0,.060,L,D,.12,m['gunmetal'],.025)
    for z,r in ((.59,.032),(b.h-.14,.044)):
        f.rod((-L/2+.04,0,z),(L/2-.04,0,z),r,m['copper'])
    pitch=(L-2*cap)/n
    for i in range(n+1):
        u=cap-L/2+i*pitch
        cyl(f.p(u,0,.13),cap,.18,m['brass_d'],seg=12)
        cyl(f.p(u,0,.51),.047,.71,m['brass_d'],r2=.037,seg=12)
        cyl(f.p(u,0,b.h-.144),.072,.10,m['case'],seg=12)
        sph(f.p(u,0,b.h-cap*.72),cap*.78,m['brass'],seg=12)
        if i<n:
            # Small pendant links between the rigid handrail and toe curb.
            for sign in (-1,1):
                f.rod((u+pitch*(.5+sign*.42),0,.53),(u+pitch*.5,0,.29),.019,m['gunmetal'])


def pigeonholes(m,b,rnd):
    f,L,D=aligned(b); h=b.h
    f.panel(0,D*.34,h*.50,L,D*.32,h-.08,m['case'],.045)
    f.panel(0,0,.20,L,D,.37,m['teal_d'],.035)
    n=max(1,round(L/.83)); w=(L-.13)/n; rows=5; dh=(h-.68)/rows
    for row in range(rows+1):
        z=.39+dh*row
        f.panel(0,0,z,L,D,.071,m['brass_d'],.012)
        f.panel(0,-D*.48,z+.021,L,.032,.066,m['copper'],.008)
    for j in range(n+1):
        u=-n*w/2+j*w
        f.panel(u,0,(h+.08)/2,.059,D,h-.41,m['case'],.013)
    for j in range(n):
        u=(j+.5-n/2)*w
        for k in range(rows):
            if rnd.random() < .44:
                # Recessed packets stand tilted inside deep slots; some slots empty.
                z=.45+k*dh; half=rnd.uniform(.14,.24)
                ob=f.panel(u+rnd.uniform(-.08,.08),-.026,z+half*.60,half*2,.055,half*1.30,rnd.choice((m['buff'],m['paper'],m['patina'])),.009,'Filed packet')
                ob.rotation_euler.y=rnd.uniform(-.14,.14)
            if k==2 and j%2==0:
                f.panel(u,-D/2-.009,.391+dh*k,.18,.022,.065,m['address'],.005)
    f.panel(0,0,h-.1,L,D,.20,m['case'],.035)
    for v in (-1,1):
        f.panel(0,v*D*.47,h-.12,L,.033,.06,m['brass_d'])
    for side in (-1,1):
        u=side*(L/2-.033)
        f.panel(u,0,h/2,.070,D,h-.025,m['patina'],.023)
    # Number on the end grain, visible looking along the public walkway.
    end = Bay(*f.p(-L/2,0,0)[:2], f.angle-math.pi/2)
    end.panel(0,-.012,2.23,D-.06,.038,.59,m['case'],.04)
    end.word(0,-.037,2.24,chr(65+round(b.x0/2.4)),.35,m['address'])
    for z in (.40,2.66,h-.16):
        end.panel(0,-.018,z,D-.048,.047,.057,m['brass_d'])


def sorting_bench(m,b,rnd):
    f,L,D=aligned(b); h=b.h
    for s in (-1,0,1):
        u=s*(L/2-.25)
        for t in (-1,1):
            f.panel(u,t*(D/2-.12),h*.39,.13,.13,h*.76,m['case'],.023)
        f.panel(u,0,.10,.32,D-.03,.14,m['brass_d'],.023)
    f.panel(0,0,.32,L-.4,.18,.11,m['copper'],.022)
    f.panel(0,0,h-.17,L,D,.16,m['brass_d'],.024)
    f.panel(0,0,h-.079,L-.045,D-.04,.075,m['teal_d'],.018)
    for i in range(6):
        u=(i-2.5)*L/6
        f.panel(u,0,h-.023,.77,.62,.038,m['gunmetal'],.022,'Sorting tray')
        for v in (-.30,.30):
            f.panel(u,v,h+.025,.77,.027,.096,m['copper'],.009)
        for s in (-1,1):
            f.panel(u+s*.375,0,h+.024,.027,.60,.095,m['brass_d'],.009)
        if i!=3:
            letter(f,u-.07,.021,h+.004,m,rnd.uniform(-.22,.22),stack=rnd.uniform(.035,.091))
        f.panel(u,-D/2-.012,h-.16,.23,.022,.071,m['address'],.012)


def counter(m,b,rnd):
    # The public screens are sectioned at the sill, not tall grilles hiding feet.
    f,L,D=aligned(b); h=b.h
    f.panel(0,0,(h-.13)/2,L-.06,D-.04,h-.13,m['case'],.043)
    f.panel(0,0,.07,L,D,.13,m['brass_d'],.026)
    f.panel(0,0,h-.12,L,D,.15,m['patina'],.035)
    f.panel(0,0,h-.035,L,D,.082,m['copper'],.025)
    f.panel(0,0,h+.014,L-.09,D-.09,.030,m['case'],.016)
    if D<1:
        n=3
        for i in range(n):
            u=((i+.5)/n-.5)*L
            f.panel(u,-.07,h+.037,L/n-.22,.34,.025,m['brass_d'],.013)
            for j in range(6):
                f.panel(u+(j-2.5)*.135,-.07,h+.055,.039,.26,.014,m['case'],.006)
            for v in (-1,1):
                f.panel(u,v*D*.486,h*.46,L/n-.12,.027,h*.43,m['teal_m'],.021)
        letter(f,L/2-.36,.19,h+.034,m,-.21)
    else:
        for k,(u,v) in enumerate(((-.56,-.25),(.32,.26),(.56,-.34))):
            f.panel(u,v,h+.079,.62,.41,.115,m['buff'] if k!=1 else m['paper'],.026,'Flat parcel')
            for du in (-.16,.16):
                f.panel(u+du,v,h+.139,.045,.4,.012,m['crease'])
        f.word(0,-D/2-.017,h*.50,'PARCELS',.20,m['address'])


def exchange(m,b):
    """A pipe-organ exchange: lobed pressure vessel, fluted risers, a low dial
    and three outward-facing bell mouths that survive play-size camera crops."""
    cx,cy=(b.x0+b.x1)/2,(b.y0+b.y1)/2
    f=Bay(cx,cy,math.pi/4); h=b.h
    # Square footprint and octagonal steps hold the circular manifold.
    box((cx,cy,.10),(b.x1-b.x0,b.y1-b.y0,.20),m['case'],bev=.12,name='Exchange foundation')
    for z,r,d,ma,r2 in ((.23,1.47,.14,'brass_d',1.47),(.37,1.39,.15,'patina',1.29),(.51,1.25,.16,'copper',1.25),(.82,1.19,.48,'case',1.12),(1.08,1.18,.10,'brass',1.18),(1.27,1.11,.29,'patina',.99),(1.49,1.02,.16,'copper',1.02)):
        cyl((cx,cy,z),r,d,m[ma],r2=r2,seg=12,smooth=False)
    # Three capsule loading throats with their sliding brass sleeves and one
    # ivory packet visible in a cradle. The centre outlet runs below the dial.
    for i,a in enumerate((-.70,0,.70)):
        q=Bay(cx,cy,math.pi/4+a)
        for v,z,r,d,ma in ((-1.01,.83,.31,.26,'copper'),(-1.18,.83,.32,.12,'brass'),(-1.251,.83,.247,.034,'gunmetal'),(-1.275,.83,.195,.023,'case')):
            q.disk(0,v,z,r,d,m[ma])
        if i!=0:
            q.disk(0,-1.30,.83,.135,.082,m['buff'] if i==2 else m['patina'])
            q.disk(0,-1.348,.83,.077,.028,m['brass_d'])
    # Wind chest and six individual trombone pipes, with varied crown heights.
    cyl((cx,cy,2.16),.62,1.29,m['gunmetal'],seg=32)
    for z in (1.56,2.66):
        cyl((cx,cy,z),.67,.12,m['brass_d'])
    for i,(u,v,top,r) in enumerate(((-.91,.02,3.64,.14),(-.60,.25,4.30,.18),(-.23,.39,h-.14,.20),(.20,.39,h-.56,.18),(.59,.23,h-.94,.16),(.92,.02,3.43,.14))):
        start=1.51
        cyl(f.p(u,v,(start+top)/2),r,top-start,m['copper'] if i%2==0 else m['brass_d'],seg=32)
        for z in (1.64,2.72,top-.29):
            cyl(f.p(u,v,z),r+.038,.085,m['brass'],seg=32)
        cyl(f.p(u,v,top-.085),r,.15,m['copper'],r2=r*1.36)
        cyl(f.p(u,v,top+.008),r*1.41,.044,m['brass'])
        cyl(f.p(u,v,top+.035),r*1.1,.013,m['case'])
        if i in (1,4):
            f.panel(u,v-r,2.18,.087,.031,.81,m['glass'],.013)
    # Manometer tilted only by the isometric camera; read from the concourse.
    for v,r,d,ma in ((-.60,.73,.38,'gunmetal'),(-.82,.74,.17,'copper'),(-.924,.65,.081,'brass'),(-.979,.574,.036,'dial')):
        f.disk(0,v,2.28,r,d,m[ma],seg=64)
    for i in range(13):
        a=math.pi*(1.18-i*1.36/12); r=.51
        f.rod((r*math.cos(a),-1.006,2.28+r*math.sin(a)),((r-(.095 if i%3==0 else .053))*math.cos(a),-1.006,2.28+(r-(.095 if i%3==0 else .053))*math.sin(a)),.013,m['case'])
    f.word(0,-1.009,2.08,'P · S · I',.092,m['case'])
    f.rod((-.116,-1.021,2.17),(.284,-1.021,2.58),.024,m['wax_d'])
    f.disk(0,-1.042,2.28,.065,.030,m['brass_d'])
    # Wrought yoke under the instrument; a maker's plate identifies the machine.
    for s in (-1,1):
        f.rod((s*.75,-.47,1.38),(s*.53,-.64,2.06),.071,m['brass'])
    f.panel(0,-1.04,1.405,.99,.12,.25,m['case'],.05)
    f.word(0,-1.109,1.41,'EXCHANGE',.14,m['address'])
    light('POINT',f.p(-1.50,-1.75,3.35),92,(1,.73,.38),.76)


def services(m,layout):
    # Receiver mains: vertical pipes return at the rear bulkhead, run behind it
    # and out of frame. No overhead line trespasses on a public route.
    x0,y0,x1,y1=layout.bounds
    for i,x in enumerate((layout.landmark[0]-.55,layout.landmark[0],layout.landmark[0]+.47)):
        y=y1+1.0+i*.38; z=3.85+i*.39; r=.14 if i!=1 else .20
        hose([(x,y1+.49,-.5),(x,y1+.5,z-.58),(x,y1+.65,z-.12),(x,y1+1.05,z),(x,y+10,z)],r,m['rust'] if i!=1 else m['copper'],'Exchange delivery trunk')
        for d in (1.64,3.80,6.3,8.45):
            cyl((x,y+d,z),r+.047,.104,m['brass_d'],rot=(math.pi/2,0,0))
    # A run of transom beams and distribution pipes behind the staff bays.
    for x in (1.5,3.9,6.3,8.7):
        f=Bay(x,y1+3.38,math.pi/2)
        f.panel(0,0,1.69,6.6,.42,3.31,m['strut'],.04)
        for z in (.42,1.21,1.98,2.77,3.30):
            f.panel(0,-.18,z,6.6,.19,.080,m['rust'],.016)
        for j in range(8):
            f.panel(-3.21+j*.87,-.17,1.75,.075,.2,2.99,m['case'])
    # Pinch the distant service runs into shadow rather than paving an extra hall.
    for d in (1.8,4.9):
        beam((-.65,y1+d,3.45),(9.1,y1+d,3.45),.079,m['strut'])


def build(M, layout, rnd):
    m = dict(M)
    for key,color,metal,rough in (
        ('case',(.018,.044,.051),.57,.34),('patina',(.037,.135,.127),.48,.43),
        ('section',(.22,.145,.081),.63,.48),('strut',(.022,.037,.041),.68,.52),
        ('joint',(.19,.092,.047),.67,.41),('inlay',(.36,.236,.101),.62,.41),
        ('address',(.65,.50,.29),.36,.45),('buff',(.37,.233,.126),.02,.73),
        ('crease',(.21,.132,.080),.08,.76)):
        m[key]=mat('Public hall '+key,color,metal,rough)
    m['lamp']=mat('Frosted amber milk glass',(.88,.68,.36),rough=.32,emit=(1,.71,.34),strength=2.6)
    m['tiles']=[mat('Sea glass %d'%i,(.016+k*.004,.073+k*.014,.083+k*.014),metal=.20,rough=.36,coat=.24) for i,k in enumerate((0,.5,1,1.6,2))]
    water_and_section(m,layout,rnd)
    pavement(m,layout,rnd)
    for b in layout.blocks:
        first=len(OBJS)
        if b.kind=='wall': wall(m,b)
        elif b.kind=='rack': pigeonholes(m,b,rnd)
        elif b.kind=='railing': cordon(m,b)
        elif b.kind=='landmark': exchange(m,b)
        elif b.kind=='counter': counter(m,b,rnd)
        elif b.kind=='table': sorting_bench(m,b,rnd)
        else: raise ValueError('Unexpected Sorting Hall fixture: '+b.kind)
        for obj in OBJS[first:]: obj['block']=b.index
    for kind,(x0,y0,x1,y1) in layout.doors.items():
        threshold(m,Bay((x0+x1)/2,layout.bounds[3]),x1-x0+.20,kind)
    # The entrance width is the actual cut between the two left-wall segments.
    x0,_,_,_=layout.bounds; ey=layout.entrance[1]
    left=[b for b in layout.blocks if b.kind=='wall' and b.x1<=x0]
    lo=max(b.y1 for b in left if b.y1<=ey); hi=min(b.y0 for b in left if b.y0>=ey)
    threshold(m,Bay(x0,(lo+hi)/2,math.pi/2),hi-lo,'entrance')
    services(m,layout)
    light('SUN',(-.60,-1,1.60),2.9,(1,.89,.74))
    light('SUN',(1,.45,1.0),1.42,(.47,.81,.96))
    for x,y,z,energy in ((2,3.3,2.4,50),(8.0,5.58,2.9,80),(15.9,4.99,2.10,78),(18.75,12.1,3.42,64)):
        light('POINT',(x,y,z),energy,(1,.67,.34),.87)
    world_color((.061,.096,.102),.88)
