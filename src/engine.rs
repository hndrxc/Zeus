//! DEAD LETTER / THE UNDERTOW POST — deterministic fixed-tick game simulation.
//!
//! Everything advances in `step` at 60 Hz. World space is the square arena
//! `[-ARENA, ARENA]^2`. `Input::movement` is a world-space direction (length is
//! clamped to 1), `Input::aim` is a world-space point. `strike`/`cast` auto-repeat
//! while held; `dash` is edge-triggered.

use std::ops::{Add, Mul, Sub};

pub const ARENA: f32 = 4.05;
pub const CHAMBERS: usize = 3;
/// Fixed simulation step in seconds.
pub const DT: f32 = 1.0 / 60.0;

const PLAYER_RADIUS: f32 = 0.28;
const PLAYER_SPEED: f32 = 4.6;
const PLAYER_HP: f32 = 100.0;
const DASH_SPEED: f32 = 14.0;
const DASH_TIME: f32 = 0.17;
const DASH_IFRAMES: f32 = 0.26;
const BASE_DASH_COOLDOWN: f32 = 0.9;
const BASE_CAST_COOLDOWN: f32 = 0.9;
const HIT_IFRAMES: f32 = 0.6;
const COMBO_WINDOW: f32 = 0.75;
const CAST_SPEED: f32 = 11.0;
const CAST_DAMAGE: f32 = 18.0;
const EMERGE_TIME: f32 = 0.8;
const LULL: f32 = 0.9;
/// Seconds a wave clerk's entry point is marked before it starts to emerge.
pub const SPAWN_WARNING: f32 = 0.9;
const HEAL_BETWEEN_CHAMBERS: f32 = 25.0;
/// Seconds the finished drill stays on screen before the run begins.
const DRILL_OUTRO: f32 = 1.0;

const S_MOVE: u8 = 0;
const S_WIND: u8 = 1;
const S_ACT: u8 = 2;
const S_REC: u8 = 3;
const S_TRANS: u8 = 4;
const S_EMERGE: u8 = 5;

pub const ATT_RING: u8 = 0;
pub const ATT_CHARGE: u8 = 1;
pub const ATT_FAN: u8 = 2;
pub const ATT_SUMMON: u8 = 3;

/// Chamber display names, 1-based.
pub fn chamber_name(chamber: usize) -> &'static str {
    match chamber {
        1 => "Sorting Floor",
        2 => "Dead Letter Vault",
        _ => "Postmaster's Office",
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn len_sq(self) -> f32 {
        self.dot(self)
    }
    pub fn len(self) -> f32 {
        self.len_sq().sqrt()
    }
    /// Unit vector, or zero when (nearly) zero length.
    pub fn norm(self) -> Vec2 {
        let l = self.len();
        if l < 1e-5 {
            Vec2::ZERO
        } else {
            self * (1.0 / l)
        }
    }
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }
    pub fn from_angle(a: f32) -> Vec2 {
        Vec2::new(a.cos(), a.sin())
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

fn clamp_arena(p: Vec2, r: f32) -> Vec2 {
    let m = ARENA - r;
    Vec2::new(p.x.clamp(-m, m), p.y.clamp(-m, m))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub movement: Vec2,
    pub aim: Vec2,
    pub strike: bool,
    pub cast: bool,
    pub dash: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Combat,
    Choice,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Bailiff,
    Censor,
    Postmaster,
}

impl EnemyKind {
    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Bailiff => 0.38,
            EnemyKind::Censor => 0.32,
            EnemyKind::Postmaster => 0.75,
        }
    }
}

/// A wave clerk marked on the floor at its entry point. It becomes an emerging
/// `Enemy` at the same position when the warning runs out.
#[derive(Clone, Debug)]
pub struct Arrival {
    pub kind: EnemyKind,
    pub pos: Vec2,
    timer: f32,
}

impl Arrival {
    /// Rising 0..=1 progress of the entry warning.
    pub fn progress(&self) -> f32 {
        (1.0 - self.timer / SPAWN_WARNING).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Strike,
    Dash,
    Hit,
    Burst,
}

/// The attack behind a hit on the courier; the defeat screen reports the lethal one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    BailiffLunge,
    CensorBolt,
    PostmasterRing,
    PostmasterCharge,
    PostmasterFan,
    /// The ring released as the Postmaster recovers from its half-hull stagger.
    PostmasterShockwave,
}

impl Blow {
    pub fn attacker(self) -> &'static str {
        match self {
            Blow::BailiffLunge => "Bailiff's lunge",
            Blow::CensorBolt => "Censor's ink bolt",
            Blow::PostmasterRing => "The Postmaster's ink ring",
            Blow::PostmasterCharge => "The Postmaster's charge",
            Blow::PostmasterFan => "The Postmaster's ink fan",
            Blow::PostmasterShockwave => "The Postmaster's stagger shockwave",
        }
    }

    /// One counterplay hint that holds for the attack as the engine resolves it.
    pub fn counterplay(self) -> &'static str {
        match self {
            Blow::BailiffLunge => {
                "Its aim locks just before the lunge. Dash sideways out of the red lane as it fills."
            }
            Blow::CensorBolt => {
                "Bolts fly straight down the marked line. Step off it, or strike to cut the ink out of the air."
            }
            Blow::PostmasterRing => {
                "The ring leaves gaps between its spokes. Stand in one, or dash through the wave as it passes."
            }
            Blow::PostmasterCharge => {
                "The charge runs the whole corridor until it meets a wall. Dash across the lane, not along it."
            }
            Blow::PostmasterFan => {
                "Fan bolts spread apart with distance. Keep back to widen the gaps, or strike them out of the air."
            }
            Blow::PostmasterShockwave => {
                "At half hull it staggers, then bursts a ring. Back away while it rears up and dash through the wave."
            }
        }
    }
}

/// Steps of the opening drill. Each advances when the engine performs that
/// action, whichever input scheme asked for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lesson {
    Strike,
    Cast,
    Dash,
    /// All three practised; the run begins after a short beat.
    Complete,
}

impl Lesson {
    /// Practised steps, in order.
    pub const STEPS: [Lesson; 3] = [Lesson::Strike, Lesson::Cast, Lesson::Dash];

    /// How many steps are already done.
    pub fn done(self) -> usize {
        match self {
            Lesson::Strike => 0,
            Lesson::Cast => 1,
            Lesson::Dash => 2,
            Lesson::Complete => 3,
        }
    }

    fn next(self) -> Option<Lesson> {
        match self {
            Lesson::Strike => Some(Lesson::Cast),
            Lesson::Cast => Some(Lesson::Dash),
            Lesson::Dash | Lesson::Complete => Some(Lesson::Complete),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boon {
    FrankingHammer,
    TidewaterReach,
    RegisteredMail,
    QuickDelivery,
    SlipstreamTag,
    PressureValve,
    ReturnReceipt,
    ExpressPost,
    UndertowLash,
}

impl Boon {
    pub const ALL: [Boon; 9] = [
        Boon::FrankingHammer,
        Boon::TidewaterReach,
        Boon::RegisteredMail,
        Boon::QuickDelivery,
        Boon::SlipstreamTag,
        Boon::PressureValve,
        Boon::ReturnReceipt,
        Boon::ExpressPost,
        Boon::UndertowLash,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Boon::FrankingHammer => "Franking Hammer",
            Boon::TidewaterReach => "Tidewater Reach",
            Boon::RegisteredMail => "Registered Mail",
            Boon::QuickDelivery => "Quick Delivery",
            Boon::SlipstreamTag => "Slipstream Tag",
            Boon::PressureValve => "Pressure Valve",
            Boon::ReturnReceipt => "Return Receipt",
            Boon::ExpressPost => "Express Post",
            Boon::UndertowLash => "Undertow Lash",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Boon::FrankingHammer => "Strikes hit 35% harder.",
            Boon::TidewaterReach => "Strikes reach 30% farther.",
            Boon::RegisteredMail => "Cast letters pierce every target and hit 25% harder.",
            Boon::QuickDelivery => "Cast cooldown reduced by 35%.",
            Boon::SlipstreamTag => "Dash recovers 30% faster and shields you longer.",
            Boon::PressureValve => "+30 max hull and restore 30 now.",
            Boon::ReturnReceipt => "Each cancelled automaton restores 6 hull.",
            Boon::ExpressPost => "Move 15% faster.",
            Boon::UndertowLash => "Dashing through an enemy rips it for 14 and shoves it back.",
        }
    }
}

/// Accumulated boon modifiers (all multipliers start at 1).
#[derive(Clone, Copy, Debug)]
pub struct Mods {
    pub strike: f32,
    pub reach: f32,
    pub cast: f32,
    pub cast_cooldown: f32,
    pub dash_cooldown: f32,
    pub dash_iframes: f32,
    pub speed: f32,
    pub pierce: bool,
    pub lifesteal: f32,
    pub lash: bool,
}

impl Default for Mods {
    fn default() -> Self {
        Mods {
            strike: 1.0,
            reach: 1.0,
            cast: 1.0,
            cast_cooldown: 1.0,
            dash_cooldown: 1.0,
            dash_iframes: 0.0,
            speed: 1.0,
            pierce: false,
            lifesteal: 0.0,
            lash: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Vec2,
    pub facing: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    /// Seconds remaining.
    pub dash_cooldown: f32,
    /// Seconds remaining.
    pub cast_cooldown: f32,
    pub dash_cooldown_max: f32,
    pub cast_cooldown_max: f32,
    pub flash: f32,
    /// Next strike's chain step (0, 1, 2 = finisher).
    pub combo: u8,
    /// Seconds remaining of invulnerability.
    pub invuln: f32,
    combo_timer: f32,
    strike_cd: f32,
    pub stride_phase: f32,
    pub walking: bool,
    pub attack_pose: Option<(EffectKind, Vec2, f32)>,
    dash_time: f32,
    dash_dir: Vec2,
}

impl Player {
    pub fn dash_pose(&self) -> Option<(Vec2, f32)> {
        self.dashing()
            .then_some((self.dash_dir, 1.0 - self.dash_time / DASH_TIME))
    }

    fn new() -> Self {
        Player {
            pos: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.0),
            hp: PLAYER_HP,
            max_hp: PLAYER_HP,
            dash_cooldown: 0.0,
            cast_cooldown: 0.0,
            dash_cooldown_max: BASE_DASH_COOLDOWN,
            cast_cooldown_max: BASE_CAST_COOLDOWN,
            flash: 0.0,
            combo: 0,
            invuln: 0.0,
            combo_timer: 0.0,
            strike_cd: 0.0,
            stride_phase: 0.0,
            walking: false,
            attack_pose: None,
            dash_time: 0.0,
            dash_dir: Vec2::new(1.0, 0.0),
        }
    }
    pub fn dashing(&self) -> bool {
        self.dash_time > 0.0
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    /// 0 when idle, otherwise rising 0..=1 progress of the telegraph.
    pub windup: f32,
    /// 1 on hit, decays to 0.
    pub flash: f32,
    /// Locked attack direction (valid during windup / action): where the lunge,
    /// shot or charge will go.
    pub dir: Vec2,
    /// Boss entered its second phase.
    pub phase2: bool,
    /// Still rising out of the floor; harmless and untouchable.
    pub emerging: bool,
    id: u32,
    stride_phase: f32,
    moving: bool,
    state: u8,
    timer: f32,
    dur: f32,
    cd: f32,
    strafe: f32,
    /// Boss attack in progress (ATT_*), valid during windup/action.
    pub attack: u8,
    attack_index: u32,
    contact_done: bool,
    dash_stamp: u32,
}

impl Enemy {
    /// Layout of the Blender bake: ready, eight strides, then five samples per attack.
    pub fn animation_frame(&self) -> usize {
        let attack = if self.kind == EnemyKind::Postmaster {
            self.attack as usize
        } else {
            0
        };
        let start = 9 + 5 * attack;
        let elapsed = if self.dur > 0.0 {
            (1.0 - self.timer / self.dur).clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self.state {
            S_MOVE if self.moving => 1 + (self.stride_phase * 8.0) as usize,
            S_WIND => start + usize::from(self.windup >= 0.48),
            S_ACT => start + 2 + usize::from(elapsed >= 0.55),
            S_REC if elapsed < 0.82 => {
                // Projectile and summon releases enter recovery on their firing tick.
                let contact = self.kind == EnemyKind::Bailiff
                    || (self.kind == EnemyKind::Postmaster && self.attack == ATT_CHARGE);
                start
                    + if elapsed < 0.22 && !contact {
                        2
                    } else if elapsed < 0.48 {
                        3
                    } else {
                        4
                    }
            }
            S_TRANS => 29 + ((elapsed * 5.0) as usize).min(4),
            _ => 0,
        }
    }

    pub fn radius(&self) -> f32 {
        self.kind.radius()
    }
    fn begin(&mut self, state: u8, dur: f32) {
        self.state = state;
        self.dur = dur;
        self.timer = dur;
        // A windup state shows its cue from its very first tick.
        self.windup = if state == S_WIND { 0.02 } else { 0.0 };
    }
    fn wind_progress(&mut self) {
        self.windup = (1.0 - self.timer / self.dur).clamp(0.02, 1.0);
    }
    fn hittable(&self) -> bool {
        self.hp > 0.0 && self.state != S_EMERGE && self.state != S_TRANS
    }

    /// Angle of the first bolt in the boss ring being wound up; alternate rings
    /// rotate by 0.2 rad so the gaps move.
    pub fn ring_offset(&self) -> f32 {
        if self.attack_index % 2 == 0 {
            0.0
        } else {
            0.2
        }
    }

    /// Rising 0..=1 progress of the half-hull stagger, which ends in a ring of
    /// `SHOCKWAVE_BOLTS` bolts; 0 at any other time.
    pub fn shockwave(&self) -> f32 {
        if self.state == S_TRANS && self.dur > 0.0 {
            (1.0 - self.timer / self.dur).clamp(0.02, 1.0)
        } else {
            0.0
        }
    }
}

/// Bolts in the ring that ends the Postmaster's half-hull stagger.
pub const SHOCKWAVE_BOLTS: u32 = 14;

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: Vec2,
    pub radius: f32,
    pub hostile: bool,
    pub velocity: Vec2,
    pub damage: f32,
    /// Set on hostile shots: the attack that fired it.
    pub blow: Option<Blow>,
    life: f32,
    pierce: bool,
    hit_mask: u64,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub kind: EffectKind,
    pub pos: Vec2,
    pub dir: Vec2,
    pub age: f32,
    pub duration: f32,
    pub radius: f32,
}

pub struct Game {
    pub phase: Phase,
    /// 1-based.
    pub chamber: usize,
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub shots: Vec<Shot>,
    pub effects: Vec<Effect>,
    pub time: f32,
    pub kills: u32,
    /// Boons taken this run.
    pub seal_count: u32,
    pub choices: [Boon; 3],
    pub mods: Mods,
    /// The hit that emptied the courier's hull this run, if any.
    pub killing_blow: Option<Blow>,
    /// The opening drill's current step; `None` once the run proper is under way.
    pub lesson: Option<Lesson>,
    drill_outro: f32,
    owned: u16,
    seed: u64,
    run: u64,
    rng: u64,
    wave: usize,
    lull: f32,
    next_id: u32,
    live: usize,
    pending: Vec<Enemy>,
    /// Wave clerks still marked on the floor, in spawn order.
    pub arrivals: Vec<Arrival>,
    prev_dash: bool,
    dash_stamp: u32,
    wipe_shots: bool,
}

fn waves_in(chamber: usize) -> usize {
    match chamber {
        1 => 2,
        2 => 3,
        _ => 1,
    }
}

fn wave_kinds(chamber: usize, wave: usize) -> &'static [EnemyKind] {
    use EnemyKind::{Bailiff as B, Censor as C, Postmaster as P};
    match (chamber, wave) {
        (1, 0) => &[B, B],
        (1, _) => &[B, C, B],
        (2, 0) => &[B, B, C],
        (2, 1) => &[C, C, B],
        (2, _) => &[B, B, B, C],
        _ => &[P],
    }
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Game {
            phase: Phase::Ready,
            chamber: 1,
            player: Player::new(),
            enemies: Vec::with_capacity(12),
            shots: Vec::with_capacity(64),
            effects: Vec::with_capacity(32),
            time: 0.0,
            kills: 0,
            seal_count: 0,
            choices: [
                Boon::FrankingHammer,
                Boon::TidewaterReach,
                Boon::RegisteredMail,
            ],
            mods: Mods::default(),
            killing_blow: None,
            lesson: None,
            drill_outro: 0.0,
            owned: 0,
            seed,
            run: 0,
            rng: 1,
            wave: 0,
            lull: 0.8,
            next_id: 0,
            live: 0,
            pending: Vec::with_capacity(4),
            arrivals: Vec::with_capacity(4),
            prev_dash: false,
            dash_stamp: 0,
            wipe_shots: false,
        };
        g.reseed();
        g
    }

    fn reseed(&mut self) {
        self.rng = (self.seed ^ self.run.wrapping_mul(0xD1B5_4A32_D192_ED03))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            | 1;
        for _ in 0..4 {
            self.rand();
        }
    }

    /// Begin (or restart) a run at chamber 1.
    pub fn start(&mut self) {
        self.run += 1;
        self.lesson = None;
        self.fresh_floor();
    }

    /// Begin a run with the opening drill on the Sorting Floor. Waves hold
    /// until the drill is completed or skipped.
    pub fn start_drill(&mut self) {
        self.start();
        self.lesson = Some(Lesson::Strike);
        self.drill_outro = DRILL_OUTRO;
    }

    /// End the drill now; the run then plays out exactly as from `start`.
    pub fn skip_drill(&mut self) {
        if self.lesson.is_some() && self.phase == Phase::Combat {
            self.lesson = None;
            self.fresh_floor();
        }
    }

    /// Reset everything a run depends on for the current `run` number. Nothing
    /// done before this (such as the drill) can change what follows.
    fn fresh_floor(&mut self) {
        self.reseed();
        self.phase = Phase::Combat;
        self.chamber = 1;
        self.player = Player::new();
        self.enemies.clear();
        self.shots.clear();
        self.effects.clear();
        self.pending.clear();
        self.arrivals.clear();
        self.time = 0.0;
        self.kills = 0;
        self.seal_count = 0;
        self.mods = Mods::default();
        self.killing_blow = None;
        self.owned = 0;
        self.wave = 0;
        self.lull = 0.8;
        self.next_id = 0;
        self.live = 0;
        self.prev_dash = false;
        self.dash_stamp = 0;
        self.wipe_shots = false;
    }

    /// Credit `done` toward the drill if it is the action being asked for.
    fn practised(&mut self, done: Lesson) {
        if self.lesson == Some(done) {
            self.lesson = done.next();
        }
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 40) as f32 / 16_777_216.0
    }

    pub fn step(&mut self, input: Input) {
        if self.phase != Phase::Combat {
            self.age_effects();
            return;
        }
        self.time += DT;
        self.update_player(&input);
        self.update_enemies();
        self.update_shots();
        self.resolve_deaths();
        self.age_effects();
        if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = Phase::Lost;
            return;
        }
        if self.lesson == Some(Lesson::Complete) {
            self.drill_outro -= DT;
            if self.drill_outro <= 0.0 {
                self.skip_drill();
            }
            return;
        }
        if self.lesson.is_some() {
            return;
        }
        self.advance_waves();
    }

    /// Pick boon `index` (0..3) during `Phase::Choice` and enter the next chamber.
    pub fn choose(&mut self, index: usize) {
        if self.phase != Phase::Choice || index >= 3 {
            return;
        }
        let boon = self.choices[index];
        self.apply_boon(boon);
        self.owned |= 1 << (boon as u16);
        self.seal_count += 1;
        self.chamber += 1;
        self.player.pos = Vec2::ZERO;
        self.player.hp = (self.player.hp + HEAL_BETWEEN_CHAMBERS).min(self.player.max_hp);
        self.player.dash_time = 0.0;
        self.player.walking = false;
        self.player.attack_pose = None;
        self.player.invuln = 0.5;
        self.enemies.clear();
        self.pending.clear();
        self.arrivals.clear();
        self.shots.clear();
        self.effects.clear();
        self.wave = 0;
        self.lull = 0.8;
        self.phase = Phase::Combat;
    }

    fn apply_boon(&mut self, boon: Boon) {
        let m = &mut self.mods;
        match boon {
            Boon::FrankingHammer => m.strike *= 1.35,
            Boon::TidewaterReach => m.reach *= 1.3,
            Boon::RegisteredMail => {
                m.pierce = true;
                m.cast *= 1.25;
            }
            Boon::QuickDelivery => m.cast_cooldown *= 0.65,
            Boon::SlipstreamTag => {
                m.dash_cooldown *= 0.7;
                m.dash_iframes += 0.12;
            }
            Boon::PressureValve => {
                self.player.max_hp += 30.0;
                self.player.hp += 30.0;
            }
            Boon::ReturnReceipt => m.lifesteal += 6.0,
            Boon::ExpressPost => m.speed *= 1.15,
            Boon::UndertowLash => m.lash = true,
        }
        self.player.dash_cooldown_max = BASE_DASH_COOLDOWN * self.mods.dash_cooldown;
        self.player.cast_cooldown_max = BASE_CAST_COOLDOWN * self.mods.cast_cooldown;
        self.player.dash_cooldown = self.player.dash_cooldown.min(self.player.dash_cooldown_max);
        self.player.cast_cooldown = self.player.cast_cooldown.min(self.player.cast_cooldown_max);
    }

    fn roll_choices(&mut self) {
        let mut pool = [Boon::FrankingHammer; 9];
        let mut n = 0;
        for b in Boon::ALL {
            if self.owned & (1 << (b as u16)) == 0 {
                pool[n] = b;
                n += 1;
            }
        }
        for slot in 0..3 {
            let remaining = n - slot.min(n);
            if remaining == 0 {
                self.choices[slot] = pool[0];
                continue;
            }
            let pick = slot + ((self.rand() * remaining as f32) as usize).min(remaining - 1);
            pool.swap(slot, pick);
            self.choices[slot] = pool[slot];
        }
    }

    // ---------------------------------------------------------------- player

    fn update_player(&mut self, input: &Input) {
        let p = &mut self.player;
        p.flash = (p.flash - DT / 0.3).max(0.0);
        p.dash_cooldown = (p.dash_cooldown - DT).max(0.0);
        p.cast_cooldown = (p.cast_cooldown - DT).max(0.0);
        p.strike_cd = (p.strike_cd - DT).max(0.0);
        p.invuln = (p.invuln - DT).max(0.0);
        if p.combo_timer > 0.0 {
            p.combo_timer -= DT;
            if p.combo_timer <= 0.0 {
                p.combo = 0;
            }
        }

        p.walking = false;
        if let Some((_, _, ref mut progress)) = p.attack_pose {
            *progress += DT / 0.27;
            if *progress >= 1.0 {
                p.attack_pose = None;
            }
        }
        let mut mv = input.movement;
        let ml = mv.len();
        if ml > 1.0 {
            mv = mv * (1.0 / ml);
        }
        let to_aim = input.aim - p.pos;
        if to_aim.len_sq() > 0.01 {
            p.facing = to_aim.norm();
        } else if mv.len_sq() > 0.01 && !input.strike && !input.cast {
            p.facing = mv.norm();
        }

        let dash_edge = input.dash && !self.prev_dash;
        self.prev_dash = input.dash;
        if dash_edge && p.dash_cooldown <= 0.0 && p.dash_time <= 0.0 {
            let d = if mv.len_sq() > 0.01 {
                mv.norm()
            } else {
                p.facing
            };
            p.dash_dir = d;
            p.dash_time = DASH_TIME;
            p.dash_cooldown = p.dash_cooldown_max;
            p.invuln = p.invuln.max(DASH_IFRAMES + self.mods.dash_iframes);
            self.dash_stamp += 1;
            self.effects.push(Effect {
                kind: EffectKind::Dash,
                pos: p.pos,
                dir: d,
                age: 0.0,
                duration: 0.3,
                radius: 0.7,
            });
            self.practised(Lesson::Dash);
        }

        if self.player.dash_time > 0.0 {
            let p = &mut self.player;
            p.dash_time -= DT;
            p.pos = clamp_arena(p.pos + p.dash_dir * (DASH_SPEED * DT), PLAYER_RADIUS);
            if self.mods.lash {
                self.dash_lash();
            }
            return;
        }

        let p = &mut self.player;
        let previous = p.pos;
        p.pos = clamp_arena(
            p.pos + mv * (PLAYER_SPEED * self.mods.speed * DT),
            PLAYER_RADIUS,
        );

        let distance = (p.pos - previous).len();
        p.walking = distance > 0.0001;
        p.stride_phase = if p.walking {
            (p.stride_phase + distance / 1.8).fract()
        } else {
            0.0
        };

        if input.strike && self.player.strike_cd <= 0.0 {
            self.do_strike();
        }
        if input.cast && self.player.cast_cooldown <= 0.0 {
            self.do_cast();
        }
    }

    fn dash_lash(&mut self) {
        let pp = self.player.pos;
        let dir = self.player.dash_dir;
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            if e.dash_stamp == self.dash_stamp || !e.hittable() {
                continue;
            }
            if (e.pos - pp).len() < e.radius() + PLAYER_RADIUS + 0.2 {
                self.enemies[i].dash_stamp = self.dash_stamp;
                self.damage_enemy(i, 14.0, dir, 0.9);
            }
        }
    }

    fn do_strike(&mut self) {
        let step = if self.player.combo_timer > 0.0 {
            self.player.combo
        } else {
            0
        };
        let (dmg, range, cos_arc, cd, kb) = match step {
            0 => (14.0, 1.55, 0.5, 0.30, 0.3),
            1 => (15.0, 1.6, 0.5, 0.30, 0.35),
            _ => (28.0, 1.95, 0.0, 0.55, 1.0),
        };
        let dmg = dmg * self.mods.strike;
        let range = range * self.mods.reach;
        let facing = self.player.facing;
        {
            let p = &mut self.player;
            p.combo = (step + 1) % 3;
            p.combo_timer = COMBO_WINDOW;
            p.strike_cd = cd;
            p.attack_pose = Some((EffectKind::Strike, facing, 0.0));
            p.pos = clamp_arena(p.pos + facing * 0.16, PLAYER_RADIUS);
        }
        let pp = self.player.pos;
        self.effects.push(Effect {
            kind: EffectKind::Strike,
            pos: pp,
            dir: facing,
            age: 0.0,
            duration: if step == 2 { 0.28 } else { 0.2 },
            radius: range,
        });
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            if !e.hittable() {
                continue;
            }
            let to = e.pos - pp;
            let d = to.len();
            if d <= range + e.radius() && (d < 0.01 || to.dot(facing) >= cos_arc * d) {
                self.damage_enemy(i, dmg, facing, kb);
            }
        }
        // The blade also cuts hostile ink out of the air.
        for s in self.shots.iter_mut() {
            if !s.hostile || s.life <= 0.0 {
                continue;
            }
            let to = s.pos - pp;
            let d = to.len();
            if d <= range + s.radius && (d < 0.01 || to.dot(facing) >= cos_arc * d) {
                s.life = 0.0;
                self.effects.push(Effect {
                    kind: EffectKind::Hit,
                    pos: s.pos,
                    dir: facing,
                    age: 0.0,
                    duration: 0.2,
                    radius: 0.3,
                });
            }
        }
        self.practised(Lesson::Strike);
    }

    fn do_cast(&mut self) {
        let p = &mut self.player;
        p.cast_cooldown = p.cast_cooldown_max;
        p.attack_pose = Some((EffectKind::Burst, p.facing, 0.0));
        let dir = p.facing;
        self.shots.push(Shot {
            pos: p.pos + dir * 0.5,
            radius: 0.2,
            hostile: false,
            velocity: dir * CAST_SPEED,
            damage: CAST_DAMAGE * self.mods.cast,
            blow: None,
            life: 1.3,
            pierce: self.mods.pierce,
            hit_mask: 0,
        });
        self.practised(Lesson::Cast);
    }

    /// Returns true when the damage actually landed.
    fn hurt_player(&mut self, dmg: f32, blow: Blow) -> bool {
        let p = &mut self.player;
        if p.invuln > 0.0 || p.dash_time > 0.0 || p.hp <= 0.0 {
            return false;
        }
        p.hp -= dmg;
        p.flash = 1.0;
        p.invuln = HIT_IFRAMES;
        let pos = p.pos;
        if p.hp <= 0.0 {
            self.killing_blow = Some(blow);
        }
        self.effects.push(Effect {
            kind: EffectKind::Hit,
            pos,
            dir: Vec2::ZERO,
            age: 0.0,
            duration: 0.3,
            radius: 0.5,
        });
        true
    }

    fn damage_enemy(&mut self, i: usize, dmg: f32, dir: Vec2, knock: f32) -> bool {
        let e = &mut self.enemies[i];
        if !e.hittable() {
            return false;
        }
        e.hp -= dmg;
        e.flash = 1.0;
        let boss = e.kind == EnemyKind::Postmaster;
        if !(boss && e.state == S_ACT) {
            let k = if boss { knock * 0.2 } else { knock };
            e.pos = clamp_arena(e.pos + dir * k, e.radius());
        }
        let (pos, r) = (e.pos, e.radius());
        let mut burst = false;
        if boss && !e.phase2 && e.hp > 0.0 && e.hp <= e.max_hp * 0.5 {
            e.phase2 = true;
            e.begin(S_TRANS, 1.3);
            burst = true;
        }
        self.effects.push(Effect {
            kind: EffectKind::Hit,
            pos,
            dir,
            age: 0.0,
            duration: 0.25,
            radius: r,
        });
        if burst {
            self.effects.push(Effect {
                kind: EffectKind::Burst,
                pos,
                dir: Vec2::ZERO,
                age: 0.0,
                duration: 1.3,
                radius: 2.2,
            });
            // Stagger wipes the ink off the floor (applied in update_shots).
            self.wipe_shots = true;
        }
        true
    }

    // --------------------------------------------------------------- enemies

    fn make_enemy(&mut self, kind: EnemyKind, pos: Vec2) -> Enemy {
        let scale = 1.0 + 0.15 * (self.chamber.saturating_sub(1)) as f32;
        let hp = match kind {
            EnemyKind::Bailiff => 40.0 * scale,
            EnemyKind::Censor => 26.0 * scale,
            EnemyKind::Postmaster => 380.0,
        };
        let id = self.next_id;
        self.next_id += 1;
        let strafe = if self.rand() < 0.5 { -1.0 } else { 1.0 };
        let mut e = Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            windup: 0.0,
            flash: 0.0,
            dir: Vec2::ZERO,
            phase2: false,
            emerging: true,
            id,
            stride_phase: 0.0,
            moving: false,
            state: S_EMERGE,
            timer: EMERGE_TIME,
            dur: EMERGE_TIME,
            cd: 0.0,
            strafe,
            attack: 0,
            attack_index: 0,
            contact_done: false,
            dash_stamp: 0,
        };
        e.begin(S_EMERGE, EMERGE_TIME);
        self.effects.push(Effect {
            kind: EffectKind::Burst,
            pos,
            dir: Vec2::ZERO,
            age: 0.0,
            duration: EMERGE_TIME,
            radius: e.radius() + 0.5,
        });
        e
    }

    fn pick_spawn(&mut self, r: f32) -> Vec2 {
        let mut best = Vec2::new(2.5, -2.5);
        for _ in 0..10 {
            let a = self.rand() * std::f32::consts::TAU;
            let rad = 2.4 + self.rand() * 1.3;
            let c = clamp_arena(Vec2::from_angle(a) * rad, r + 0.1);
            best = c;
            let mut ok = (c - self.player.pos).len() >= 3.0;
            let taken = self
                .enemies
                .iter()
                .chain(self.pending.iter())
                .map(|e| e.pos);
            for p in taken.chain(self.arrivals.iter().map(|a| a.pos)) {
                if (p - c).len() < 1.1 {
                    ok = false;
                }
            }
            if ok {
                break;
            }
        }
        best
    }

    fn spawn_wave(&mut self) {
        for &kind in wave_kinds(self.chamber, self.wave) {
            let r = match kind {
                EnemyKind::Postmaster => 0.75,
                _ => 0.38,
            };
            let pos = self.pick_spawn(r);
            self.arrivals.push(Arrival {
                kind,
                pos,
                timer: SPAWN_WARNING,
            });
        }
        self.wave += 1;
    }

    /// Count down marked entry points; each expired one emerges where it was marked.
    fn land_arrivals(&mut self) {
        let mut i = 0;
        while i < self.arrivals.len() {
            self.arrivals[i].timer -= DT;
            if self.arrivals[i].timer > 0.0 {
                i += 1;
                continue;
            }
            let a = self.arrivals.remove(i);
            let e = self.make_enemy(a.kind, a.pos);
            self.enemies.push(e);
        }
    }

    fn update_enemies(&mut self) {
        self.live = self.enemies.len();
        let mut enemies = std::mem::take(&mut self.enemies);
        for e in enemies.iter_mut() {
            if e.hp > 0.0 {
                let before = e.pos;
                if e.state == S_MOVE || e.state == S_EMERGE {
                    let target = self.player.pos - e.pos;
                    if target.len_sq() > 0.0001 {
                        e.dir = target.norm();
                    }
                }
                self.step_enemy(e);
                let distance = (e.pos - before).len();
                e.moving = e.state == S_MOVE && distance > 0.0001;
                if e.moving {
                    e.stride_phase = (e.stride_phase + distance / 1.25).fract();
                }
            }
        }
        // Separation between enemies and from the player's body.
        let pp = self.player.pos;
        for i in 0..enemies.len() {
            let (head, tail) = enemies.split_at_mut(i + 1);
            let a = &mut head[i];
            for b in tail.iter_mut() {
                let d = b.pos - a.pos;
                let min = (a.radius() + b.radius()) * 0.9;
                let l = d.len();
                if l < min {
                    let n = if l < 1e-4 {
                        Vec2::new(1.0, 0.0)
                    } else {
                        d * (1.0 / l)
                    };
                    let push = (min - l) * 0.5;
                    a.pos = a.pos - n * push;
                    b.pos = b.pos + n * push;
                }
            }
            if a.state != S_EMERGE {
                let d = a.pos - pp;
                let min = a.radius() + PLAYER_RADIUS * 0.8;
                let l = d.len();
                if l < min {
                    let n = if l < 1e-4 {
                        Vec2::new(1.0, 0.0)
                    } else {
                        d * (1.0 / l)
                    };
                    a.pos = a.pos + n * (min - l);
                }
            }
            a.pos = clamp_arena(a.pos, a.radius());
        }
        self.enemies = enemies;
        self.enemies.append(&mut self.pending);
    }

    fn fire(&mut self, pos: Vec2, dir: Vec2, speed: f32, dmg: f32, radius: f32, blow: Blow) {
        self.shots.push(Shot {
            pos,
            radius,
            hostile: true,
            velocity: dir * speed,
            damage: dmg,
            blow: Some(blow),
            life: 4.0,
            pierce: false,
            hit_mask: 0,
        });
    }

    fn step_enemy(&mut self, e: &mut Enemy) {
        e.flash = (e.flash - DT / 0.2).max(0.0);
        e.cd = (e.cd - DT).max(0.0);
        e.timer -= DT;
        match e.state {
            S_EMERGE => {
                if e.timer <= 0.0 {
                    e.emerging = false;
                    e.begin(S_MOVE, 0.0);
                    e.cd = 0.6;
                }
                return;
            }
            S_TRANS => {
                if e.timer <= 0.0 {
                    e.begin(S_MOVE, 0.0);
                    e.cd = 0.8;
                    let pos = e.pos;
                    self.ring(
                        pos,
                        SHOCKWAVE_BOLTS,
                        0.0,
                        4.6,
                        9.0,
                        Blow::PostmasterShockwave,
                    );
                }
                return;
            }
            _ => {}
        }
        match e.kind {
            EnemyKind::Bailiff => self.step_bailiff(e),
            EnemyKind::Censor => self.step_censor(e),
            EnemyKind::Postmaster => self.step_boss(e),
        }
    }

    fn ring(&mut self, pos: Vec2, n: u32, offset: f32, speed: f32, dmg: f32, blow: Blow) {
        for k in 0..n {
            let a = offset + k as f32 * std::f32::consts::TAU / n as f32;
            self.fire(pos, Vec2::from_angle(a), speed, dmg, 0.2, blow);
        }
    }

    fn step_bailiff(&mut self, e: &mut Enemy) {
        let ch = self.chamber.saturating_sub(1) as f32;
        let to = self.player.pos - e.pos;
        let dist = to.len();
        let dir = to.norm();
        match e.state {
            S_MOVE => {
                e.pos = e.pos + dir * (2.0 * (1.0 + 0.1 * ch) * DT);
                if dist < 2.7 && e.cd <= 0.0 {
                    e.begin(S_WIND, 0.7 - 0.05 * ch);
                    e.dir = dir;
                }
            }
            S_WIND => {
                if e.timer > 0.18 {
                    e.dir = dir;
                }
                e.wind_progress();
                if e.timer <= 0.0 {
                    e.begin(S_ACT, 0.28);
                    e.contact_done = false;
                }
            }
            S_ACT => {
                e.pos = e.pos + e.dir * (9.5 * DT);
                if !e.contact_done && dist < e.radius() + PLAYER_RADIUS + 0.1 {
                    e.contact_done = true;
                    self.hurt_player(14.0, Blow::BailiffLunge);
                }
                if e.timer <= 0.0 {
                    e.begin(S_REC, 0.75);
                }
            }
            _ => {
                if e.timer <= 0.0 {
                    e.begin(S_MOVE, 0.0);
                    e.cd = 1.1;
                }
            }
        }
    }

    fn step_censor(&mut self, e: &mut Enemy) {
        let ch = self.chamber.saturating_sub(1) as f32;
        let to = self.player.pos - e.pos;
        let dist = to.len();
        let dir = to.norm();
        match e.state {
            S_MOVE => {
                let want = if dist < 3.0 {
                    dir * -1.0
                } else if dist > 4.6 {
                    dir
                } else {
                    dir.perp() * e.strafe
                };
                let before = e.pos;
                e.pos = clamp_arena(e.pos + want * (1.7 * DT), e.radius());
                if (e.pos - before - want * (1.7 * DT)).len_sq() > 1e-6 {
                    e.strafe = -e.strafe;
                }
                if e.cd <= 0.0 && dist < 7.0 {
                    e.begin(S_WIND, 0.85 - 0.07 * ch);
                    e.dir = dir;
                }
            }
            S_WIND => {
                if e.timer > 0.25 {
                    e.dir = dir;
                }
                e.wind_progress();
                if e.timer <= 0.0 {
                    let n = if self.chamber >= 2 { 3 } else { 1 };
                    let base = e.dir.angle();
                    for k in 0..n {
                        let off = (k as f32 - (n - 1) as f32 * 0.5) * 0.32;
                        let dir = Vec2::from_angle(base + off);
                        self.fire(e.pos, dir, 4.8, 9.0, 0.17, Blow::CensorBolt);
                    }
                    e.begin(S_REC, 0.5);
                }
            }
            _ => {
                if e.timer <= 0.0 {
                    e.begin(S_MOVE, 0.0);
                    e.cd = 1.4 + self.rand() * 0.8;
                    e.strafe = -e.strafe;
                }
            }
        }
    }

    fn step_boss(&mut self, e: &mut Enemy) {
        let to = self.player.pos - e.pos;
        let dist = to.len();
        let dir = to.norm();
        match e.state {
            S_MOVE => {
                let speed = if e.phase2 { 1.5 } else { 1.0 };
                if dist > 1.8 {
                    e.pos = e.pos + dir * (speed * DT);
                }
                if e.cd <= 0.0 {
                    let cycle = if e.phase2 { 4 } else { 3 };
                    let att = (e.attack_index % cycle) as u8;
                    e.attack_index += 1;
                    e.attack = att;
                    e.dir = dir;
                    let dur = match att {
                        ATT_RING => 1.0,
                        ATT_CHARGE => 0.9,
                        ATT_FAN => 0.8,
                        _ => 1.0,
                    };
                    e.begin(S_WIND, dur);
                }
            }
            S_WIND => {
                if e.timer > 0.2 && e.attack != ATT_RING {
                    e.dir = dir;
                }
                e.wind_progress();
                if e.timer <= 0.0 {
                    self.boss_attack(e, dir);
                }
            }
            S_ACT => {
                let before = e.pos;
                e.pos = clamp_arena(e.pos + e.dir * (7.5 * DT), e.radius());
                let hit_wall = (e.pos - before - e.dir * (7.5 * DT)).len_sq() > 1e-6;
                if !e.contact_done && dist < e.radius() + PLAYER_RADIUS + 0.1 {
                    e.contact_done = true;
                    self.hurt_player(22.0, Blow::PostmasterCharge);
                }
                if e.timer <= 0.0 || hit_wall {
                    e.begin(S_REC, 1.0);
                }
            }
            _ => {
                if e.timer <= 0.0 {
                    e.begin(S_MOVE, 0.0);
                    e.cd = if e.phase2 { 0.9 } else { 1.4 };
                }
            }
        }
    }

    fn boss_attack(&mut self, e: &mut Enemy, dir: Vec2) {
        let phase2 = e.phase2;
        match e.attack {
            ATT_RING => {
                let n = if phase2 { 18 } else { 12 };
                let off = e.ring_offset();
                self.ring(e.pos, n, off, 4.2, 9.0, Blow::PostmasterRing);
                e.begin(S_REC, 0.9);
            }
            ATT_CHARGE => {
                e.begin(S_ACT, 0.55);
                e.contact_done = false;
            }
            ATT_FAN => {
                let n: u32 = if phase2 { 7 } else { 5 };
                let base = e.dir.angle();
                for k in 0..n {
                    let off = (k as f32 - (n - 1) as f32 * 0.5) * 0.26;
                    let dir = Vec2::from_angle(base + off);
                    self.fire(e.pos, dir, 5.0, 9.0, 0.17, Blow::PostmasterFan);
                }
                e.begin(S_REC, 0.8);
            }
            _ => {
                let side = dir.perp();
                if self.live + self.pending.len() < 5 {
                    for (k, kind) in [EnemyKind::Bailiff, EnemyKind::Censor]
                        .into_iter()
                        .enumerate()
                    {
                        let s = if k == 0 { 1.0 } else { -1.0 };
                        let pos = clamp_arena(e.pos + side * (s * 1.6), 0.4);
                        let en = self.make_enemy(kind, pos);
                        self.pending.push(en);
                    }
                }
                e.begin(S_REC, 1.0);
            }
        }
    }

    // ---------------------------------------------------------------- shots

    fn update_shots(&mut self) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|s| {
            s.pos = s.pos + s.velocity * DT;
            s.life -= DT;
            if s.life <= 0.0 || s.pos.x.abs() > ARENA + 0.6 || s.pos.y.abs() > ARENA + 0.6 {
                return false;
            }
            if let Some(blow) = s.blow {
                if (s.pos - self.player.pos).len() < s.radius + PLAYER_RADIUS * 0.85
                    && self.hurt_player(s.damage, blow)
                {
                    return false;
                }
                return true;
            }
            let dir = s.velocity.norm();
            for i in 0..self.enemies.len() {
                let e = &self.enemies[i];
                let bit = 1u64 << (e.id % 64);
                if s.hit_mask & bit != 0 || !e.hittable() {
                    continue;
                }
                if (e.pos - s.pos).len() < s.radius + e.radius() {
                    s.hit_mask |= bit;
                    self.damage_enemy(i, s.damage, dir, 0.15);
                    if !s.pierce {
                        return false;
                    }
                }
            }
            true
        });
        if self.wipe_shots {
            self.wipe_shots = false;
            shots.retain(|s| !s.hostile);
        }
        self.shots = shots;
    }

    fn resolve_deaths(&mut self) {
        let mut i = 0;
        let mut boss_dead = false;
        while i < self.enemies.len() {
            if self.enemies[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let e = self.enemies.swap_remove(i);
            self.kills += 1;
            self.effects.push(Effect {
                kind: EffectKind::Burst,
                pos: e.pos,
                dir: Vec2::ZERO,
                age: 0.0,
                duration: 0.5,
                radius: e.radius() + 0.4,
            });
            if self.mods.lifesteal > 0.0 {
                let p = &mut self.player;
                p.hp = (p.hp + self.mods.lifesteal).min(p.max_hp);
            }
            if e.kind == EnemyKind::Postmaster {
                boss_dead = true;
            }
        }
        if boss_dead {
            for e in self.enemies.drain(..) {
                self.effects.push(Effect {
                    kind: EffectKind::Burst,
                    pos: e.pos,
                    dir: Vec2::ZERO,
                    age: 0.0,
                    duration: 0.5,
                    radius: e.radius() + 0.4,
                });
            }
            self.pending.clear();
            self.arrivals.clear();
            self.shots.retain(|s| !s.hostile);
        }
    }

    fn age_effects(&mut self) {
        for e in self.effects.iter_mut() {
            e.age += DT;
        }
        self.effects.retain(|e| e.age < e.duration);
    }

    fn advance_waves(&mut self) {
        self.land_arrivals();
        if !self.enemies.is_empty() || !self.pending.is_empty() || !self.arrivals.is_empty() {
            self.lull = LULL;
            return;
        }
        self.lull -= DT;
        if self.lull > 0.0 {
            return;
        }
        if self.wave < waves_in(self.chamber) {
            self.spawn_wave();
            self.lull = LULL;
        } else if self.chamber >= CHAMBERS {
            self.phase = Phase::Won;
        } else {
            self.roll_choices();
            self.phase = Phase::Choice;
        }
    }
}

/// Hooks for tests outside this module that stage single attacks.
#[cfg(test)]
impl Game {
    /// A started run whose waves never spawn.
    pub(crate) fn quiet_arena(seed: u64) -> Game {
        let mut g = Game::new(seed);
        g.start();
        g.lull = f32::INFINITY;
        g.wave = waves_in(1);
        g
    }

    /// Place an enemy one tick from releasing `attack` toward `dir`, with its aim
    /// already locked. `attack_index` picks the boss ring's rotation; an
    /// `attack` of `None` stages the end of the Postmaster's half-hull stagger.
    pub(crate) fn stage_release(
        &mut self,
        kind: EnemyKind,
        attack: Option<u8>,
        attack_index: u32,
        pos: Vec2,
        dir: Vec2,
        phase2: bool,
    ) {
        let mut e = self.make_enemy(kind, pos);
        e.emerging = false;
        e.dir = dir;
        e.phase2 = phase2;
        e.attack_index = attack_index;
        match attack {
            Some(a) => {
                e.attack = a;
                e.begin(S_WIND, 0.8);
            }
            None => e.begin(S_TRANS, 1.3),
        }
        e.timer = DT;
        self.effects.clear();
        self.enemies.push(e);
    }

    /// True while enemy `i` is still winding up or carrying out an attack.
    pub(crate) fn attacking(&self, i: usize) -> bool {
        matches!(self.enemies[i].state, S_WIND | S_ACT | S_TRANS)
    }

    /// Keep enemy `i` in the arena but never let it start another attack.
    pub(crate) fn disarm(&mut self, i: usize) {
        let e = &mut self.enemies[i];
        if e.state != S_MOVE {
            e.begin(S_MOVE, 0.0);
        }
        e.cd = f32::INFINITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena_game() -> Game {
        let mut g = Game::new(7);
        g.start();
        // Skip the opening lull so tests control the roster.
        g.lull = 99.0;
        g.wave = waves_in(1);
        g
    }

    fn place(g: &mut Game, kind: EnemyKind, pos: Vec2) -> usize {
        let mut e = g.make_enemy(kind, pos);
        e.emerging = false;
        e.begin(S_MOVE, 0.0);
        e.cd = 5.0;
        g.enemies.push(e);
        g.enemies.len() - 1
    }

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn enemy_release_poses_match_the_attack_tick() {
        let mut g = arena_game();
        for (kind, attack, expected) in [
            (EnemyKind::Bailiff, 0, 11),
            (EnemyKind::Censor, 0, 11),
            (EnemyKind::Postmaster, ATT_RING, 11),
            (EnemyKind::Postmaster, ATT_CHARGE, 16),
            (EnemyKind::Postmaster, ATT_FAN, 21),
            (EnemyKind::Postmaster, ATT_SUMMON, 26),
        ] {
            let mut enemy = g.make_enemy(kind, Vec2::new(2.0, 0.0));
            enemy.dir = Vec2::new(-1.0, 0.0);
            enemy.attack = attack;
            enemy.begin(S_WIND, DT);
            g.step_enemy(&mut enemy);
            assert_eq!(enemy.animation_frame(), expected);
        }
    }

    #[test]
    fn deterministic_for_same_seed_and_inputs() {
        let run = || {
            let mut g = Game::new(42);
            g.start();
            for t in 0..900 {
                let f = t as f32 * 0.05;
                g.step(Input {
                    movement: Vec2::new(f.cos(), f.sin()),
                    aim: Vec2::new(2.0, 1.0),
                    strike: t % 7 < 3,
                    cast: t % 50 == 0,
                    dash: t % 120 == 0,
                });
            }
            (
                g.player.pos,
                g.player.hp,
                g.enemies.len(),
                g.kills,
                g.shots.len(),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn dash_iframes_dodge_lunge_but_standing_still_is_hit() {
        for dashing in [false, true] {
            let mut g = arena_game();
            g.player.pos = Vec2::ZERO;
            let i = place(&mut g, EnemyKind::Bailiff, Vec2::new(0.55, 0.0));
            g.enemies[i].dir = Vec2::new(-1.0, 0.0);
            g.enemies[i].begin(S_ACT, 0.28);
            g.step(Input {
                dash: dashing,
                movement: Vec2::new(0.0, 1.0),
                ..idle()
            });
            if dashing {
                assert_eq!(g.player.hp, PLAYER_HP);
            } else {
                assert_eq!(g.player.hp, PLAYER_HP - 14.0);
            }
        }
    }

    #[test]
    fn bailiff_telegraphs_before_lunge_and_deals_no_damage_while_winding() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        let i = place(&mut g, EnemyKind::Bailiff, Vec2::new(2.0, 0.0));
        g.enemies[i].cd = 0.0;
        let mut wound = 0;
        let mut last = 0.0f32;
        for _ in 0..40 {
            g.step(idle());
            let e = &g.enemies[0];
            // The cue must be visible on every tick of the windup, from its first.
            assert_eq!(
                e.windup > 0.0,
                e.state == S_WIND,
                "state {} windup {}",
                e.state,
                e.windup
            );
            if e.state == S_WIND {
                wound += 1;
                assert!(e.windup >= last, "windup must rise monotonically");
                last = e.windup;
            }
            assert_eq!(g.player.hp, PLAYER_HP);
        }
        assert!(
            wound >= 38,
            "telegraph should last ~0.7s, was {wound} ticks"
        );
        // Telegraph completes, then the lunge lands and the cue clears.
        let mut hit = false;
        for _ in 0..30 {
            g.step(idle());
            if g.player.hp < PLAYER_HP {
                hit = true;
                assert_eq!(g.enemies[0].windup, 0.0);
                break;
            }
        }
        assert!(hit, "lunge should connect after the telegraph");
    }

    #[test]
    fn three_hit_chain_finisher_hits_harder_and_resets_after_window() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        let i = place(&mut g, EnemyKind::Postmaster, Vec2::new(1.2, 0.0));
        g.enemies[i].hp = 1000.0;
        g.enemies[i].max_hp = 1000.0;
        g.enemies[i].state = S_MOVE;
        let aim = Input {
            aim: Vec2::new(3.0, 0.0),
            strike: true,
            ..idle()
        };
        let mut dmgs = [0.0f32; 3];
        for d in dmgs.iter_mut() {
            let before = g.enemies[0].hp;
            // Wait until the strike lands.
            for _ in 0..60 {
                g.step(aim);
                if g.enemies[0].hp < before {
                    break;
                }
            }
            *d = before - g.enemies[0].hp;
            g.enemies[0].pos = Vec2::new(g.player.pos.x + 1.2, 0.0);
            g.player.pos.y = 0.0;
        }
        assert!(dmgs[2] > dmgs[0] * 1.5, "{dmgs:?}");
        // After the window lapses the chain restarts.
        for _ in 0..60 {
            g.step(idle());
        }
        assert_eq!(g.player.combo, 0);
    }

    #[test]
    fn strike_cuts_hostile_shots_in_front_but_not_behind() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        g.fire(
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            0.0,
            9.0,
            0.17,
            Blow::CensorBolt,
        );
        g.fire(
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            0.0,
            9.0,
            0.17,
            Blow::CensorBolt,
        );
        g.step(Input {
            aim: Vec2::new(3.0, 0.0),
            strike: true,
            ..idle()
        });
        assert_eq!(g.shots.len(), 1);
        assert!(g.shots[0].pos.x < 0.0);
    }

    #[test]
    fn cast_respects_cooldown_and_pierce_hits_each_target_once() {
        let mut g = arena_game();
        g.player.pos = Vec2::new(-3.0, 0.0);
        g.apply_boon(Boon::RegisteredMail);
        for x in [-1.0, 0.5] {
            let i = place(&mut g, EnemyKind::Postmaster, Vec2::new(x, 0.0));
            g.enemies[i].state = S_MOVE;
            g.enemies[i].hp = 500.0;
            g.enemies[i].max_hp = 500.0;
        }
        let cast = Input {
            aim: Vec2::new(0.0, 0.0),
            cast: true,
            ..idle()
        };
        g.step(cast);
        g.step(cast);
        assert_eq!(
            g.shots.iter().filter(|s| !s.hostile).count(),
            1,
            "cooldown gates second cast"
        );
        for _ in 0..60 {
            g.step(idle());
        }
        let expected = CAST_DAMAGE * 1.25;
        for e in &g.enemies {
            assert!(
                (500.0 - e.hp - expected).abs() < 1e-3,
                "hit once for {}",
                500.0 - e.hp
            );
        }
    }

    #[test]
    fn boss_enters_invulnerable_transition_at_half_health() {
        let mut g = arena_game();
        let i = place(&mut g, EnemyKind::Postmaster, Vec2::new(2.0, 0.0));
        g.enemies[i].hp = g.enemies[i].max_hp * 0.5 + 1.0;
        assert!(g.damage_enemy(i, 5.0, Vec2::new(1.0, 0.0), 0.0));
        assert!(g.enemies[i].phase2);
        let hp = g.enemies[i].hp;
        assert!(!g.damage_enemy(i, 50.0, Vec2::new(1.0, 0.0), 0.0));
        assert_eq!(g.enemies[i].hp, hp);
    }

    #[test]
    fn clearing_chambers_offers_three_distinct_boons_and_ends_in_victory() {
        let mut g = Game::new(3);
        g.start();
        let mut ticks = 0;
        loop {
            ticks += 1;
            assert!(ticks < 20_000, "run did not progress");
            for e in g.enemies.iter_mut() {
                e.hp = 0.0;
            }
            g.player.hp = g.player.max_hp;
            g.step(idle());
            match g.phase {
                Phase::Choice => {
                    let c = g.choices;
                    assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2], "{c:?}");
                    let before = g.chamber;
                    g.choose(1);
                    assert_eq!(g.chamber, before + 1);
                    assert_eq!(g.phase, Phase::Combat);
                }
                Phase::Won => break,
                _ => {}
            }
        }
        assert_eq!(g.chamber, CHAMBERS);
        assert_eq!(g.seal_count as usize, CHAMBERS - 1);
        // Restart resets the run.
        g.start();
        assert_eq!(g.phase, Phase::Combat);
        assert_eq!(g.chamber, 1);
        assert_eq!(g.kills, 0);
    }

    #[test]
    fn lethal_damage_loses_and_freezes_world() {
        let mut g = arena_game();
        g.player.hp = 5.0;
        g.player.pos = Vec2::ZERO;
        g.fire(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            0.0,
            9.0,
            0.17,
            Blow::CensorBolt,
        );
        g.step(idle());
        assert_eq!(g.phase, Phase::Lost);
        let t = g.time;
        g.step(idle());
        assert_eq!(g.time, t);
    }

    /// Step idle from a fresh run until the first wave is marked on the floor.
    fn until_marked(g: &mut Game) -> usize {
        for tick in 0..600 {
            g.step(idle());
            if !g.arrivals.is_empty() {
                return tick;
            }
        }
        panic!("no wave was marked");
    }

    #[test]
    fn wave_entry_points_are_marked_for_the_warning_interval_before_clerks_emerge() {
        let mut g = Game::new(19);
        g.start();
        until_marked(&mut g);
        let kinds: Vec<EnemyKind> = g.arrivals.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, wave_kinds(1, 0));
        let marks: Vec<Vec2> = g.arrivals.iter().map(|a| a.pos).collect();
        for m in &marks {
            assert!(
                (*m - g.player.pos).len() >= 1.0,
                "marker on top of the courier"
            );
        }

        // Nothing in the wave exists, and so nothing can attack, while it is marked.
        let mut marked_ticks = 1;
        let mut last = g.arrivals[0].progress();
        while !g.arrivals.is_empty() {
            assert!(g.enemies.is_empty());
            assert!(g.arrivals[0].progress() >= last, "progress must rise");
            last = g.arrivals[0].progress();
            g.step(idle());
            if !g.arrivals.is_empty() {
                marked_ticks += 1;
            }
        }
        let expected = (SPAWN_WARNING / DT).round() as i32;
        assert!(
            (marked_ticks - expected).abs() <= 1,
            "marked for {marked_ticks} ticks, expected ~{expected}"
        );

        // Each clerk surfaces exactly where it was marked, still emerging and harmless.
        assert_eq!(g.enemies.len(), marks.len());
        for (e, m) in g.enemies.iter().zip(&marks) {
            assert!((e.pos - *m).len() < 1e-6, "{:?} vs {:?}", e.pos, m);
            assert!(e.emerging);
        }
        while g.enemies.iter().any(|e| e.emerging) {
            assert!(g.enemies.iter().all(|e| e.windup == 0.0));
            assert_eq!(g.player.hp, PLAYER_HP);
            g.step(idle());
        }
    }

    #[test]
    fn chamber_cleanup_and_restart_clear_marked_arrivals() {
        let mut g = Game::new(23);
        g.start();
        until_marked(&mut g);
        // Leaving the chamber while a wave is still marked drops it.
        g.phase = Phase::Choice;
        g.choose(0);
        assert!(g.arrivals.is_empty());
        assert!(g.enemies.is_empty());
        until_marked(&mut g);
        let kinds: Vec<EnemyKind> = g.arrivals.iter().map(|a| a.kind).collect();
        assert_eq!(
            kinds,
            wave_kinds(2, 0),
            "next chamber starts from its first wave"
        );

        g.start();
        assert!(g.arrivals.is_empty());
    }

    fn press(strike: bool, cast: bool, dash: bool) -> Input {
        Input {
            aim: Vec2::new(2.0, 1.0),
            strike,
            cast,
            dash,
            ..idle()
        }
    }

    /// Scripted play used to compare runs: wandering, chaining strikes,
    /// casting and dashing.
    fn scripted(t: usize) -> Input {
        let f = t as f32 * 0.05;
        Input {
            movement: Vec2::new(f.cos(), f.sin()),
            aim: Vec2::new(2.0, 1.0),
            strike: t % 7 < 3,
            cast: t % 50 == 0,
            dash: t % 120 == 0,
        }
    }

    fn fingerprint(g: &Game) -> String {
        let foes: Vec<_> = g.enemies.iter().map(|e| (e.kind, e.pos, e.hp)).collect();
        let shots: Vec<_> = g.shots.iter().map(|s| s.pos).collect();
        format!(
            "{:?} {:?} {} {} {} {} {:?} {:?} {:?} {:?}",
            g.phase,
            g.player.pos,
            g.player.hp,
            g.time,
            g.kills,
            g.chamber,
            foes,
            shots,
            g.choices,
            g.rng
        )
    }

    /// Finish a drill with wandering play, then run the script.
    fn after_drill(g: &mut Game) -> String {
        let mut t = 0;
        while g.lesson.is_some() {
            assert!(t < 2_000, "drill did not finish");
            let mut input = scripted(t + 3);
            input.dash = t % 70 == 0;
            input.cast = t % 40 == 0;
            g.step(input);
            assert!(g.enemies.is_empty() && g.pending.is_empty() && g.arrivals.is_empty());
            t += 1;
        }
        for t in 0..1_500 {
            g.step(scripted(t));
        }
        fingerprint(g)
    }

    #[test]
    fn drill_advances_only_on_the_requested_action() {
        let mut g = Game::new(9);
        g.start_drill();
        assert_eq!(g.lesson, Some(Lesson::Strike));
        g.step(press(false, true, false));
        g.step(press(false, false, true));
        assert_eq!(
            g.lesson,
            Some(Lesson::Strike),
            "cast and dash do not teach strike"
        );
        // The engine ignores strikes mid-dash, so let the dash finish.
        for _ in 0..12 {
            g.step(idle());
        }
        g.step(press(true, false, false));
        assert_eq!(g.lesson, Some(Lesson::Cast));
        // Let the earlier cast recharge, then a strike still does not count.
        for _ in 0..60 {
            g.step(idle());
        }
        g.step(press(true, false, false));
        assert_eq!(g.lesson, Some(Lesson::Cast));
        g.step(press(false, true, false));
        assert_eq!(g.lesson, Some(Lesson::Dash));
        // The dash from the first tick has recharged; this fresh press counts.
        g.step(press(false, false, true));
        assert_eq!(g.lesson, Some(Lesson::Complete));
        assert_eq!(Lesson::Complete.done(), Lesson::STEPS.len());

        // Waves hold for the closing beat, then the run proper begins.
        let mut beat = 0;
        while g.lesson.is_some() {
            assert!(g.enemies.is_empty());
            g.step(idle());
            beat += 1;
        }
        assert!(
            (55..=65).contains(&beat),
            "closing beat lasted {beat} ticks"
        );
        assert_eq!(g.time, 0.0);
        assert_eq!(g.player.pos, Vec2::ZERO);
        let mut spawned = false;
        for _ in 0..120 {
            g.step(idle());
            spawned |= !g.enemies.is_empty();
        }
        assert!(spawned, "first wave arrives once the drill is over");
    }

    #[test]
    fn drill_skip_and_replay_never_change_the_seeded_run() {
        let plain = {
            let mut g = Game::new(77);
            g.start();
            for t in 0..1_500 {
                g.step(scripted(t));
            }
            fingerprint(&g)
        };

        let mut drilled = Game::new(77);
        drilled.start_drill();
        assert_eq!(after_drill(&mut drilled), plain, "completing the drill");

        let mut skipped = Game::new(77);
        skipped.start_drill();
        for t in 0..40 {
            skipped.step(scripted(t));
        }
        skipped.skip_drill();
        assert_eq!(skipped.lesson, None);
        for t in 0..1_500 {
            skipped.step(scripted(t));
        }
        assert_eq!(fingerprint(&skipped), plain, "skipping part-way");

        // A replayed drill on the next run matches that run started directly.
        let second = {
            let mut g = Game::new(77);
            g.start();
            g.start();
            for t in 0..1_500 {
                g.step(scripted(t));
            }
            fingerprint(&g)
        };
        drilled.start_drill();
        assert_eq!(after_drill(&mut drilled), second, "replaying the drill");
        assert_ne!(second, plain);
    }

    #[test]
    fn lethal_lunge_is_reported_as_the_killing_blow() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        g.player.hp = 10.0;
        let i = place(&mut g, EnemyKind::Bailiff, Vec2::new(0.55, 0.0));
        g.enemies[i].dir = Vec2::new(-1.0, 0.0);
        g.enemies[i].begin(S_ACT, 0.28);
        g.step(idle());
        assert_eq!(g.phase, Phase::Lost);
        assert_eq!(g.killing_blow, Some(Blow::BailiffLunge));
    }

    #[test]
    fn lethal_censor_bolt_is_reported_and_a_fresh_run_clears_it() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        g.player.hp = 9.0;
        // A Censor releasing its volley two units away, aimed at the courier.
        let i = place(&mut g, EnemyKind::Censor, Vec2::new(2.0, 0.0));
        g.enemies[i].begin(S_WIND, 0.85);
        g.enemies[i].timer = DT;
        g.enemies[i].dir = Vec2::new(-1.0, 0.0);
        for _ in 0..120 {
            if g.phase == Phase::Lost {
                break;
            }
            g.step(idle());
            // A non-lethal phase never records a cause.
            if g.phase == Phase::Combat {
                assert_eq!(g.killing_blow, None);
            }
        }
        assert_eq!(g.phase, Phase::Lost);
        assert_eq!(g.killing_blow, Some(Blow::CensorBolt));
        g.start();
        assert_eq!(g.killing_blow, None);
    }

    #[test]
    fn surviving_a_hit_records_no_killing_blow() {
        let mut g = arena_game();
        g.player.pos = Vec2::ZERO;
        g.fire(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            0.0,
            9.0,
            0.17,
            Blow::PostmasterFan,
        );
        g.step(idle());
        assert_eq!(g.player.hp, PLAYER_HP - 9.0);
        assert_eq!(g.killing_blow, None);
    }

    #[test]
    fn every_killing_blow_has_a_name_and_a_hint() {
        for blow in [
            Blow::BailiffLunge,
            Blow::CensorBolt,
            Blow::PostmasterRing,
            Blow::PostmasterCharge,
            Blow::PostmasterFan,
            Blow::PostmasterShockwave,
        ] {
            assert!(!blow.attacker().is_empty());
            assert!(blow.counterplay().ends_with('.'));
        }
    }

    #[test]
    fn player_and_enemies_stay_inside_arena() {
        let mut g = arena_game();
        place(&mut g, EnemyKind::Bailiff, Vec2::new(4.0, 4.0));
        for _ in 0..300 {
            g.step(Input {
                movement: Vec2::new(1.0, 1.0),
                dash: true,
                ..idle()
            });
            assert!(g.player.pos.x.abs() <= ARENA && g.player.pos.y.abs() <= ARENA);
            for e in &g.enemies {
                assert!(e.pos.x.abs() <= ARENA && e.pos.y.abs() <= ARENA);
            }
        }
    }
}
