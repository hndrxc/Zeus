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
    EffectKind, EnemyKind, Game, Phase, Vec2, ARENA, ATT_CHARGE, ATT_FAN, ATT_RING, ATT_SUMMON,
};

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
/// Bailiff lunge: 9.5 u/s for 0.28 s plus contact (0.38 + 0.28 + 0.1).
const BAILIFF_LUNGE: f32 = 9.5 * 0.28 + 0.76;
const BAILIFF_HALF_WIDTH: f32 = 0.76;
/// Censor / boss fan: bolts fly ~4.8-5 u/s for 4 s, clipped to the arena.
const CENSOR_LANE: f32 = 8.0;
const CENSOR_SPREAD: f32 = 0.32;
const BOSS_FAN_SPREAD: f32 = 0.26;
const BOSS_RING_LANE: f32 = 3.2;
/// Boss charge: 7.5 u/s for 0.55 s plus contact (0.75 + 0.28 + 0.1).
const BOSS_CHARGE: f32 = 7.5 * 0.55 + 1.13;
const BOSS_CHARGE_HALF_WIDTH: f32 = 1.13;

const PEARL: u32 = 0xe9e4d2;
const TEAL: u32 = 0x2fb3a2;
const TEAL_DEEP: u32 = 0x0b3d3b;
const WAX: u32 = 0xd2301f;
const BRASS: u32 = 0xe6c43a;
const INK: u32 = 0x060f12;

static ARENA_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena.png")));
macro_rules! sprite_frames {
    ($actor:literal, $direction:literal; $($frame:literal),+) => {
        [$(png(include_bytes!(concat!("../assets/", $actor, "/", $direction, "_", $frame, ".png")))),+]
    };
}

macro_rules! courier_direction {
    ($direction:literal) => {
        sprite_frames!("courier", $direction; "00","01","02","03","04","05","06","07","08","09","10","11","12","13","14","15","16","17","18")
    };
}

static COURIER_FRAMES: LazyLock<[[Arc<Image>; 19]; 4]> = LazyLock::new(|| {
    [
        courier_direction!("0"),
        courier_direction!("1"),
        courier_direction!("2"),
        courier_direction!("3"),
    ]
});
macro_rules! enemy_direction {
    ($actor:literal, $direction:literal $(, $extra:literal)*) => {
        sprite_frames!($actor,$direction; "00","01","02","03","04","05","06","07","08","09","10","11","12","13" $(,$extra)*)
    }
}

macro_rules! enemy_views {
    ($actor:literal $(, $extra:literal)*) => {
        [enemy_direction!($actor,"0" $(,$extra)*),enemy_direction!($actor,"1" $(,$extra)*),
         enemy_direction!($actor,"2" $(,$extra)*),enemy_direction!($actor,"3" $(,$extra)*)]
    }
}

static BAILIFF_FRAMES: LazyLock<[[Arc<Image>; 14]; 4]> = LazyLock::new(|| enemy_views!("bailiff"));
static CENSOR_FRAMES: LazyLock<[[Arc<Image>; 14]; 4]> = LazyLock::new(|| enemy_views!("censor"));
static POSTMASTER_FRAMES: LazyLock<[[Arc<Image>; 34]; 4]> = LazyLock::new(|| {
    enemy_views!(
        "postmaster",
        "14",
        "15",
        "16",
        "17",
        "18",
        "19",
        "20",
        "21",
        "22",
        "23",
        "24",
        "25",
        "26",
        "27",
        "28",
        "29",
        "30",
        "31",
        "32",
        "33"
    )
});
fn png(bytes: &'static [u8]) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec()))
}

fn arena_image() -> Arc<Image> {
    ARENA_IMG.clone()
}
fn courier_image(player: &crate::engine::Player) -> Arc<Image> {
    let (facing, frame) = if let Some((dir, progress)) = player.dash_pose() {
        (dir, 16 + (progress * 3.0) as usize)
    } else if let Some((kind, dir, progress)) = player.attack_pose {
        let frame = if kind == EffectKind::Strike {
            9 + (progress * 4.0) as usize
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
    if facing.x.abs() >= facing.y.abs() {
        if facing.x >= 0.0 {
            0
        } else {
            2
        }
    } else if facing.y >= 0.0 {
        1
    } else {
        3
    }
}

fn col(hex: u32, a: f32) -> Hsla {
    let mut c: Hsla = rgb(hex).into();
    c.a = a;
    c
}

/// Uniform fit of the 900x600 design space into the viewport.
#[derive(Clone, Copy)]
struct Fit {
    s: f32,
    ox: f32,
    oy: f32,
}

impl Fit {
    fn new(width: f32, height: f32) -> Self {
        let s = (width / DESIGN_W).min(height / DESIGN_H).max(0.0001);
        Fit {
            s,
            ox: (width - DESIGN_W * s) * 0.5,
            oy: (height - DESIGN_H * s) * 0.5,
        }
    }

    /// World (x, y, height z) to viewport pixels.
    fn project(&self, x: f32, y: f32, z: f32) -> (f32, f32) {
        let dx = 450.0 + 44.0 * (x + y);
        let dy = 308.0 + 24.2 * (x - y) - 52.0 * z;
        (self.ox + dx * self.s, self.oy + dy * self.s)
    }

    fn unproject(&self, sx: f32, sy: f32) -> Vec2 {
        let dx = (sx - self.ox) / self.s - 450.0;
        let dy = (sy - self.oy) / self.s - 308.0;
        let u = dx / 44.0;
        let v = dy / 24.2;
        Vec2::new((u + v) * 0.5, (u - v) * 0.5)
    }
}

/// Inverse of the ground projection (z = 0) for the same fit as `render`.
pub fn screen_to_world(x: f32, y: f32, width: f32, height: f32) -> Vec2 {
    Fit::new(width, height).unproject(x, y)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bailiff,
    Censor,
    Postmaster,
}

impl Kind {
    fn of(k: &EnemyKind) -> Kind {
        match k {
            EnemyKind::Bailiff => Kind::Bailiff,
            EnemyKind::Censor => Kind::Censor,
            EnemyKind::Postmaster => Kind::Postmaster,
        }
    }
    fn shadow(self) -> f32 {
        match self {
            Kind::Bailiff => 0.52,
            Kind::Censor => 0.44,
            Kind::Postmaster => 1.0,
        }
    }
    /// Approximate pixel height of the art above the foot (hp bar anchor).
    fn height(self) -> f32 {
        match self {
            Kind::Bailiff => 84.0,
            Kind::Censor => 78.0,
            Kind::Postmaster => 135.0,
        }
    }
    fn image(self, facing: Vec2, frame: usize) -> Arc<Image> {
        let direction = view_direction(facing);
        match self {
            Kind::Bailiff => BAILIFF_FRAMES[direction][frame].clone(),
            Kind::Censor => CENSOR_FRAMES[direction][frame].clone(),
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
}

#[derive(Clone, Copy)]
struct Bolt {
    pos: Vec2,
    radius: f32,
    hostile: bool,
    vel: Vec2,
}

#[derive(Clone, Copy)]
enum FxKind {
    Strike,
    Dash,
    Hit,
    Burst,
}

#[derive(Clone, Copy)]
struct Fx {
    kind: FxKind,
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
    bolts: Vec<Bolt>,
    fx: Vec<Fx>,
    aim: Option<Vec2>,
    combat: bool,
    chamber: usize,
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

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Hsla) {
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
    let steps = 8;
    let feather_x = 28.0;
    let feather_y = 18.0;
    for i in 0..steps {
        let u = 1.0 - (i as f32 + 0.5) / steps as f32;
        let a = u * u * 0.48;
        let dx = feather_x / steps as f32;
        let dy = feather_y / steps as f32;
        pen.rect(i as f32 * dx, 0.0, dx + 0.5, DESIGN_H, col(INK, a));
        pen.rect(
            DESIGN_W - (i as f32 + 1.0) * dx - 0.5,
            0.0,
            dx + 0.5,
            DESIGN_H,
            col(INK, a),
        );
        pen.rect(0.0, i as f32 * dy, DESIGN_W, dy + 0.5, col(INK, a));
        pen.rect(
            0.0,
            DESIGN_H - (i as f32 + 1.0) * dy - 0.5,
            DESIGN_W,
            dy + 0.5,
            col(INK, a),
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

    // Enemy telegraphs: wax-red, filling toward the strike moment. Shapes match
    // what the engine does on resolution (windup is 0..=1 progress, `dir` is the
    // locked attack direction).
    for f in &snap.foes {
        if f.windup <= 0.0 {
            continue;
        }
        let w = f.windup.clamp(0.0, 1.0);
        let to = norm(f.dir, Vec2::new(1.0, 0.0));
        let ang = to.y.atan2(to.x);
        let pulse = 0.5 + 0.5 * (t * 28.0).sin();
        let o = f.pos;
        match f.kind {
            Kind::Bailiff => lane(&mut pen, o, to, BAILIFF_LUNGE, BAILIFF_HALF_WIDTH, w, pulse),
            Kind::Censor => {
                // Volley: one bolt, or a three-bolt fan from chamber 2.
                let n: i32 = if snap.chamber >= 2 { 3 } else { 1 };
                for k in 0..n {
                    let a = ang + (k as f32 - (n - 1) as f32 * 0.5) * CENSOR_SPREAD;
                    let end = (
                        o.x + a.cos() * CENSOR_LANE,
                        o.y + a.sin() * CENSOR_LANE,
                        0.0,
                    );
                    let start = (o.x, o.y, 0.0);
                    pen.line(start, end, 6.0 * (1.0 - w) + 1.2, col(WAX, 0.08 + 0.10 * w));
                    pen.line(start, end, 1.3, col(WAX, 0.4 + 0.45 * w));
                }
            }
            Kind::Postmaster => match f.attack {
                ATT_RING => {
                    // Radial ink burst: ring around the boss, one lane per bolt.
                    let n = if f.phase2 { 18 } else { 12 };
                    let reach = BOSS_RING_LANE;
                    pen.arc(
                        o,
                        0.0,
                        reach,
                        0.0,
                        TAU,
                        0.0,
                        Some(col(WAX, 0.06 + 0.06 * w)),
                        Some((2.0, col(WAX, 0.5 + 0.3 * pulse * w))),
                    );
                    pen.arc(
                        o,
                        0.0,
                        reach * w,
                        0.0,
                        TAU,
                        0.0,
                        Some(col(WAX, 0.22 + 0.3 * w)),
                        None,
                    );
                    for k in 0..n {
                        let a = k as f32 * TAU / n as f32;
                        pen.line(
                            (o.x + a.cos() * 0.9, o.y + a.sin() * 0.9, 0.0),
                            (o.x + a.cos() * reach, o.y + a.sin() * reach, 0.0),
                            1.1,
                            col(WAX, 0.3 + 0.4 * w),
                        );
                    }
                }
                ATT_CHARGE => lane(
                    &mut pen,
                    o,
                    to,
                    BOSS_CHARGE,
                    BOSS_CHARGE_HALF_WIDTH,
                    w,
                    pulse,
                ),
                ATT_FAN => {
                    let n: i32 = if f.phase2 { 7 } else { 5 };
                    for k in 0..n {
                        let a = ang + (k as f32 - (n - 1) as f32 * 0.5) * BOSS_FAN_SPREAD;
                        let end = (
                            o.x + a.cos() * CENSOR_LANE,
                            o.y + a.sin() * CENSOR_LANE,
                            0.0,
                        );
                        let start = (o.x, o.y, 0.0);
                        pen.line(start, end, 5.0 * (1.0 - w) + 1.0, col(WAX, 0.08 + 0.10 * w));
                        pen.line(start, end, 1.3, col(WAX, 0.4 + 0.45 * w));
                    }
                }
                ATT_SUMMON => {
                    // Summons: wax seals open at the two flanks where clerks surface.
                    let side = Vec2::new(-to.y, to.x);
                    for s in [1.0f32, -1.0] {
                        let c = Vec2::new(o.x + side.x * s * 1.6, o.y + side.y * s * 1.6);
                        pen.arc(
                            c,
                            0.0,
                            0.55 * (0.4 + 0.6 * w),
                            0.0,
                            TAU,
                            0.0,
                            Some(col(WAX, 0.15 + 0.3 * w)),
                            Some((1.6, col(PEARL, 0.5))),
                        );
                    }
                }
                _ => {}
            },
        }
    }

    // Ground effects: ripples, dash wakes, strike arcs.
    for e in &snap.fx {
        let u = (e.age / e.duration.max(1e-3)).clamp(0.0, 1.0);
        let fade = 1.0 - u;
        let d = norm(e.dir, Vec2::new(1.0, 0.0));
        let ang = d.y.atan2(d.x);
        match e.kind {
            FxKind::Dash => {
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
            FxKind::Burst => {
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
            FxKind::Hit => {
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
            FxKind::Strike => {
                // Crescent swept across the strike reach.
                let reach = e.radius.max(0.5);
                let half = if e.duration > 0.25 {
                    FINISHER_HALF_ARC
                } else {
                    STRIKE_HALF_ARC
                };
                let sweep = half * (1.0 + 0.2 * ease_out(u));
                let head = ang - sweep + 2.0 * sweep * ease_out(u * 1.6);
                let tail = (head - 1.5).max(ang - sweep);
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
            // Reticle at the aim point; its inner arc refills as the cast cooldown elapses.
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
        // Dash recharge arc under the courier.
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
    for i in 0..16u32 {
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
            FxKind::Hit => {
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
            FxKind::Strike => {
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
            FxKind::Burst => {
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
            FxKind::Dash => {
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
        if !(f.hp < f.max_hp || f.kind == Kind::Postmaster) {
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
        pen.rect(x0 - 1.0, y0 - 1.0, w + 2.0, h + 2.0, col(PEARL, 0.55));
        pen.rect(x0, y0, w, h, col(INK, 0.9));
        pen.rect(x0, y0, w * k, h, col(WAX, 1.0));
    }

    // Telegraph glints above windup-ing enemies (a brass warning pip).
    for f in &snap.foes {
        if f.windup > 0.0 {
            let w = f.windup.clamp(0.0, 1.0);
            let blink = if (t * 18.0).sin() > 0.0 { 1.0 } else { 0.55 };
            let z = f.kind.height() / 52.0 + 0.25;
            pen.disc(f.pos.x, f.pos.y, z, 3.0 + 3.0 * w, col(BRASS, 0.9 * blink));
            pen.disc(f.pos.x, f.pos.y, z, 1.4, col(INK, 0.9));
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
    let fit = Fit::new(width, height);
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
    let foes: Vec<Foe> = world
        .enemies
        .iter()
        .map(|e| Foe {
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
            kind: match &e.kind {
                EffectKind::Strike => FxKind::Strike,
                EffectKind::Dash => FxKind::Dash,
                EffectKind::Hit => FxKind::Hit,
                EffectKind::Burst => FxKind::Burst,
            },
            pos: e.pos,
            dir: e.dir,
            age: e.age,
            duration: e.duration,
            radius: e.radius,
        })
        .collect();
    let combat = world.phase == Phase::Combat;
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
        bolts: bolts.clone(),
        fx: fx.clone(),
        aim,
        combat,
        chamber: world.chamber,
    });
    let front = Arc::clone(&ground);

    let mut root = div()
        .relative()
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .bg(rgb(INK))
        .child(
            img(arena_image())
                .absolute()
                .left(px(fit.ox))
                .top(px(fit.oy))
                .w(px(DESIGN_W * fit.s))
                .h(px(DESIGN_H * fit.s))
                .object_fit(ObjectFit::Fill),
        )
        .child(
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
                let bob = if combat {
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

/// Lunge corridor: a wax-red lane that floods from the origin as the windup fills.
fn lane(pen: &mut Pen, o: Vec2, dir: Vec2, len: f32, half_w: f32, w: f32, pulse: f32) {
    let n = Vec2::new(-dir.y, dir.x);
    let corner = |d: f32, s: f32| {
        (
            o.x + dir.x * d + n.x * s * half_w,
            o.y + dir.y * d + n.y * s * half_w,
            0.0,
        )
    };
    let quad = |d0: f32, d1: f32| {
        [
            corner(d0, -1.0),
            corner(d1, -1.0),
            corner(d1, 1.0),
            corner(d0, 1.0),
        ]
    };
    let full = quad(0.0, len);
    pen.path(
        &full,
        true,
        Some(col(WAX, 0.10 + 0.08 * w)),
        Some((1.6, col(WAX, 0.5 + 0.35 * pulse * w))),
    );
    let fill_pts = quad(0.0, len * w);
    pen.path(&fill_pts, true, Some(col(WAX, 0.28 + 0.35 * w)), None);
}
