//! Scene presentation: the Blender-baked arena, depth-sorted sprites, and
//! GPUI-painted telegraphs, shadows, particles and aiming aids.
//!
//! Design space is 900x600 and is fitted (uniformly, centred) into the given
//! viewport. `render` and `screen_to_world` share `Fit` so projection and its
//! inverse can never drift apart.

use std::f32::consts::{PI, TAU};
use std::sync::{Arc, LazyLock};

use gpui::prelude::*;
use gpui::{
    canvas, div, fill, img, point, px, rgb, size, Bounds, Div, Hsla, Image, ImageFormat, ObjectFit,
    PathBuilder, Pixels, Window,
};

use crate::engine::{
    EffectKind, Enemy, EnemyKind, Game, Phase, Vec2, ARENA, ATT_CHARGE, ATT_FAN, ATT_RING,
    ATT_SUMMON, SHOCKWAVE_BOLTS,
};
use crate::floor::{floor_for, Block, Floor};

const DESIGN_W: f32 = 900.0;
const DESIGN_H: f32 = 600.0;
/// Sprite sheets are 256x256 with the foot pixel at (128,176); one sprite
/// pixel equals one design pixel (62.225 px/unit / sqrt2 = 44 px per axis unit).
const SPRITE: f32 = 256.0;
const FOOT_X: f32 = 128.0;
const FOOT_Y: f32 = 176.0;

// Engine-derived geometry (mirrors private constants in engine.rs; boon-scaled
// reach, combo step and cooldown maxima are read live from the game).
const STRIKE_HALF_ARC: f32 = 1.047_197_6; // acos(0.5): chain strikes 1 and 2
const FINISHER_HALF_ARC: f32 = PI * 0.5; // acos(0.0): chain finisher
                                         // Attack footprints are built from these; the tests below check them against
                                         // the ticks on which the engine actually damages the courier.
const COURIER_RADIUS: f32 = 0.28;
/// Bailiff lunge: 9.5 u/s for 0.28 s, connecting within body + courier + 0.1.
const BAILIFF_TRAVEL: f32 = 9.5 * 0.28;
const BAILIFF_CONTACT: f32 = 0.38 + COURIER_RADIUS + 0.1;
/// Boss charge: 7.5 u/s for 0.55 s or until it meets a wall.
const BOSS_CHARGE_TRAVEL: f32 = 7.5 * 0.55;
const BOSS_CONTACT: f32 = 0.75 + COURIER_RADIUS + 0.1;
/// Hostile bolts hit within their own radius plus 85% of the courier's.
const VOLLEY_HIT: f32 = 0.17 + COURIER_RADIUS * 0.85;
const RING_HIT: f32 = 0.2 + COURIER_RADIUS * 0.85;
/// Bolts fly until they are this far from the centre on either axis.
const BOLT_LIMIT: f32 = ARENA + 0.6;
const CENSOR_SPREAD: f32 = 0.32;
const BOSS_FAN_SPREAD: f32 = 0.26;
/// Radius of the boss ring's reference circle: the ring closes in from here.
const RING_GUIDE: f32 = 3.2;

const PEARL: u32 = 0xe9e4d2;
const TEAL: u32 = 0x2fb3a2;
const TEAL_DEEP: u32 = 0x0b3d3b;
const WAX: u32 = 0xd2301f;
const BRASS: u32 = 0xe6c43a;
const INK: u32 = 0x060f12;
/// Ward tether and badge: violet, never wax-red (red means "attack").
const WARD: u32 = 0xb08cf0;
const WARD_DEEP: u32 = 0x3a1f78;

static ARENA_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena.png")));
static ARENA_VAULT_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena_vault.png")));
static ARENA_OFFICE_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena_office.png")));
// Construct GPUI images on demand as each pose is first used.
type SpriteFrame = LazyLock<Arc<Image>>;
macro_rules! sprite_frames {
    ($actor:literal, $direction:literal; $($frame:literal),+) => {
        [$(LazyLock::new(|| png(include_bytes!(concat!("../assets/", $actor, "/", $direction, "_", $frame, ".png"))))),+]
    };
}

macro_rules! actor_views {
    ($actor:literal; $($frame:literal),+) => {{
        macro_rules! view {
            ($dir:literal) => { sprite_frames!($actor, $dir; $($frame),+) };
        }
        [view!("0"), view!("1"), view!("2"), view!("3"),
         view!("4"), view!("5"), view!("6"), view!("7")]
    }};
}

static COURIER_FRAMES: [[SpriteFrame; 27]; 8] = actor_views!("courier";
    "00","01","02","03","04","05","06","07","08","09","10","11","12","13","14","15","16","17","18",
    "19","20","21","22","23","24","25","26");
static WARD_CLERK_FRAMES: [[SpriteFrame; 9]; 8] = actor_views!("ward_clerk";
    "00","01","02","03","04","05","06","07","08");
static BAILIFF_FRAMES: [[SpriteFrame; 14]; 8] = actor_views!("bailiff";
    "00","01","02","03","04","05","06","07","08","09","10","11","12","13");
static CENSOR_FRAMES: [[SpriteFrame; 14]; 8] = actor_views!("censor";
    "00","01","02","03","04","05","06","07","08","09","10","11","12","13");
static POSTMASTER_FRAMES: [[SpriteFrame; 34]; 8] = actor_views!("postmaster";
    "00","01","02","03","04","05","06","07","08","09","10","11","12","13","14","15","16","17","18",
    "19","20","21","22","23","24","25","26","27","28","29","30","31","32","33");

fn png(bytes: &'static [u8]) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec()))
}

/// Chamber background, matching `engine::chamber_name`: anything past the
/// Vault is the Postmaster's Office.
fn arena_image(chamber: usize) -> Arc<Image> {
    match chamber {
        1 => ARENA_IMG.clone(),
        2 => ARENA_VAULT_IMG.clone(),
        _ => ARENA_OFFICE_IMG.clone(),
    }
}
fn courier_image(player: &crate::engine::Player) -> Arc<Image> {
    const STRIKE_START: [usize; 3] = [9, 19, 23];
    let (facing, frame) = if let Some((dir, progress)) = player.dash_pose() {
        (dir, 16 + (progress * 3.0) as usize)
    } else if player.strike_windup {
        (player.facing, STRIKE_START[player.combo as usize])
    } else if let Some((kind, dir, progress)) = player.attack_pose {
        let frame = if let EffectKind::Strike(step) = kind {
            STRIKE_START[step as usize] + 1 + (progress * 3.0) as usize
        } else {
            13 + (progress * 3.0) as usize
        };
        (dir, frame)
    } else {
        let frame = if player.walking {
            1 + (player.stride_phase * 8.0) as usize
        } else {
            0
        };
        (player.facing, frame)
    };
    COURIER_FRAMES[view_direction(facing)][frame].clone()
}

fn view_direction(facing: Vec2) -> usize {
    // The baker keeps cardinal views in slots 0..4 and diagonals in 4..8.
    let octant = (facing.y.atan2(facing.x) / (PI / 4.0)).round() as i32;
    [0, 4, 1, 5, 2, 6, 3, 7][octant.rem_euclid(8) as usize]
}

fn col(hex: u32, a: f32) -> Hsla {
    let mut c: Hsla = rgb(hex).into();
    c.a = a;
    c
}

/// Uniform fit of the 900x600 design space into the viewport, with the
/// camera point `cam` (world) held at the centre of the design view.
#[derive(Clone, Copy)]
struct Fit {
    s: f32,
    ox: f32,
    oy: f32,
    cam: Vec2,
}

impl Fit {
    fn new(width: f32, height: f32, cam: Vec2) -> Self {
        let s = (width / DESIGN_W).min(height / DESIGN_H).max(0.0001);
        Fit {
            s,
            ox: (width - DESIGN_W * s) * 0.5,
            oy: (height - DESIGN_H * s) * 0.5,
            cam,
        }
    }

    /// World (x, y, height z) to viewport pixels.
    fn project(&self, x: f32, y: f32, z: f32) -> (f32, f32) {
        let (rx, ry) = (x - self.cam.x, y - self.cam.y);
        let dx = 450.0 + 44.0 * (rx + ry);
        let dy = 308.0 + 24.2 * (rx - ry) - 52.0 * z;
        (self.ox + dx * self.s, self.oy + dy * self.s)
    }

    fn unproject(&self, sx: f32, sy: f32) -> Vec2 {
        let dx = (sx - self.ox) / self.s - 450.0;
        let dy = (sy - self.oy) / self.s - 308.0;
        let u = dx / 44.0;
        let v = dy / 24.2;
        Vec2::new(self.cam.x + (u + v) * 0.5, self.cam.y + (u - v) * 0.5)
    }
}

/// Inverse of the ground projection (z = 0) for the same fit as `render`.
pub fn screen_to_world(x: f32, y: f32, width: f32, height: f32, camera: Vec2) -> Vec2 {
    Fit::new(width, height, camera).unproject(x, y)
}

// ------------------------------------------------------------------ halls ---

/// Opacity of the occluder pieces laid back over the courier.
const OCCLUDER_OPACITY: f32 = 0.45;

/// One occluder piece: the atlas rect `atlas` (x, y, w, h in atlas px) shows
/// the visible pixels of `block`, which sit at `dest` (design px) in the floor
/// image.
struct Piece {
    block: usize,
    atlas: (f32, f32, f32, f32),
    dest: (f32, f32, f32, f32),
}

/// A hall's baked background and occluder atlas.
struct HallArt {
    bg: Arc<Image>,
    atlas: Arc<Image>,
    atlas_size: (f32, f32),
    pieces: Vec<Piece>,
}

impl HallArt {
    fn new(bg: &'static [u8], atlas: &'static [u8], table: &str) -> Self {
        let mut atlas_size = (0.0, 0.0);
        let mut pieces = Vec::new();
        for line in table.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut words = line.split_whitespace();
            let head = words.next();
            let n: Vec<f32> = words
                .map(|w| {
                    w.parse()
                        .unwrap_or_else(|_| panic!("bad number {w:?} in .occ line {line:?}"))
                })
                .collect();
            match (head, n.as_slice()) {
                (Some("atlas"), &[w, h]) => atlas_size = (w, h),
                (Some("piece"), &[block, ax, ay, w, h, dx, dy, dw, dh]) => pieces.push(Piece {
                    block: block as usize,
                    atlas: (ax, ay, w, h),
                    dest: (dx, dy, dw, dh),
                }),
                _ => panic!("bad .occ line {line:?}"),
            }
        }
        HallArt {
            bg: png(bg),
            atlas: png(atlas),
            atlas_size,
            pieces,
        }
    }
}

static SORTING_HALL_ART: LazyLock<HallArt> = LazyLock::new(|| {
    HallArt::new(
        include_bytes!("../assets/floors/sorting_hall.png"),
        include_bytes!("../assets/floors/sorting_hall_occ.png"),
        include_str!("../assets/floors/sorting_hall.occ"),
    )
});
static VAULT_STACKS_ART: LazyLock<HallArt> = LazyLock::new(|| {
    HallArt::new(
        include_bytes!("../assets/floors/vault_stacks.png"),
        include_bytes!("../assets/floors/vault_stacks_occ.png"),
        include_str!("../assets/floors/vault_stacks.occ"),
    )
});

fn hall_art(id: &str) -> Option<&'static HallArt> {
    match id {
        "sorting_hall" => Some(&SORTING_HALL_ART),
        "vault_stacks" => Some(&VAULT_STACKS_ART),
        _ => None,
    }
}

/// Viewport rect (left, top, width, height) of the floor image: it is placed
/// so world (0, 0, 0) lands at the camera projection, the same `Fit` that
/// places every sprite, so image and projection cannot drift apart.
fn floor_rect(fit: &Fit, floor: &Floor) -> (f32, f32, f32, f32) {
    let (ox, oy) = fit.project(0.0, 0.0, 0.0);
    (
        ox - floor.origin.0 * fit.s,
        oy - floor.origin.1 * fit.s,
        floor.image_size.0 * fit.s,
        floor.image_size.1 * fit.s,
    )
}

/// Is `p` on the far side (toward -x or +y) of `b` as seen by the camera,
/// where the block hides the floor?
fn behind(p: Vec2, b: &Block) -> bool {
    (p.x < b.min.x || p.y > b.max.y) && !(p.x > b.max.x || p.y < b.min.y)
}

/// Translucent copies of every block the courier is behind (and whose pieces
/// overlap the courier sprite), absolutely placed over the floor image.
fn occluders(art: &HallArt, floor: &Floor, fit: &Fit, hero: Vec2) -> Vec<Div> {
    let (left, top, _, _) = floor_rect(fit, floor);
    let s = fit.s;
    let (hx, hy) = fit.project(hero.x, hero.y, 0.0);
    let (sx0, sy0) = (hx - FOOT_X * s, hy - FOOT_Y * s);
    let (sx1, sy1) = (sx0 + SPRITE * s, sy0 + SPRITE * s);
    let (aw, ah) = art.atlas_size;
    art.pieces
        .iter()
        .filter(|p| floor.blocks.get(p.block).is_some_and(|b| behind(hero, b)))
        .filter_map(|p| {
            let (dx, dy, dw, dh) = p.dest;
            let (l, t, w, h) = (left + dx * s, top + dy * s, dw * s, dh * s);
            if l >= sx1 || l + w <= sx0 || t >= sy1 || t + h <= sy0 {
                return None;
            }
            let (ax, ay, _, _) = p.atlas;
            Some(
                div()
                    .absolute()
                    .left(px(l))
                    .top(px(t))
                    .w(px(w))
                    .h(px(h))
                    .overflow_hidden()
                    .opacity(OCCLUDER_OPACITY)
                    .child(
                        img(art.atlas.clone())
                            .absolute()
                            .left(px(-ax * 0.5 * s))
                            .top(px(-ay * 0.5 * s))
                            .w(px(aw * 0.5 * s))
                            .h(px(ah * 0.5 * s))
                            .object_fit(ObjectFit::Fill),
                    ),
            )
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bailiff,
    Censor,
    Postmaster,
    WardClerk,
}

impl Kind {
    fn of(k: &EnemyKind) -> Kind {
        match k {
            EnemyKind::Bailiff => Kind::Bailiff,
            EnemyKind::Censor => Kind::Censor,
            EnemyKind::Postmaster => Kind::Postmaster,
            EnemyKind::WardClerk => Kind::WardClerk,
        }
    }
    fn shadow(self) -> f32 {
        match self {
            Kind::Bailiff => 0.52,
            Kind::Censor => 0.44,
            Kind::Postmaster => 1.0,
            Kind::WardClerk => 0.46,
        }
    }
    /// Approximate pixel height of the art above the foot (hp bar anchor).
    fn height(self) -> f32 {
        match self {
            Kind::Bailiff => 84.0,
            Kind::Censor => 78.0,
            Kind::Postmaster => 135.0,
            Kind::WardClerk => 80.0,
        }
    }
    fn image(self, facing: Vec2, frame: usize) -> Arc<Image> {
        let direction = view_direction(facing);
        match self {
            Kind::Bailiff => BAILIFF_FRAMES[direction][frame].clone(),
            Kind::Censor => CENSOR_FRAMES[direction][frame].clone(),
            Kind::WardClerk => WARD_CLERK_FRAMES[direction][frame].clone(),
            Kind::Postmaster => POSTMASTER_FRAMES[direction][frame].clone(),
        }
    }
}

#[derive(Clone, Copy)]
struct Foe {
    kind: Kind,
    frame: usize,
    pos: Vec2,
    dir: Vec2,
    hp: f32,
    max_hp: f32,
    windup: f32,
    flash: f32,
    attack: u8,
    phase2: bool,
    emerging: bool,
    radius: f32,
    /// Currently linked to a live ward source.
    warded: bool,
    /// Currently a live ward source.
    guarding: bool,
    ring_offset: f32,
    /// Progress of the half-hull stagger that ends in a shockwave ring.
    shock: f32,
}

fn foe(e: &Enemy) -> Foe {
    Foe {
        kind: Kind::of(&e.kind),
        frame: e.animation_frame(),
        pos: e.pos,
        dir: e.dir,
        hp: e.hp,
        max_hp: e.max_hp,
        windup: e.windup,
        flash: e.flash,
        attack: e.attack,
        phase2: e.phase2,
        emerging: e.emerging,
        radius: e.radius(),
        warded: false,
        guarding: false,
        ring_offset: e.ring_offset(),
        shock: e.shockwave(),
    }
}

/// A marked wave entry point.
#[derive(Clone, Copy)]
struct Mark {
    pos: Vec2,
    radius: f32,
    progress: f32,
}

#[derive(Clone, Copy)]
struct Bolt {
    pos: Vec2,
    radius: f32,
    hostile: bool,
    vel: Vec2,
}

#[derive(Clone, Copy)]
struct Fx {
    kind: EffectKind,
    pos: Vec2,
    dir: Vec2,
    age: f32,
    duration: f32,
    radius: f32,
}

#[derive(Clone, Copy)]
struct Hero {
    pos: Vec2,
    facing: Vec2,
    dash_cd: f32,
    dash_max: f32,
    cast_cd: f32,
    cast_max: f32,
    combo: u8,
    reach: f32,
    flash: f32,
    dashing: bool,
}

/// Everything the paint closures need, copied out of `Game` once per render.
struct Snapshot {
    fit: Fit,
    time: f32,
    hero: Hero,
    foes: Vec<Foe>,
    marks: Vec<Mark>,
    bolts: Vec<Bolt>,
    fx: Vec<Fx>,
    aim: Option<Vec2>,
    combat: bool,
    /// Walking a hall: no arena rim, water motes or combat cues.
    hall: bool,
    chamber: usize,
    /// (source, target) indices into `foes`, straight from `Game::wards`.
    wards: Vec<(usize, usize)>,
}

fn norm(v: Vec2, fallback: Vec2) -> Vec2 {
    let l = (v.x * v.x + v.y * v.y).sqrt();
    if l < 1e-4 {
        fallback
    } else {
        Vec2::new(v.x / l, v.y / l)
    }
}

fn hash01(n: u32) -> f32 {
    let mut h = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Thin wrapper over the window with the fit and viewport origin applied.
struct Pen<'a> {
    win: &'a mut Window,
    fit: Fit,
    org: (f32, f32),
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32, z: f32) -> gpui::Point<Pixels> {
        let (sx, sy) = self.fit.project(x, y, z);
        point(px(self.org.0 + sx), px(self.org.1 + sy))
    }

    fn path(
        &mut self,
        pts: &[(f32, f32, f32)],
        close: bool,
        fill_c: Option<Hsla>,
        stroke: Option<(f32, Hsla)>,
    ) {
        if pts.len() < 2 {
            return;
        }
        if let Some(c) = fill_c {
            let mut b = PathBuilder::fill();
            b.move_to(self.pt(pts[0].0, pts[0].1, pts[0].2));
            for p in &pts[1..] {
                b.line_to(self.pt(p.0, p.1, p.2));
            }
            b.close();
            if let Ok(p) = b.build() {
                self.win.paint_path(p, c);
            }
        }
        if let Some((w, c)) = stroke {
            let mut b = PathBuilder::stroke(px((w * self.fit.s).max(0.75)));
            b.move_to(self.pt(pts[0].0, pts[0].1, pts[0].2));
            for p in &pts[1..] {
                b.line_to(self.pt(p.0, p.1, p.2));
            }
            if close {
                b.close();
            }
            if let Ok(p) = b.build() {
                self.win.paint_path(p, c);
            }
        }
    }

    /// Ground arc/ring in world space; `wedge` closes it through the centre.
    #[allow(clippy::too_many_arguments)]
    fn arc(
        &mut self,
        c: Vec2,
        r0: f32,
        r1: f32,
        a0: f32,
        a1: f32,
        z: f32,
        fill_c: Option<Hsla>,
        stroke: Option<(f32, Hsla)>,
    ) {
        let n = (((a1 - a0).abs() / TAU) * 40.0).ceil().max(4.0) as usize;
        let mut pts: Vec<(f32, f32, f32)> = Vec::with_capacity(2 * n + 2);
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pts.push((c.x + r1 * a.cos(), c.y + r1 * a.sin(), z));
        }
        if r0 > 0.001 {
            for i in (0..=n).rev() {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                pts.push((c.x + r0 * a.cos(), c.y + r0 * a.sin(), z));
            }
        } else if (a1 - a0).abs() < TAU - 0.01 {
            pts.push((c.x, c.y, z));
        }
        let full = (a1 - a0).abs() >= TAU - 0.01 && r0 <= 0.001;
        // Open stroke for a bare arc, closed for rings/wedges.
        let closed = full || r0 > 0.001 || (a1 - a0).abs() < TAU - 0.01;
        self.path(&pts, closed, fill_c, stroke);
    }

    /// Open polyline arc stroke only (no closing chord).
    fn arc_line(&mut self, c: Vec2, r: f32, a0: f32, a1: f32, z: f32, w: f32, color: Hsla) {
        let n = (((a1 - a0).abs() / TAU) * 40.0).ceil().max(3.0) as usize;
        let pts: Vec<_> = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                (c.x + r * a.cos(), c.y + r * a.sin(), z)
            })
            .collect();
        self.path(&pts, false, None, Some((w, color)));
    }

    fn line(&mut self, a: (f32, f32, f32), b: (f32, f32, f32), w: f32, color: Hsla) {
        self.path(&[a, b], false, None, Some((w, color)));
    }

    /// Screen-space disc centred on a projected world point.
    fn disc(&mut self, x: f32, y: f32, z: f32, r_px: f32, color: Hsla) {
        let p = self.pt(x, y, z);
        let r = (r_px * self.fit.s).max(0.5);
        let b = Bounds::new(
            point(p.x - px(r), p.y - px(r)),
            size(px(2.0 * r), px(2.0 * r)),
        );
        self.win.paint_quad(fill(b, color).corner_radii(px(r)));
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: impl Into<gpui::Background>) {
        let b = Bounds::new(
            point(
                px(self.org.0 + self.fit.ox + x * self.fit.s),
                px(self.org.1 + self.fit.oy + y * self.fit.s),
            ),
            size(px(w * self.fit.s), px(h * self.fit.s)),
        );
        self.win.paint_quad(fill(b, color));
    }

    fn shadow(&mut self, p: Vec2, r: f32, strength: f32) {
        self.arc(
            p,
            0.0,
            r * 1.15,
            0.0,
            TAU,
            0.0,
            Some(col(INK, 0.20 * strength)),
            None,
        );
        self.arc(
            p,
            0.0,
            r * 0.85,
            0.0,
            TAU,
            0.0,
            Some(col(INK, 0.26 * strength)),
            None,
        );
        self.arc(
            p,
            0.0,
            r * 0.52,
            0.0,
            TAU,
            0.0,
            Some(col(INK, 0.30 * strength)),
            None,
        );
    }
}

fn paint_ground(snap: &Snapshot, b: Bounds<Pixels>, win: &mut Window) {
    let fit = snap.fit;
    let mut pen = Pen {
        win,
        fit,
        org: (f32::from(b.origin.x), f32::from(b.origin.y)),
    };
    let t = snap.time;
    let hero = snap.hero;

    // Soften the outer edges of the baked 900x600 plate into the abyss background
    // so full-screen viewports of any aspect ratio blend without a seam.
    let feather_x = 28.0;
    let feather_y = 18.0;
    let edge = gpui::linear_color_stop(col(INK, 1.0), 0.0);
    let clear = gpui::linear_color_stop(col(INK, 0.0), 1.0);
    if !snap.hall && fit.ox > 0.0 {
        pen.rect(
            0.0,
            0.0,
            feather_x,
            DESIGN_H,
            gpui::linear_gradient(90.0, edge, clear),
        );
        pen.rect(
            DESIGN_W - feather_x,
            0.0,
            feather_x,
            DESIGN_H,
            gpui::linear_gradient(270.0, edge, clear),
        );
    }
    if !snap.hall && fit.oy > 0.0 {
        pen.rect(
            0.0,
            0.0,
            DESIGN_W,
            feather_y,
            gpui::linear_gradient(180.0, edge, clear),
        );
        pen.rect(
            0.0,
            DESIGN_H - feather_y,
            DESIGN_W,
            feather_y,
            gpui::linear_gradient(0.0, edge, clear),
        );
    }

    // Contact shadows (the sprites carry none).
    pen.shadow(hero.pos, 0.46, 1.0);
    if hero.dashing {
        // Brass halo on the floor under the dashing courier.
        let pulse = 0.5 + 0.5 * (t * 30.0).sin();
        pen.arc(
            hero.pos,
            0.0,
            0.62,
            0.0,
            TAU,
            0.0,
            Some(col(BRASS, 0.14 + 0.08 * pulse)),
            Some((1.6, col(PEARL, 0.55))),
        );
    }
    for f in &snap.foes {
        pen.shadow(f.pos, f.kind.shadow(), 1.0);
    }

    // Wave entry points: brass postmarks (never wax-red, which means "attack")
    // that close in as the clerk is about to surface, then hold under it while
    // it emerges and cannot yet act.
    for m in &snap.marks {
        entry_mark(&mut pen, m.pos, m.radius, m.progress, t);
    }
    for f in snap.foes.iter().filter(|f| f.emerging) {
        entry_mark(&mut pen, f.pos, f.radius, 1.0, t);
    }

    // Ward tethers: a violet flow line on the floor, under every red warning.
    for &(s, d) in &snap.wards {
        ward_tether(&mut pen, &snap.foes[s], &snap.foes[d], t);
    }

    // Enemy telegraphs. Each attack family has its own shape and motion cue so
    // they stay apart without colour. All fills go down before any outline or
    // cue, so a crowded floor never buries one warning's edge under another's
    // wash; within a layer the warning closest to release paints last.
    let mut warned: Vec<(&Foe, Family)> = snap
        .foes
        .iter()
        .filter_map(|f| family(f, snap.chamber).map(|fam| (f, fam)))
        .collect();
    warned.sort_by(|a, b| urgency(a.0).total_cmp(&urgency(b.0)));
    for layer in [Layer::Fill, Layer::Cue] {
        for &(f, fam) in &warned {
            telegraph(&mut pen, f, fam, t, layer);
        }
    }

    // Ground effects: ripples, dash wakes, strike arcs.
    for e in &snap.fx {
        let u = (e.age / e.duration.max(1e-3)).clamp(0.0, 1.0);
        let fade = 1.0 - u;
        let d = norm(e.dir, Vec2::new(1.0, 0.0));
        let ang = d.y.atan2(d.x);
        match e.kind {
            EffectKind::Dash => {
                let len = e.radius.max(0.6) * 2.2;
                let tail = (
                    e.pos.x - d.x * len * (0.4 + 0.6 * u),
                    e.pos.y - d.y * len * (0.4 + 0.6 * u),
                    0.0,
                );
                pen.line(
                    (e.pos.x, e.pos.y, 0.0),
                    tail,
                    9.0 * fade,
                    col(TEAL, 0.28 * fade),
                );
                pen.line(
                    (e.pos.x, e.pos.y, 0.0),
                    tail,
                    2.5 * fade + 0.5,
                    col(PEARL, 0.55 * fade),
                );
                let r = e.radius.max(0.4) * (0.5 + ease_out(u));
                pen.arc(
                    e.pos,
                    0.0,
                    r,
                    0.0,
                    TAU,
                    0.0,
                    None,
                    Some((1.8 * fade + 0.4, col(TEAL, 0.6 * fade))),
                );
            }
            EffectKind::Burst => {
                let r = e.radius.max(0.5) * ease_out(u);
                pen.arc(
                    e.pos,
                    0.0,
                    r,
                    0.0,
                    TAU,
                    0.0,
                    Some(col(TEAL, 0.10 * fade)),
                    Some((3.0 * fade + 0.6, col(PEARL, 0.75 * fade))),
                );
                pen.arc(
                    e.pos,
                    0.0,
                    r * 0.72,
                    0.0,
                    TAU,
                    0.0,
                    None,
                    Some((1.6 * fade + 0.4, col(WAX, 0.55 * fade))),
                );
            }
            EffectKind::Hit => {
                let r = e.radius.max(0.3) * (0.4 + 0.9 * ease_out(u));
                pen.arc(
                    e.pos,
                    0.0,
                    r,
                    0.0,
                    TAU,
                    0.0,
                    None,
                    Some((1.8 * fade + 0.3, col(TEAL, 0.55 * fade))),
                );
            }
            EffectKind::Strike(step) => {
                // Crescent swept across the strike reach.
                let reach = e.radius.max(0.5);
                let half = if step == 2 {
                    FINISHER_HALF_ARC
                } else {
                    STRIKE_HALF_ARC
                };
                let sweep = half * (1.0 + 0.2 * ease_out(u));
                let head = ang - sweep + 2.0 * sweep * ease_out(u * 1.6);
                let tail = (head - 1.5).max(ang - sweep);
                let (tail, head) = if step == 1 {
                    (2.0 * ang - head, 2.0 * ang - tail)
                } else {
                    (tail, head)
                };
                pen.arc(
                    e.pos,
                    reach * 0.78,
                    reach,
                    tail,
                    head,
                    0.0,
                    Some(col(PEARL, 0.20 * fade)),
                    None,
                );
                pen.arc_line(
                    e.pos,
                    reach,
                    tail,
                    head,
                    0.0,
                    3.5 * fade + 0.6,
                    col(PEARL, 0.85 * fade),
                );
                pen.arc_line(
                    e.pos,
                    reach * 0.9,
                    tail,
                    head,
                    0.0,
                    6.0 * fade,
                    col(TEAL, 0.35 * fade),
                );
            }
        }
    }

    // Player range cues (only in combat, quiet so the arena stays dominant).
    // The strike cone mirrors engine::do_strike: combo step picks reach/arc,
    // Tidewater Reach scales it, and the cone follows the courier's facing.
    if snap.combat {
        let d = norm(hero.facing, Vec2::new(1.0, 0.0));
        let ang = d.y.atan2(d.x);
        let (base, half) = match hero.combo {
            0 => (1.55, STRIKE_HALF_ARC),
            1 => (1.6, STRIKE_HALF_ARC),
            _ => (1.95, FINISHER_HALF_ARC),
        };
        let reach = base * hero.reach;
        let cone = if hero.combo >= 2 {
            col(BRASS, 0.5)
        } else {
            col(PEARL, 0.4)
        };
        pen.arc_line(hero.pos, reach, ang - half, ang + half, 0.0, 1.4, cone);
        for s in [-half, half] {
            let (c, sn) = ((ang + s).cos(), (ang + s).sin());
            pen.line(
                (
                    hero.pos.x + c * (reach - 0.14),
                    hero.pos.y + sn * (reach - 0.14),
                    0.0,
                ),
                (
                    hero.pos.x + c * (reach + 0.1),
                    hero.pos.y + sn * (reach + 0.1),
                    0.0,
                ),
                1.4,
                cone,
            );
        }
        if let Some(a) = snap.aim {
            // Cast lane: dotted teal beyond melee reach, along the engine's cast direction.
            let dist = ((a.x - hero.pos.x).powi(2) + (a.y - hero.pos.y).powi(2)).sqrt();
            let ready = hero.cast_cd <= 0.0;
            let n = (dist / 0.34) as i32;
            for i in 0..n {
                let s = reach + 0.2 + i as f32 * 0.34 + (t * 0.8).fract() * 0.34;
                if s >= dist - 0.3 {
                    break;
                }
                pen.disc(
                    hero.pos.x + d.x * s,
                    hero.pos.y + d.y * s,
                    0.0,
                    1.5,
                    col(TEAL, if ready { 0.5 } else { 0.18 }),
                );
            }
            reticle(&mut pen, &hero, a, t);
        }
    }
    if snap.combat || snap.hall {
        dash_ring(&mut pen, &hero);
    }
}

/// Aim reticle at the aim point; its inner arc refills as the cast cooldown elapses.
fn reticle(pen: &mut Pen, hero: &Hero, a: Vec2, t: f32) {
    let ready = hero.cast_cd <= 0.0;
    let charge = if ready {
        1.0
    } else {
        1.0 - (hero.cast_cd / hero.cast_max.max(1e-3)).clamp(0.0, 1.0)
    };
    let alpha = if ready { 0.9 } else { 0.35 };
    let spin = t * 1.2;
    let rr = 0.3 + 0.03 * (t * 6.0).sin();
    pen.arc_line(a, rr, spin, spin + 1.2, 0.0, 1.6, col(PEARL, alpha));
    pen.arc_line(
        a,
        rr,
        spin + PI,
        spin + PI + 1.2,
        0.0,
        1.6,
        col(PEARL, alpha),
    );
    pen.arc_line(
        a,
        rr * 0.55,
        -PI * 0.5,
        -PI * 0.5 + TAU * charge.max(0.02),
        0.0,
        1.4,
        col(TEAL, alpha * 0.9),
    );
    pen.disc(a.x, a.y, 0.0, 1.6, col(WAX, alpha));
}

/// Dash recharge arc under the courier.
fn dash_ring(pen: &mut Pen, hero: &Hero) {
    if hero.dash_cd > 0.0 {
        let k = (hero.dash_cd / hero.dash_max.max(1e-3)).clamp(0.0, 1.0);
        pen.arc_line(
            hero.pos,
            0.62,
            -PI * 0.5,
            -PI * 0.5 + TAU * (1.0 - k),
            0.0,
            1.4,
            col(BRASS, 0.55),
        );
    } else {
        pen.arc_line(hero.pos, 0.62, 0.0, TAU, 0.0, 1.0, col(TEAL, 0.28));
    }
}

fn paint_front(snap: &Snapshot, b: Bounds<Pixels>, win: &mut Window) {
    let fit = snap.fit;
    let mut pen = Pen {
        win,
        fit,
        org: (f32::from(b.origin.x), f32::from(b.origin.y)),
    };
    let t = snap.time;

    // Rising motes in the petrol water: ambient only, very low contrast.
    // Halls have no water to rise from.
    let motes = if snap.hall { 0 } else { 16u32 };
    for i in 0..motes {
        let h1 = hash01(i * 3 + 1);
        let h2 = hash01(i * 3 + 2);
        let h3 = hash01(i * 3 + 3);
        let life = (t * (0.05 + 0.05 * h3) + h1).fract();
        let x = (h1 * 2.0 - 1.0) * ARENA + (t * 0.4 + h2 * 6.0).sin() * 0.15;
        let y = (h2 * 2.0 - 1.0) * ARENA;
        let z = life * 3.2;
        let a = (life * PI).sin() * 0.22;
        pen.disc(x, y, z, 1.0 + 1.5 * h3, col(PEARL, a));
    }

    // Hostile/friendly projectiles with ground shadow.
    for s in &snap.bolts {
        let z = 0.55;
        pen.arc(
            s.pos,
            0.0,
            s.radius.max(0.1) * 1.1,
            0.0,
            TAU,
            0.0,
            Some(col(INK, 0.35)),
            None,
        );
        let r_px = (s.radius.max(0.08) * 62.0).clamp(3.5, 16.0);
        let (outer, core) = if s.hostile {
            (WAX, PEARL)
        } else {
            (TEAL, PEARL)
        };
        // Short motion trail.
        let sp = (s.vel.x * s.vel.x + s.vel.y * s.vel.y).sqrt();
        if sp > 0.01 {
            let k = 0.07;
            pen.line(
                (s.pos.x, s.pos.y, z),
                (s.pos.x - s.vel.x * k, s.pos.y - s.vel.y * k, z),
                r_px * 1.2,
                col(outer, 0.35),
            );
        }
        pen.disc(s.pos.x, s.pos.y, z, r_px * 1.7, col(outer, 0.22));
        pen.disc(s.pos.x, s.pos.y, z, r_px, col(outer, 0.95));
        pen.disc(s.pos.x, s.pos.y, z, r_px * 0.45, col(core, 0.95));
    }

    // Particles from effects.
    for e in &snap.fx {
        let u = (e.age / e.duration.max(1e-3)).clamp(0.0, 1.0);
        let fade = 1.0 - u;
        let d = norm(e.dir, Vec2::new(1.0, 0.0));
        match e.kind {
            EffectKind::Hit => {
                for i in 0..9u32 {
                    let a = hash01(i + 17) * TAU;
                    let v = 0.6 + 1.4 * hash01(i + 41);
                    let r = v * ease_out(u) * 0.8;
                    let z = 0.55 + 0.9 * (u * (1.0 - u)) * 4.0 * hash01(i + 5) - 0.6 * u;
                    let c = if i % 3 == 0 { WAX } else { PEARL };
                    pen.disc(
                        e.pos.x + a.cos() * r,
                        e.pos.y + a.sin() * r,
                        z.max(0.0),
                        2.4 * fade + 0.4,
                        col(c, 0.95 * fade),
                    );
                }
            }
            EffectKind::Strike(_) => {
                for i in 0..6u32 {
                    let a = d.y.atan2(d.x) + (hash01(i + 90) - 0.5) * 1.8;
                    let r = e.radius.max(0.9) * (0.7 + 0.5 * hash01(i + 70)) * ease_out(u);
                    pen.disc(
                        e.pos.x + a.cos() * r,
                        e.pos.y + a.sin() * r,
                        0.5 + 0.4 * u,
                        1.8 * fade + 0.3,
                        col(PEARL, 0.8 * fade),
                    );
                }
            }
            EffectKind::Burst => {
                for i in 0..14u32 {
                    let a = i as f32 / 14.0 * TAU + hash01(i + 3) * 0.4;
                    let r = e.radius.max(0.5) * (0.5 + 0.55 * hash01(i + 9)) * ease_out(u);
                    let z = 0.2 + 1.3 * (1.0 - (2.0 * u - 1.0).powi(2)) * hash01(i + 21);
                    let c = if i % 2 == 0 { PEARL } else { TEAL };
                    pen.disc(
                        e.pos.x + a.cos() * r,
                        e.pos.y + a.sin() * r,
                        z,
                        2.6 * fade + 0.4,
                        col(c, 0.9 * fade),
                    );
                }
            }
            EffectKind::Dash => {
                for i in 0..5u32 {
                    let off = (i as f32 + 1.0) * 0.28 * ease_out(u);
                    let side = (hash01(i + 55) - 0.5) * 0.5;
                    pen.disc(
                        e.pos.x - d.x * off - d.y * side,
                        e.pos.y - d.y * off + d.x * side,
                        0.3 + 0.5 * u,
                        2.0 * fade + 0.3,
                        col(TEAL, 0.6 * fade),
                    );
                }
            }
        }
    }

    // Enemy health slips: ink plate, wax fill, pearl rim.
    for f in &snap.foes {
        if !(f.hp < f.max_hp || f.kind == Kind::Postmaster || f.warded) {
            continue;
        }
        let (fx, fy) = fit.project(f.pos.x, f.pos.y, 0.0);
        let design_x = (fx - fit.ox) / fit.s;
        let design_y = (fy - fit.oy) / fit.s - f.kind.height() - 8.0;
        let w = if f.kind == Kind::Postmaster {
            96.0
        } else {
            36.0
        };
        let h = if f.kind == Kind::Postmaster { 6.0 } else { 4.0 };
        let k = (f.hp / f.max_hp.max(1e-3)).clamp(0.0, 1.0);
        let (x0, y0) = (design_x - w * 0.5, design_y);
        let rim = if f.warded {
            col(WARD, 1.0)
        } else {
            col(PEARL, 0.55)
        };
        let rw = if f.warded { 1.6 } else { 1.0 };
        pen.rect(
            x0 - rw - 0.6,
            y0 - rw - 0.6,
            w + 2.0 * rw + 1.2,
            h + 2.0 * rw + 1.2,
            col(INK, 0.9),
        );
        pen.rect(x0 - rw, y0 - rw, w + 2.0 * rw, h + 2.0 * rw, rim);
        pen.rect(x0, y0, w, h, col(INK, 0.9));
        pen.rect(x0, y0, w * k, h, col(WAX, 1.0));
        if f.warded {
            // Shield glyph left of the bar; shape alone says "protected".
            let z = (f.kind.height() + 6.0) / 52.0;
            let c = (f.pos.x, f.pos.y, z);
            let g = |ox: f32, up: f32| ward_pt(c, -(w * 0.5) - 10.0 + ox, up);
            let shield = [
                g(-6.0, 5.0),
                g(6.0, 5.0),
                g(6.0, -1.0),
                g(0.0, -7.5),
                g(-6.0, -1.0),
            ];
            pen.path(&shield, true, None, Some((4.0, col(INK, 0.95))));
            pen.path(
                &shield,
                true,
                Some(col(WARD, 1.0)),
                Some((1.2, col(PEARL, 0.95))),
            );
            pen.path(
                &[g(0.0, 3.5), g(0.0, -4.0)],
                false,
                None,
                Some((1.4, col(INK, 0.9))),
            );
        }
    }

    // Telegraph glints above windup-ing enemies (a brass warning pip).
    for f in &snap.foes {
        let w = f.windup.max(f.shock).clamp(0.0, 1.0);
        if w > 0.0 {
            let blink = if (t * 18.0).sin() > 0.0 { 1.0 } else { 0.55 };
            let z = f.kind.height() / 52.0 + 0.25;
            pen.disc(f.pos.x, f.pos.y, z, 3.0 + 3.0 * w, col(BRASS, 0.9 * blink));
            pen.disc(f.pos.x, f.pos.y, z, 1.4, col(INK, 0.9));
        }
    }

    // Ward source badge: a diamond over every surfaced clerk, hollow while it
    // guards nobody, filled with a core while the link is live.
    for f in snap.foes.iter() {
        if f.kind != Kind::WardClerk || f.emerging {
            continue;
        }
        let active = f.guarding;
        let pulse = 0.5 + 0.5 * (t * 4.0).sin();
        let c = (f.pos.x, f.pos.y, (f.kind.height() + 20.0) / 52.0);
        let d = |ox: f32, up: f32| ward_pt(c, ox, up);
        let (r, hgt) = if active { (8.0, 10.0) } else { (7.0, 9.0) };
        let dia = [d(0.0, hgt), d(r, 0.0), d(0.0, -hgt), d(-r, 0.0)];
        pen.path(&dia, true, None, Some((4.5, col(INK, 0.95))));
        if active {
            pen.path(
                &dia,
                true,
                Some(col(WARD, 0.85 + 0.15 * pulse)),
                Some((1.4, col(PEARL, 1.0))),
            );
            pen.path(
                &[d(0.0, 4.0), d(3.0, 0.0), d(0.0, -4.0), d(-3.0, 0.0)],
                true,
                Some(col(PEARL, 1.0)),
                None,
            );
        } else {
            pen.path(
                &dia,
                true,
                Some(col(WARD_DEEP, 0.9)),
                Some((1.4, col(WARD, 0.95))),
            );
        }
    }

    // Hall floor target: always on top of the scenery and its occluders.
    if snap.hall {
        if let Some(a) = snap.aim {
            reticle(&mut pen, &snap.hero, a, t);
        }
    }
}

fn sprite(
    fit: Fit,
    image: Arc<Image>,
    pos: Vec2,
    bob: f32,
    shake: f32,
    stretch: f32,
    gray: bool,
    alpha: f32,
) -> gpui::Img {
    let (fx, fy) = fit.project(pos.x, pos.y, 0.0);
    let s = fit.s;
    let w = SPRITE * s * (1.0 - 0.5 * stretch);
    let h = SPRITE * s * (1.0 + stretch);
    // Anchor the foot pixel, scaling the sheet about it.
    let left = fx - FOOT_X * s * (w / (SPRITE * s)) + shake * s;
    let top = fy - FOOT_Y * s * (h / (SPRITE * s)) - bob * s;
    img(image)
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(w))
        .h(px(h))
        .object_fit(ObjectFit::Fill)
        .grayscale(gray)
        .opacity(alpha)
}

/// Render the world into a viewport of `width` x `height`; `aim` is a world point.
pub fn render(world: &Game, aim: Option<Vec2>, width: f32, height: f32) -> Div {
    let fit = Fit::new(width, height, world.camera);
    let time = world.time;
    let hero = Hero {
        pos: world.player.pos,
        facing: world.player.facing,
        dashing: world.player.dashing(),
        dash_cd: world.player.dash_cooldown,
        dash_max: world.player.dash_cooldown_max,
        cast_cd: world.player.cast_cooldown,
        cast_max: world.player.cast_cooldown_max,
        combo: world.player.combo,
        reach: world.mods.reach,
        flash: world.player.flash,
    };
    let mut foes: Vec<Foe> = world.enemies.iter().map(foe).collect();
    let wards: Vec<(usize, usize)> = world.wards().collect();
    for &(s, t) in &wards {
        foes[s].guarding = true;
        foes[t].warded = true;
    }
    let marks: Vec<Mark> = world
        .arrivals
        .iter()
        .map(|a| Mark {
            pos: a.pos,
            radius: a.kind.radius(),
            progress: a.progress(),
        })
        .collect();
    let bolts: Vec<Bolt> = world
        .shots
        .iter()
        .map(|s| Bolt {
            pos: s.pos,
            radius: s.radius,
            hostile: s.hostile,
            vel: s.velocity,
        })
        .collect();
    let fx: Vec<Fx> = world
        .effects
        .iter()
        .map(|e| Fx {
            kind: e.kind,
            pos: e.pos,
            dir: e.dir,
            age: e.age,
            duration: e.duration,
            radius: e.radius,
        })
        .collect();
    let combat = world.phase == Phase::Combat;
    let hall = world.floor().and_then(|f| hall_art(f.id).map(|a| (f, a)));
    let in_hall = world.floor().is_some();
    let lost = world.phase == Phase::Lost;

    // Depth-sorted actor list: (depth, foe index or None for the courier).
    let mut order: Vec<(f32, Option<usize>)> = Vec::with_capacity(foes.len() + 1);
    order.push((hero.pos.x - hero.pos.y, None));
    for (i, f) in foes.iter().enumerate() {
        order.push((f.pos.x - f.pos.y, Some(i)));
    }
    order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let ground = Arc::new(Snapshot {
        fit,
        time,
        hero,
        foes: foes.clone(),
        wards,
        marks,
        bolts: bolts.clone(),
        fx: fx.clone(),
        aim,
        combat,
        hall: in_hall,
        chamber: world.chamber,
    });
    let front = Arc::clone(&ground);

    let mut root = div()
        .relative()
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .bg(rgb(INK));
    let mut preload: Vec<Arc<Image>> = Vec::new();
    match hall {
        Some((floor, art)) => {
            let (left, top, w, h) = floor_rect(&fit, floor);
            root = root.child(
                img(art.bg.clone())
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(w))
                    .h(px(h))
                    .object_fit(ObjectFit::Fill),
            );
            // The chamber this hall leads to, and this hall's own occluders.
            preload.push(art.atlas.clone());
            preload.push(arena_image(world.chamber));
        }
        None => {
            root = root.child(
                img(arena_image(world.chamber))
                    .absolute()
                    .left(px(fit.ox))
                    .top(px(fit.oy))
                    .w(px(DESIGN_W * fit.s))
                    .h(px(DESIGN_H * fit.s))
                    .object_fit(ObjectFit::Fill),
            );
            // The next hall if there is one, else the next arena (in the last
            // chamber that is the current background and costs nothing). A
            // finished run restarts in the first hall.
            let next = match world.phase {
                Phase::Won | Phase::Lost => floor_for(1),
                _ => floor_for(world.chamber + 1),
            };
            match next.and_then(|f| hall_art(f.id)) {
                Some(next_art) => {
                    preload.push(next_art.bg.clone());
                    preload.push(next_art.atlas.clone());
                }
                None => preload.push(arena_image(world.chamber + 1)),
            }
        }
    }
    // GPUI decodes an image off-thread the first time it is laid out and
    // draws nothing until then. Lay out whatever comes next, hidden, so it is
    // ready when the game advances.
    for image in preload {
        root = root.child(img(image).absolute().size_0().invisible());
    }
    root = root.child(
        canvas(
            |_, _, _| (),
            move |b, _, win, _| paint_ground(&ground, b, win),
        )
        .absolute()
        .size_full(),
    );

    for (_, who) in order {
        match who {
            None => {
                let flicker = hero.flash > 0.0 && (time * 40.0).sin() > 0.0;
                let bob = if combat || hall.is_some() {
                    0.0
                } else {
                    (time * 2.2).sin() * 1.2
                };
                let alpha = if lost { 0.55 } else { 1.0 };
                if hero.dashing {
                    // Pale spectral afterimages trailing behind the dash.
                    let back = world.player.dash_pose().unwrap().0;
                    for (k, a) in [(2.0f32, 0.16f32), (1.0, 0.30)] {
                        let p = Vec2::new(
                            hero.pos.x - back.x * 0.32 * k,
                            hero.pos.y - back.y * 0.32 * k,
                        );
                        root = root.child(sprite(
                            fit,
                            courier_image(&world.player),
                            p,
                            bob,
                            0.0,
                            0.0,
                            false,
                            a,
                        ));
                    }
                }
                root = root.child(sprite(
                    fit,
                    courier_image(&world.player),
                    hero.pos,
                    bob,
                    0.0,
                    0.0,
                    flicker,
                    alpha,
                ));
            }
            Some(i) => {
                let f = foes[i];
                let gray = f.flash > 0.35;
                root = root.child(sprite(
                    fit,
                    f.kind.image(f.dir, f.frame),
                    f.pos,
                    0.0,
                    0.0,
                    0.0,
                    gray,
                    1.0,
                ));
            }
        }
    }

    // Local occlusion treatment: where the courier stands behind tall
    // geometry, a translucent copy of that geometry goes back over it.
    if let Some((floor, art)) = hall {
        for piece in occluders(art, floor, &fit, hero.pos) {
            root = root.child(piece);
        }
    }

    root = root.child(
        canvas(
            |_, _, _| (),
            move |b, _, win, _| paint_front(&front, b, win),
        )
        .absolute()
        .size_full(),
    );

    if lost {
        root = root.child(div().absolute().size_full().bg(col(TEAL_DEEP, 0.35)));
    }
    root
}

/// World-space point offset from `c` by screen pixels (`ox` right, `up` up).
fn ward_pt(c: (f32, f32, f32), ox: f32, up: f32) -> (f32, f32, f32) {
    // Projection: screen x = 44 * (x + y), screen y drops 52 px per unit z.
    let k = ox / 88.0;
    (c.0 + k, c.1 + k, c.2 + up / 52.0)
}

/// Flow tether on the floor from ward source to warded ally: dark edge, dashed
/// violet core, beads travelling source -> target, ring under the target.
fn ward_tether(pen: &mut Pen, src: &Foe, dst: &Foe, t: f32) {
    let d = Vec2::new(dst.pos.x - src.pos.x, dst.pos.y - src.pos.y);
    let len = (d.x * d.x + d.y * d.y).sqrt();
    if len < 1e-3 {
        return;
    }
    let u = Vec2::new(d.x / len, d.y / len);
    let a = src.radius * 0.6;
    let b = (len - dst.radius * 0.8).max(a + 0.05);
    let at = |s: f32| (src.pos.x + u.x * s, src.pos.y + u.y * s, 0.03);
    pen.line(at(a), at(b), 5.0, col(INK, 0.55));
    // Marching dashes (shape cue independent of colour).
    let (dash, gap) = (0.34, 0.2);
    let mut s = a - ((t * 0.9) % (dash + gap)) + (dash + gap);
    while s < b {
        let (s0, s1) = (s.max(a), (s + dash).min(b));
        if s1 > s0 {
            pen.line(at(s0), at(s1), 2.4, col(WARD, 0.8));
        }
        s += dash + gap;
    }
    // Beads carry the direction: source end to target end.
    for i in 0..4 {
        let f = (t * 0.6 + i as f32 * 0.25).fract();
        let p = at(a + (b - a) * f);
        let fade = (f * PI).sin();
        pen.disc(p.0, p.1, p.2, 3.6, col(INK, 0.8 * fade));
        pen.disc(p.0, p.1, p.2, 2.6, col(PEARL, 0.95 * fade));
    }
    let pulse = 0.5 + 0.5 * (t * 4.0).sin();
    pen.arc(
        dst.pos,
        dst.radius * 1.0,
        dst.radius * 1.15,
        0.0,
        TAU,
        0.02,
        Some(col(WARD, 0.55 + 0.25 * pulse)),
        Some((1.0, col(INK, 0.6))),
    );
    pen.arc(
        src.pos,
        src.radius * 0.9,
        src.radius * 1.05,
        0.0,
        TAU,
        0.02,
        Some(col(WARD, 0.5)),
        Some((1.0, col(INK, 0.6))),
    );
}

/// Wave entry postmark: a pearl ring sized to the clerk, a brass sweep that
/// completes as it surfaces, and four ticks that close onto the ring.
fn entry_mark(pen: &mut Pen, c: Vec2, body: f32, progress: f32, t: f32) {
    let p = progress.clamp(0.0, 1.0);
    let r = body + 0.3;
    let pulse = 0.5 + 0.5 * (t * 9.0).sin();
    pen.arc(
        c,
        0.0,
        r * (0.35 + 0.65 * p),
        0.0,
        TAU,
        0.0,
        Some(col(BRASS, 0.08 + 0.14 * p)),
        None,
    );
    pen.arc(
        c,
        0.0,
        r,
        0.0,
        TAU,
        0.0,
        None,
        Some((1.3, col(PEARL, 0.35 + 0.25 * pulse))),
    );
    pen.arc_line(
        c,
        r,
        -PI * 0.5,
        -PI * 0.5 + TAU * p.max(0.02),
        0.0,
        2.4,
        col(BRASS, 0.9),
    );
    let gap = 0.08 + 0.35 * (1.0 - p);
    for k in 0..4 {
        let a = PI * 0.25 + k as f32 * PI * 0.5;
        let (dx, dy) = (a.cos(), a.sin());
        pen.line(
            (c.x + dx * (r + gap), c.y + dy * (r + gap), 0.0),
            (
                c.x + dx * (r + gap + 0.22),
                c.y + dy * (r + gap + 0.22),
                0.0,
            ),
            2.0,
            col(BRASS, 0.85),
        );
    }
}

// ------------------------------------------------------------- telegraphs ---

/// One attack family per distinct warning shape.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Family {
    /// Bailiff: contact lunge down a lane.
    Lunge,
    /// Censor: one bolt, or a three-bolt spread from chamber 2.
    Volley { n: u32 },
    /// Postmaster: bolts in every direction.
    Ring { n: u32, offset: f32 },
    /// Postmaster: wide contact charge that stops at a wall.
    Charge,
    /// Postmaster: a five- or seven-bolt fan.
    Fan { n: u32 },
    /// Postmaster: two clerks surface at its flanks (no damage of its own).
    Summon,
    /// Postmaster: the ring released as its half-hull stagger ends.
    Shockwave,
}

fn family(f: &Foe, chamber: usize) -> Option<Family> {
    if f.shock > 0.0 {
        return Some(Family::Shockwave);
    }
    if f.windup <= 0.0 {
        return None;
    }
    Some(match f.kind {
        Kind::WardClerk => return None,
        Kind::Bailiff => Family::Lunge,
        Kind::Censor => Family::Volley {
            n: if chamber >= 2 { 3 } else { 1 },
        },
        Kind::Postmaster => match f.attack {
            ATT_RING => Family::Ring {
                n: if f.phase2 { 18 } else { 12 },
                offset: f.ring_offset,
            },
            ATT_CHARGE => Family::Charge,
            ATT_FAN => Family::Fan {
                n: if f.phase2 { 7 } else { 5 },
            },
            ATT_SUMMON => Family::Summon,
            _ => return None,
        },
    })
}

fn urgency(f: &Foe) -> f32 {
    f.windup.max(f.shock).clamp(0.0, 1.0)
}

/// Floor region an attack can damage: every courier position within `half`
/// of the segment from `o` along unit `dir` for `len`.
#[derive(Clone, Copy, Debug)]
struct Zone {
    o: Vec2,
    dir: Vec2,
    len: f32,
    half: f32,
}

impl Zone {
    /// Drawn length: bolts fly a little past the floor before they are culled,
    /// but nothing out there can be hit, so the warning stops at the edge.
    fn on_floor(&self) -> f32 {
        self.len.min(exit_distance(self.o, self.dir, ARENA))
    }

    fn at(&self, s: f32, side: f32) -> Vec2 {
        Vec2::new(
            self.o.x + self.dir.x * s - self.dir.y * side,
            self.o.y + self.dir.y * s + self.dir.x * side,
        )
    }

    /// Distance from `p` to the zone's centre segment.
    #[cfg(test)]
    fn distance(&self, p: Vec2) -> f32 {
        let d = p - self.o;
        let s = d.dot(self.dir).clamp(0.0, self.len);
        (d - self.dir * s).len()
    }
}

/// Distance along unit `d` from `o` to the edge of the square `[-h, h]^2`.
fn exit_distance(o: Vec2, d: Vec2, h: f32) -> f32 {
    let mut t = f32::INFINITY;
    for (p, v) in [(o.x, d.x), (o.y, d.y)] {
        if v > 1e-6 {
            t = t.min((h - p) / v);
        } else if v < -1e-6 {
            t = t.min((-h - p) / v);
        }
    }
    t.max(0.0)
}

fn bolt_zone(o: Vec2, angle: f32, half: f32) -> Zone {
    let dir = Vec2::from_angle(angle);
    Zone {
        o,
        dir,
        len: exit_distance(o, dir, BOLT_LIMIT),
        half,
    }
}

fn spread(o: Vec2, base: f32, n: u32, step: f32, half: f32) -> Vec<Zone> {
    (0..n)
        .map(|k| bolt_zone(o, base + (k as f32 - (n - 1) as f32 * 0.5) * step, half))
        .collect()
}

fn ring(o: Vec2, n: u32, offset: f32, half: f32) -> Vec<Zone> {
    (0..n)
        .map(|k| bolt_zone(o, offset + k as f32 * TAU / n as f32, half))
        .collect()
}

/// Where the attack being wound up can damage the courier once it releases.
fn footprint(f: &Foe, fam: Family) -> Vec<Zone> {
    let dir = norm(f.dir, Vec2::new(1.0, 0.0));
    let base = dir.y.atan2(dir.x);
    // Contact attacks stop where the attacker's body meets a wall.
    let lane = |travel: f32, contact: f32| Zone {
        o: f.pos,
        dir,
        len: travel.min(exit_distance(f.pos, dir, ARENA - f.radius)),
        half: contact,
    };
    match fam {
        Family::Lunge => vec![lane(BAILIFF_TRAVEL, BAILIFF_CONTACT)],
        Family::Charge => vec![lane(BOSS_CHARGE_TRAVEL, BOSS_CONTACT)],
        Family::Volley { n } => spread(f.pos, base, n, CENSOR_SPREAD, VOLLEY_HIT),
        Family::Fan { n } => spread(f.pos, base, n, BOSS_FAN_SPREAD, VOLLEY_HIT),
        Family::Ring { n, offset } => ring(f.pos, n, offset, RING_HIT),
        Family::Shockwave => ring(f.pos, SHOCKWAVE_BOLTS, 0.0, RING_HIT),
        Family::Summon => Vec::new(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Fill,
    Cue,
}

fn ground(p: Vec2) -> (f32, f32, f32) {
    (p.x, p.y, 0.0)
}

/// Lane outline with round ends, `len` long.
fn stadium(z: &Zone, len: f32) -> Vec<(f32, f32, f32)> {
    let base = z.dir.y.atan2(z.dir.x);
    let n = 10;
    let mut pts = Vec::with_capacity(2 * n + 2);
    for (c, a0) in [(len, base - PI * 0.5), (0.0, base + PI * 0.5)] {
        let centre = z.at(c, 0.0);
        for i in 0..=n {
            let a = a0 + PI * i as f32 / n as f32;
            pts.push((
                centre.x + z.half * a.cos(),
                centre.y + z.half * a.sin(),
                0.0,
            ));
        }
    }
    pts
}

/// Straight band covering a bolt's path, `len` long.
fn band(z: &Zone, len: f32) -> [(f32, f32, f32); 4] {
    [
        ground(z.at(0.0, -z.half)),
        ground(z.at(len, -z.half)),
        ground(z.at(len, z.half)),
        ground(z.at(0.0, z.half)),
    ]
}

/// Paint one layer of an enemy's warning. Wax red marks danger; the shape and
/// motion say which attack it is, so the cues survive in grayscale:
/// - lunge: round-ended lane with pearl chevrons running down it;
/// - charge: wide double-edged lane with rungs sweeping across it and a wall bar;
/// - volley: bolt-wide bands with ink drops streaming out;
/// - fan: the same bands, joined near the boss by a rim arc;
/// - ring: spokes with a circle closing onto the boss;
/// - shockwave: spokes with a dashed circle swelling outward;
/// - summon: two seals with turning ticks.
///
/// The edge turns pearl and blinks in the last quarter of the windup.
fn telegraph(pen: &mut Pen, f: &Foe, fam: Family, t: f32, layer: Layer) {
    let zones = footprint(f, fam);
    let w = urgency(f);
    let edge = if w > 0.75 && (t * 20.0).sin() > 0.0 {
        col(PEARL, 0.9)
    } else {
        col(WAX, 0.5 + 0.35 * w)
    };
    let cue = col(PEARL, 0.3 + 0.5 * w);
    match (fam, layer) {
        (Family::Lunge | Family::Charge, Layer::Fill) => {
            for z in &zones {
                pen.path(
                    &stadium(z, z.len),
                    true,
                    Some(col(WAX, 0.08 + 0.08 * w)),
                    None,
                );
                pen.path(
                    &stadium(z, z.len * w),
                    true,
                    Some(col(WAX, 0.22 + 0.3 * w)),
                    None,
                );
            }
        }
        (Family::Lunge, Layer::Cue) => {
            for z in &zones {
                pen.path(&stadium(z, z.len), true, None, Some((1.6, edge)));
                for k in 0..3 {
                    let s = ((t * 1.8 + k as f32 / 3.0).fract()) * z.len;
                    let tip = ground(z.at(s + 0.22, 0.0));
                    for side in [-1.0, 1.0] {
                        pen.line(ground(z.at(s - 0.12, side * z.half * 0.55)), tip, 2.2, cue);
                    }
                }
            }
        }
        (Family::Charge, Layer::Cue) => {
            for z in &zones {
                pen.path(&stadium(z, z.len), true, None, Some((2.8, edge)));
                let inner = Zone {
                    half: z.half * 0.8,
                    ..*z
                };
                pen.path(
                    &stadium(&inner, z.len),
                    true,
                    None,
                    Some((1.0, col(WAX, 0.7))),
                );
                let gap = 0.45;
                let mut s = (t * 2.6).fract() * gap;
                while s < z.len {
                    let h = inner.half * 0.9;
                    pen.line(ground(z.at(s, -h)), ground(z.at(s, h)), 1.8, cue);
                    s += gap;
                }
                let far = z.len + z.half * 0.6;
                pen.line(
                    ground(z.at(far, -z.half)),
                    ground(z.at(far, z.half)),
                    3.2,
                    edge,
                );
            }
        }
        (
            Family::Volley { .. } | Family::Fan { .. } | Family::Ring { .. } | Family::Shockwave,
            Layer::Fill,
        ) => {
            for z in &zones {
                pen.path(
                    &band(z, z.on_floor()),
                    true,
                    Some(col(WAX, 0.05 + 0.07 * w)),
                    None,
                );
            }
        }
        (Family::Volley { .. } | Family::Fan { .. }, Layer::Cue) => {
            for z in &zones {
                pen.line(ground(z.o), ground(z.at(z.on_floor(), 0.0)), 1.3, edge);
                let gap = 0.7;
                let mut s = 0.4 + (t * 1.6).fract() * gap;
                while s < z.on_floor() {
                    let p = z.at(s, 0.0);
                    pen.disc(p.x, p.y, 0.0, 2.4, cue);
                    s += gap;
                }
            }
            if let Family::Fan { n } = fam {
                // Sweep from the facing angle, not per-lane atan2, so a fan
                // aimed at -X does not wrap the long way round.
                let d = norm(f.dir, Vec2::new(1.0, 0.0));
                let half = (n - 1) as f32 * 0.5 * BOSS_FAN_SPREAD;
                let (a0, a1) = (d.y.atan2(d.x) - half, d.y.atan2(d.x) + half);
                let r = f.radius + 0.55;
                pen.arc_line(f.pos, r, a0, a1, 0.0, 2.6, edge);
                pen.arc_line(f.pos, r + 0.18, a0, a1, 0.0, 1.0, cue);
            }
        }
        (Family::Ring { .. } | Family::Shockwave, Layer::Cue) => {
            for z in &zones {
                let near = z.on_floor().min(RING_GUIDE);
                pen.line(
                    ground(z.at(f.radius, 0.0)),
                    ground(z.at(near, 0.0)),
                    1.2,
                    edge,
                );
                if z.on_floor() > near {
                    pen.line(
                        ground(z.at(near, 0.0)),
                        ground(z.at(z.on_floor(), 0.0)),
                        1.0,
                        col(WAX, 0.25 + 0.2 * w),
                    );
                }
            }
            if fam == Family::Shockwave {
                // Dashed circle swelling outward, one dash between each pair of spokes.
                let r = f.radius + 0.2 + (RING_GUIDE - f.radius) * w;
                let n = zones.len().max(1);
                for k in 0..n {
                    let a = (k as f32 + 0.5) * TAU / n as f32;
                    pen.arc_line(f.pos, r, a - 0.12, a + 0.12, 0.0, 3.0, cue);
                }
            } else {
                // A full circle closing in on the boss as release nears.
                pen.arc_line(f.pos, RING_GUIDE, 0.0, TAU, 0.0, 1.0, col(WAX, 0.45));
                let r = f.radius + 0.2 + (RING_GUIDE - f.radius - 0.2) * (1.0 - w);
                pen.arc_line(f.pos, r, 0.0, TAU, 0.0, 2.6, cue);
            }
        }
        (Family::Summon, layer) => {
            // Seals open at the two flanks where clerks surface.
            let to = norm(f.dir, Vec2::new(1.0, 0.0));
            let side = Vec2::new(-to.y, to.x);
            for s in [1.0f32, -1.0] {
                let c = Vec2::new(f.pos.x + side.x * s * 1.6, f.pos.y + side.y * s * 1.6);
                let r = 0.55 * (0.4 + 0.6 * w);
                if layer == Layer::Fill {
                    pen.arc(
                        c,
                        0.0,
                        r,
                        0.0,
                        TAU,
                        0.0,
                        Some(col(WAX, 0.15 + 0.3 * w)),
                        None,
                    );
                } else {
                    pen.arc(c, 0.0, r, 0.0, TAU, 0.0, None, Some((1.6, col(PEARL, 0.5))));
                    for k in 0..3 {
                        let a = t * 2.0 * s + k as f32 * TAU / 3.0;
                        pen.arc_line(c, r + 0.12, a, a + 0.6, 0.0, 1.8, cue);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Input;

    /// Ordered like the baked file indices.
    const DIRECTIONS: [Vec2; 8] = [
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(DIAG, DIAG),
        Vec2::new(-DIAG, DIAG),
        Vec2::new(-DIAG, -DIAG),
        Vec2::new(DIAG, -DIAG),
    ];
    const DIAG: f32 = std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn nearest_sprite_view_matches_baked_angles() {
        for (index, facing) in DIRECTIONS.iter().enumerate() {
            let angle = facing.y.atan2(facing.x);
            for offset in [-0.38, 0.0, 0.38] {
                let a = angle + offset;
                assert_eq!(view_direction(Vec2::new(a.cos(), a.sin())), index);
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Attack {
        name: &'static str,
        kind: EnemyKind,
        /// `None` stages the end of the half-hull stagger.
        attack: Option<u8>,
        attack_index: u32,
        phase2: bool,
        chamber: usize,
    }

    const fn attack(
        name: &'static str,
        kind: EnemyKind,
        attack: Option<u8>,
        attack_index: u32,
        phase2: bool,
        chamber: usize,
    ) -> Attack {
        Attack {
            name,
            kind,
            attack,
            attack_index,
            phase2,
            chamber,
        }
    }

    const ATTACKS: [Attack; 10] = [
        attack("bailiff lunge", EnemyKind::Bailiff, Some(0), 0, false, 1),
        attack("censor bolt", EnemyKind::Censor, Some(0), 0, false, 1),
        attack("censor volley", EnemyKind::Censor, Some(0), 0, false, 2),
        attack("ring", EnemyKind::Postmaster, Some(ATT_RING), 2, false, 3),
        attack(
            "rotated ring",
            EnemyKind::Postmaster,
            Some(ATT_RING),
            1,
            false,
            3,
        ),
        attack(
            "second-shift ring",
            EnemyKind::Postmaster,
            Some(ATT_RING),
            1,
            true,
            3,
        ),
        attack(
            "charge",
            EnemyKind::Postmaster,
            Some(ATT_CHARGE),
            0,
            false,
            3,
        ),
        attack("fan", EnemyKind::Postmaster, Some(ATT_FAN), 0, false, 3),
        attack(
            "second-shift fan",
            EnemyKind::Postmaster,
            Some(ATT_FAN),
            0,
            true,
            3,
        ),
        attack("stagger shockwave", EnemyKind::Postmaster, None, 0, true, 3),
    ];

    /// A quiet arena with the courier at `player` and each attack one tick
    /// from release at its origin.
    fn stage(attacks: &[(Attack, Vec2, Vec2)], player: Vec2) -> Game {
        let mut g = Game::quiet_arena(5);
        g.player.pos = player;
        for &(a, origin, dir) in attacks {
            g.chamber = a.chamber;
            g.stage_release(a.kind, a.attack, a.attack_index, origin, dir, a.phase2);
        }
        g
    }

    /// Run the staged attacks out with the courier standing still. Each
    /// attacker is disarmed once its attack has resolved so it cannot start
    /// another one (it stays, so the chamber never counts as cleared).
    /// Returns true if the courier was damaged.
    fn lands(mut g: Game) -> bool {
        let hp = g.player.hp;
        for _ in 0..240 {
            g.step(Input::default());
            if g.player.hp < hp {
                return true;
            }
            let mut live = false;
            for i in 0..g.enemies.len() {
                if g.attacking(i) {
                    live = true;
                } else {
                    g.disarm(i);
                }
            }
            if !live && !g.shots.iter().any(|s| s.hostile) {
                return false;
            }
        }
        false
    }

    /// The warning footprints the scene draws for the staged attacks.
    fn warnings(g: &Game) -> Vec<Zone> {
        g.enemies
            .iter()
            .map(foe)
            .flat_map(|f| {
                let fam = family(&f, g.chamber).expect("staged attack shows a warning");
                footprint(&f, fam)
            })
            .collect()
    }

    /// Courier positions on a grid across the reachable floor.
    fn floor() -> Vec<Vec2> {
        let m = ARENA - COURIER_RADIUS;
        let n = 26;
        let mut pts = Vec::new();
        for i in 0..=n {
            for j in 0..=n {
                let u = -m + 2.0 * m * i as f32 / n as f32;
                let v = -m + 2.0 * m * j as f32 / n as f32;
                pts.push(Vec2::new(u, v));
            }
        }
        pts
    }

    #[test]
    fn every_warning_covers_its_damaging_ticks_in_all_eight_directions() {
        let floor = floor();
        for a in ATTACKS {
            for d in DIRECTIONS {
                // Open floor ahead, then a wall close ahead.
                for origin in [d * -1.2, d * 2.4] {
                    let staged = [(a, origin, d)];
                    let zones = warnings(&stage(&staged, Vec2::new(9.0, 9.0)));
                    assert!(!zones.is_empty(), "{}: no footprint", a.name);
                    for &p in &floor {
                        let hit = lands(stage(&staged, p));
                        let gap = zones
                            .iter()
                            .map(|z| z.distance(p) - z.half)
                            .fold(f32::INFINITY, f32::min);
                        assert!(
                            !hit || gap <= 0.02,
                            "{} toward {d:?} from {origin:?}: hit at {p:?}, {gap:.2} outside the warning",
                            a.name
                        );
                        // Well inside the warning the attack must connect: the
                        // footprint is not padded beyond what can hit.
                        let deep = zones.iter().any(|z| {
                            z.distance(p) <= z.half - 0.15 && (p - z.o).dot(z.dir) <= z.len * 0.85
                        });
                        assert!(
                            !deep || hit,
                            "{} toward {d:?} from {origin:?}: never connected at {p:?} inside the warning",
                            a.name
                        );
                    }
                }
            }
        }
    }

    /// A crowded moment: a Bailiff lunging and a Censor volley crossing the
    /// Postmaster's fan. Rotated to each of the eight directions, every hit
    /// lands inside the union of the three warnings, and each warning keeps
    /// its own family. Writes a top-down capture per direction to
    /// `target/warning-capture/` for review.
    #[test]
    fn overlapping_encounter_hits_only_inside_its_warnings() {
        let floor = floor();
        for (i, d) in DIRECTIONS.into_iter().enumerate() {
            let side = Vec2::new(-d.y, d.x);
            let staged = [
                (ATTACKS[0], d * -1.6 + side * 1.0, d),
                (ATTACKS[2], d * -2.6 + side * -1.4, (d + side * 0.45).norm()),
                (ATTACKS[8], d * -2.4, d),
            ];
            let g = stage(&staged, Vec2::new(9.0, 9.0));
            let families: Vec<Family> = g
                .enemies
                .iter()
                .map(|e| family(&foe(e), g.chamber).unwrap())
                .collect();
            assert!(matches!(
                families[..],
                [Family::Lunge, Family::Volley { n: 3 }, Family::Fan { n: 7 }]
            ));
            let zones = warnings(&g);
            let mut hits = Vec::new();
            for &p in &floor {
                if lands(stage(&staged, p)) {
                    assert!(
                        zones.iter().any(|z| z.distance(p) <= z.half + 0.02),
                        "direction {d:?}: hit at {p:?} outside every warning"
                    );
                    hits.push(p);
                }
            }
            assert!(!hits.is_empty());
            write_capture(i, &zones, &floor, &hits);
        }
    }

    fn write_capture(direction: usize, zones: &[Zone], floor: &[Vec2], hits: &[Vec2]) {
        let fit = Fit::new(DESIGN_W, DESIGN_H, Vec2::ZERO);
        let pt = |p: Vec2| {
            let (x, y) = fit.project(p.x, p.y, 0.0);
            format!("{x:.1},{y:.1}")
        };
        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {DESIGN_W} {DESIGN_H}'>\
             <rect width='100%' height='100%' fill='#060f12'/>"
        );
        let a = ARENA;
        svg += &format!(
            "<polygon points='{} {} {} {}' fill='#10353b'/>",
            pt(Vec2::new(-a, -a)),
            pt(Vec2::new(a, -a)),
            pt(Vec2::new(a, a)),
            pt(Vec2::new(-a, a))
        );
        for z in zones {
            let pts: Vec<String> = if z.len < 6.0 && z.half > 0.7 {
                stadium(z, z.len)
                    .iter()
                    .map(|p| pt(Vec2::new(p.0, p.1)))
                    .collect()
            } else {
                band(z, z.len)
                    .iter()
                    .map(|p| pt(Vec2::new(p.0, p.1)))
                    .collect()
            };
            svg += &format!(
                "<polygon points='{}' fill='#d2301f' fill-opacity='0.22' stroke='#d2301f'/>",
                pts.join(" ")
            );
        }
        for &p in floor {
            let colour = if hits.contains(&p) {
                "#e9e4d2"
            } else {
                "#2fb3a2"
            };
            let (x, y) = fit.project(p.x, p.y, 0.0);
            svg += &format!("<circle cx='{x:.1}' cy='{y:.1}' r='1.6' fill='{colour}'/>");
        }
        svg += "</svg>";
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/warning-capture");
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join(format!("direction-{direction}.svg")), svg);
        }
    }

    // ------------------------------------------------------------ halls ---

    const VIEWPORTS: [(f32, f32); 5] = [
        (820.0, 560.0),
        (900.0, 600.0),
        (1280.0, 800.0),
        (1920.0, 1080.0),
        (700.0, 1400.0),
    ];
    const CAMERAS: [Vec2; 5] = [
        Vec2::new(0.0, 0.0),
        Vec2::new(6.5, -3.25),
        Vec2::new(-12.0, 9.5),
        Vec2::new(21.25, 14.0),
        Vec2::new(-4.0, -17.5),
    ];

    #[test]
    fn unprojection_inverts_projection_for_every_camera_and_viewport() {
        for (w, h) in VIEWPORTS {
            for cam in CAMERAS {
                let fit = Fit::new(w, h, cam);
                for p in [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(3.5, -2.0),
                    Vec2::new(-8.0, 11.0),
                    Vec2::new(25.0, 30.0),
                ] {
                    let (sx, sy) = fit.project(p.x, p.y, 0.0);
                    let back = screen_to_world(sx, sy, w, h, cam);
                    assert!(
                        (back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3,
                        "{w}x{h} camera {cam:?}: {p:?} came back as {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn camera_at_the_origin_is_the_arena_projection() {
        for (w, h) in VIEWPORTS {
            let fit = Fit::new(w, h, Vec2::ZERO);
            let (sx, sy) = fit.project(2.0, -1.5, 1.0);
            let dx = 450.0 + 44.0 * (2.0 - 1.5);
            let dy = 308.0 + 24.2 * (2.0 + 1.5) - 52.0;
            assert!((sx - (fit.ox + dx * fit.s)).abs() < 1e-3);
            assert!((sy - (fit.oy + dy * fit.s)).abs() < 1e-3);
        }
    }

    #[test]
    fn camera_point_lands_at_the_design_view_centre() {
        for (w, h) in VIEWPORTS {
            for cam in CAMERAS {
                let fit = Fit::new(w, h, cam);
                let (sx, sy) = fit.project(cam.x, cam.y, 0.0);
                assert!((sx - (fit.ox + 450.0 * fit.s)).abs() < 1e-3);
                assert!((sy - (fit.oy + 308.0 * fit.s)).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn a_world_point_maps_to_the_same_floor_image_pixel_everywhere() {
        for chamber in [1, 2] {
            let floor = floor_for(chamber).expect("hall");
            for (w, h) in VIEWPORTS {
                for cam in CAMERAS {
                    let fit = Fit::new(w, h, cam);
                    let (left, top, iw, ih) = floor_rect(&fit, floor);
                    assert!((iw - floor.image_size.0 * fit.s).abs() < 1e-3);
                    assert!((ih - floor.image_size.1 * fit.s).abs() < 1e-3);
                    for p in [floor.entrance, floor.bounds_max, Vec2::new(-3.0, 4.5)] {
                        let (sx, sy) = fit.project(p.x, p.y, 0.0);
                        let px = (sx - left) / fit.s;
                        let py = (sy - top) / fit.s;
                        let want_x = floor.origin.0 + 44.0 * (p.x + p.y);
                        let want_y = floor.origin.1 + 24.2 * (p.x - p.y);
                        assert!(
                            (px - want_x).abs() < 2e-3 && (py - want_y).abs() < 2e-3,
                            "{} {w}x{h} camera {cam:?}: {p:?} at image ({px}, {py}), want ({want_x}, {want_y})",
                            floor.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cursor_over_a_floor_point_aims_at_it_after_the_camera_moves() {
        let floor = floor_for(1).expect("hall");
        let target = floor.bounds_max;
        for (w, h) in VIEWPORTS {
            for cam in CAMERAS {
                let fit = Fit::new(w, h, cam);
                let (sx, sy) = fit.project(target.x, target.y, 0.0);
                let aim = screen_to_world(sx, sy, w, h, cam);
                assert!((aim.x - target.x).abs() < 1e-3 && (aim.y - target.y).abs() < 1e-3);
            }
        }
    }

    fn slab(x0: f32, y0: f32, x1: f32, y1: f32) -> Block {
        Block {
            min: Vec2::new(x0, y0),
            max: Vec2::new(x1, y1),
        }
    }

    #[test]
    fn courier_is_behind_a_block_only_on_its_far_sides() {
        let b = slab(0.0, 0.0, 4.0, 2.0);
        // Far sides: toward -x, toward +y.
        assert!(behind(Vec2::new(-1.0, 1.0), &b));
        assert!(behind(Vec2::new(2.0, 3.0), &b));
        assert!(behind(Vec2::new(-1.0, 3.0), &b));
        // Near sides: toward +x, toward -y.
        assert!(!behind(Vec2::new(5.0, 1.0), &b));
        assert!(!behind(Vec2::new(2.0, -1.0), &b));
        assert!(!behind(Vec2::new(5.0, -1.0), &b));
        // Corners that are far on one axis and near on the other are beside it.
        assert!(!behind(Vec2::new(-1.0, -1.0), &b));
        assert!(!behind(Vec2::new(5.0, 3.0), &b));
        // Inside the footprint is never behind.
        assert!(!behind(Vec2::new(2.0, 1.0), &b));
    }

    #[test]
    fn occluder_tables_name_real_blocks_and_fit_inside_their_atlas() {
        for chamber in [1, 2] {
            let floor = floor_for(chamber).expect("hall");
            let art = hall_art(floor.id).expect("art");
            assert!(!art.pieces.is_empty(), "{} has no occluders", floor.id);
            for p in &art.pieces {
                assert!(
                    p.block < floor.blocks.len(),
                    "{}: block {}",
                    floor.id,
                    p.block
                );
                assert!(p.atlas.0 + p.atlas.2 <= art.atlas_size.0 + 0.5);
                assert!(p.atlas.1 + p.atlas.3 <= art.atlas_size.1 + 0.5);
                assert!((p.dest.2 - p.atlas.2 * 0.5).abs() < 0.51);
                assert!((p.dest.3 - p.atlas.3 * 0.5).abs() < 0.51);
            }
        }
    }
}
