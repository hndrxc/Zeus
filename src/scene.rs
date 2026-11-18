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

static ARENA_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena.png")));
static ARENA_VAULT_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena_vault.png")));
static ARENA_OFFICE_IMG: LazyLock<Arc<Image>> =
    LazyLock::new(|| png(include_bytes!("../assets/arena_office.png")));
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
    emerging: bool,
    radius: f32,
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
    marks: Vec<Mark>,
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
    if fit.ox > 0.0 {
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
    if fit.oy > 0.0 {
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
        let w = f.windup.max(f.shock).clamp(0.0, 1.0);
        if w > 0.0 {
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
    let foes: Vec<Foe> = world.enemies.iter().map(foe).collect();
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
        marks,
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
            img(arena_image(world.chamber))
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

    /// +X, +Y, -X, -Y: the four sprite directions.
    const DIRECTIONS: [Vec2; 4] = [
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, -1.0),
    ];

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
    fn every_warning_covers_its_damaging_ticks_in_all_four_directions() {
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
    /// Postmaster's fan. Rotated to each of the four directions, every hit
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
        let fit = Fit::new(DESIGN_W, DESIGN_H);
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
}
