//! DEAD LETTER / THE UNDERTOW POST: native GPUI shell.
//!
//! This file owns the window, the full-screen HUD, input capture and the
//! fixed-step loop. Simulation lives in `engine`, drawing of the arena in `scene`.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod engine;
mod scene;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    div, prelude::*, px, relative, rgb, rgba, size, App, Application, Bounds, Context, Div,
    FocusHandle, FontWeight, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Render, SharedString, Styled, Subscription, Task, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};

use engine::{Boon, EnemyKind, Game, Input, Lesson, Phase, Vec2, CHAMBERS};

// ---------------------------------------------------------------- palette ---
const INK: u32 = 0x060f12; // abyss slate matching outer arena water
const TEAL: u32 = 0x10353b;
const TEAL_HI: u32 = 0x216268;
const PEARL: u32 = 0xede8d8; // warm BotW parchment ivory
const PEARL_DIM: u32 = 0x9ab0ac;
const BRASS: u32 = 0xe5c148; // ancient gold
const WAX: u32 = 0xe03c28; // crimson seal
const GLOW: u32 = 0x5ce0d8; // luminous rune cyan

const HUD_BG: u32 = 0x051013bf;
const HUD_BG_SOFT: u32 = 0x0510138c;
const HUD_BORDER: u32 = 0xede8d829;
const HUD_RULE: u32 = 0xede8d822;

const MONO: &str = "Consolas";
const SERIF: &str = "Georgia";

// ----------------------------------------------------------------- timing ---
const STEP: f32 = 1.0 / 60.0;
const MAX_FRAME: f32 = 0.1;
const MAX_STEPS_PER_TICK: u32 = 4;

#[derive(Clone, Copy)]
struct Metrics {
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
}

fn metrics(vw: f32, vh: f32) -> Metrics {
    Metrics {
        bx: 0.0,
        by: 0.0,
        bw: vw.max(240.0),
        bh: vh.max(160.0),
    }
}

// ------------------------------------------------------------------ state ---
#[derive(Default)]
struct Held {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    strike_key: bool,
    cast_key: bool,
    lmb: bool,
    rmb: bool,
}

#[derive(Default)]
struct Latch {
    strike: bool,
    cast: bool,
    dash: bool,
}

struct Shell {
    game: Game,
    seed: u64,
    focus: FocusHandle,
    held: Held,
    latch: Latch,
    /// Last cursor position in window coordinates.
    mouse: Option<(f32, f32)>,
    keyboard_aim: bool,
    paused: bool,
    acc: f32,
    viewport: (f32, f32),
    boons: Vec<Boon>,
    /// The opening drill has run (or been skipped) this session.
    drilled: bool,
    _loop: Task<()>,
    _subs: Vec<Subscription>,
}

fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EA)
}

impl Shell {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);

        let mut subs = Vec::new();
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.lose_focus();
                cx.notify();
            } else {
                this.acc = 0.0;
            }
        }));
        subs.push(cx.on_focus_out(&focus, window, |this, _ev, _window, cx| {
            this.lose_focus();
            cx.notify();
        }));

        // Fixed-step driver: real elapsed time feeds an accumulator, capped so a
        // stall can never trigger a catch-up spiral.
        let task = cx.spawn(async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(4))
                    .await;
                let now = Instant::now();
                let dt = now.duration_since(last).as_secs_f32();
                last = now;
                if this.update(cx, |shell, cx| shell.tick(dt, cx)).is_err() {
                    break;
                }
            }
        });

        let seed = clock_seed();
        Shell {
            game: Game::new(seed),
            seed,
            focus,
            held: Held::default(),
            latch: Latch::default(),
            mouse: None,
            keyboard_aim: false,
            paused: false,
            acc: 0.0,
            viewport: (1280.0, 800.0),
            boons: Vec::new(),
            drilled: false,
            _loop: task,
            _subs: subs,
        }
    }

    /// Window deactivated / focus lost: release everything, pause live combat.
    fn lose_focus(&mut self) {
        self.held = Held::default();
        self.latch = Latch::default();
        if self.game.phase == Phase::Combat {
            self.paused = true;
        }
    }

    /// Leave the title screen. The session's first dispatch opens with the drill.
    fn dispatch(&mut self, drill: bool) {
        if drill || !self.drilled {
            self.game.start_drill();
        } else {
            self.game.start();
        }
        self.drilled = true;
        self.acc = 0.0;
        self.latch = Latch::default();
    }

    fn restart(&mut self, drill: bool) {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15) ^ clock_seed();
        self.game = Game::new(self.seed);
        self.boons.clear();
        self.paused = false;
        self.dispatch(drill);
    }

    fn skip_drill(&mut self) {
        if self.game.lesson.is_some() {
            self.game.skip_drill();
            self.latch = Latch::default();
            self.acc = 0.0;
        }
    }

    fn pick(&mut self, index: usize) {
        if self.game.phase == Phase::Choice && index < 3 {
            self.boons.push(self.game.choices[index]);
            self.game.choose(index);
            self.latch = Latch::default();
            self.acc = 0.0;
        }
    }

    fn live(&self) -> bool {
        self.game.phase == Phase::Combat && !self.paused
    }

    fn build_input(&self) -> Input {
        let h = &self.held;
        let sx = h.right as i32 as f32 - h.left as i32 as f32;
        let sy = h.up as i32 as f32 - h.down as i32 as f32;
        // Screen right = (+x,+y), screen up = (-x,+y).
        let (mut wx, mut wy) = (sx - sy, sx + sy);
        let len = (wx * wx + wy * wy).sqrt();
        if len > 0.0 {
            wx /= len;
            wy /= len;
        }
        let movement = Vec2::new(wx, wy);

        let p = self.game.player.pos;
        let aim = match self.mouse {
            Some((mx, my)) if !self.keyboard_aim => {
                let m = metrics(self.viewport.0, self.viewport.1);
                scene::screen_to_world(mx - m.bx, my - m.by, m.bw, m.bh)
            }
            _ => {
                let f = if len > 0.0 {
                    movement
                } else {
                    self.game.player.facing
                };
                Vec2::new(p.x + f.x * 3.0, p.y + f.y * 3.0)
            }
        };

        Input {
            movement,
            aim,
            strike: h.strike_key || h.lmb || self.latch.strike,
            cast: h.cast_key || h.rmb || self.latch.cast,
            dash: self.latch.dash,
        }
    }

    fn tick(&mut self, dt: f32, cx: &mut Context<Self>) {
        let phase = self.game.phase;
        if phase == Phase::Ready || self.paused {
            self.acc = 0.0;
            return;
        }
        self.acc += dt.min(MAX_FRAME);
        let mut steps = 0;
        while self.acc >= STEP {
            if steps >= MAX_STEPS_PER_TICK {
                self.acc = 0.0;
                break;
            }
            let input = self.build_input();
            self.game.step(input);
            self.latch = Latch::default();
            self.acc -= STEP;
            steps += 1;
        }
        if steps > 0 {
            cx.notify();
        }
    }

    // ------------------------------------------------------------- events ---
    fn key_down(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = e.keystroke.key.as_str();
        let fresh = !e.is_held;
        match key {
            "w" | "up" => self.held.up = true,
            "s" | "down" => self.held.down = true,
            "a" | "left" => self.held.left = true,
            "d" | "right" => self.held.right = true,
            "j" => {
                self.held.strike_key = true;
                self.keyboard_aim = true;
                if fresh && self.live() {
                    self.latch.strike = true;
                }
            }
            "k" => {
                self.held.cast_key = true;
                self.keyboard_aim = true;
                if fresh && self.live() {
                    self.latch.cast = true;
                }
            }
            "space" if fresh => match self.game.phase {
                Phase::Ready => self.dispatch(false),
                Phase::Combat if !self.paused => self.latch.dash = true,
                _ => {}
            },
            "enter" if fresh => match self.game.phase {
                Phase::Ready => self.dispatch(false),
                Phase::Won | Phase::Lost => self.restart(false),
                Phase::Combat if self.paused => self.resume(),
                _ => {}
            },
            "tab" if fresh && self.live() => self.skip_drill(),
            "t" if fresh => match self.game.phase {
                Phase::Ready => self.dispatch(true),
                Phase::Won | Phase::Lost => self.restart(true),
                _ => {}
            },
            "escape" if fresh => {
                if self.paused {
                    self.resume();
                } else if self.game.phase == Phase::Combat {
                    self.held = Held::default();
                    self.latch = Latch::default();
                    self.paused = true;
                }
            }
            "r" if fresh => {
                if matches!(self.game.phase, Phase::Won | Phase::Lost) {
                    self.restart(false);
                }
            }
            "1" if fresh => self.pick(0),
            "2" if fresh => self.pick(1),
            "3" if fresh => self.pick(2),
            _ => {}
        }
        cx.notify();
    }

    fn resume(&mut self) {
        self.paused = false;
        self.acc = 0.0;
    }

    fn key_up(&mut self, e: &KeyUpEvent) {
        match e.keystroke.key.as_str() {
            "w" | "up" => self.held.up = false,
            "s" | "down" => self.held.down = false,
            "a" | "left" => self.held.left = false,
            "d" | "right" => self.held.right = false,
            "j" => self.held.strike_key = false,
            "k" => self.held.cast_key = false,
            _ => {}
        }
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.mouse = Some((f32::from(e.position.x), f32::from(e.position.y)));
        self.keyboard_aim = false;
        if self.live() {
            match e.button {
                MouseButton::Left => {
                    self.held.lmb = true;
                    self.latch.strike = true;
                }
                MouseButton::Right => {
                    self.held.rmb = true;
                    self.latch.cast = true;
                }
                _ => {}
            }
        } else if self.game.phase == Phase::Ready && e.button == MouseButton::Left {
            self.dispatch(false);
        } else if self.paused && e.button == MouseButton::Left {
            self.resume();
        }
        cx.notify();
    }

    fn mouse_up(&mut self, e: &MouseUpEvent) {
        match e.button {
            MouseButton::Left => self.held.lmb = false,
            MouseButton::Right => self.held.rmb = false,
            _ => {}
        }
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent) {
        let p = (f32::from(e.position.x), f32::from(e.position.y));
        if self.mouse.map_or(true, |m| m != p) {
            self.mouse = Some(p);
            self.keyboard_aim = false;
        }
    }
}

// --------------------------------------------------------------- widgets ----
fn mono(s: impl Into<SharedString>, size: f32, color: u32) -> Div {
    div()
        .font_family(MONO)
        .text_size(px(size))
        .text_color(rgb(color))
        .whitespace_nowrap()
        .child(s.into())
}

fn serif(s: impl Into<SharedString>, size: f32, color: u32) -> Div {
    div()
        .font_family(SERIF)
        .text_size(px(size))
        .text_color(rgb(color))
        .child(s.into())
}

fn rule(color: u32) -> Div {
    div().w_full().h(px(1.0)).flex_none().bg(rgb(color))
}

fn rule_alpha(rgba_hex: u32) -> Div {
    div().w_full().h(px(1.0)).flex_none().bg(rgba(rgba_hex))
}

fn hud_card() -> Div {
    div()
        .bg(rgba(HUD_BG))
        .border_1()
        .border_color(rgba(HUD_BORDER))
        .rounded(px(4.0))
}

fn life_pips(frac: f32, total: usize, color: u32) -> Div {
    let lit = (frac.clamp(0.0, 1.0) * total as f32).ceil() as usize;
    let mut row = div().flex().flex_row().items_center().gap(px(3.0));
    for i in 0..total {
        let active = i < lit;
        row = row.child(
            div()
                .w(px(13.0))
                .h(px(10.0))
                .rounded(px(2.0))
                .bg(if active { rgb(color) } else { rgba(0x13282cc8) })
                .border_1()
                .border_color(if active { rgb(color) } else { rgba(0xede8d824) }),
        );
    }
    row
}

fn gauge(frac: f32, color: u32) -> Div {
    div()
        .w_full()
        .h(px(4.0))
        .rounded(px(2.0))
        .overflow_hidden()
        .bg(rgba(0x13282cd8))
        .child(
            div()
                .h_full()
                .w(relative(frac.clamp(0.0, 1.0)))
                .bg(rgb(color)),
        )
}

fn keycap(label: impl Into<SharedString>) -> Div {
    div()
        .px(px(5.0))
        .py(px(1.0))
        .rounded(px(3.0))
        .bg(rgba(0xede8d816))
        .border_1()
        .border_color(rgba(0xede8d838))
        .child(mono(label, 9.0, PEARL).font_weight(FontWeight::BOLD))
}

fn control_hint(key: &'static str, action: &'static str) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(keycap(key))
        .child(mono(action, 10.0, PEARL_DIM))
}

fn roman(n: usize) -> &'static str {
    ["0", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"]
        .get(n)
        .copied()
        .unwrap_or("?")
}

fn clock(t: f32) -> String {
    let s = t.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

// ----------------------------------------------------------------- render ---
impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vs = window.viewport_size();
        let (vw, vh) = (f32::from(vs.width), f32::from(vs.height));
        self.viewport = (vw, vh);
        let m = metrics(vw, vh);

        let aim = match self.mouse {
            Some((mx, my))
                if !self.keyboard_aim
                    && self.game.phase == Phase::Combat
                    && mx >= m.bx
                    && mx <= m.bx + m.bw
                    && my >= m.by
                    && my <= m.by + m.bh =>
            {
                Some(scene::screen_to_world(mx - m.bx, my - m.by, m.bw, m.bh))
            }
            _ => None,
        };

        div()
            .id("dead-letter")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _w, cx| this.key_down(e, cx)))
            .on_key_up(cx.listener(|this, e: &KeyUpEvent, _w, _cx| this.key_up(e)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, w, cx| this.mouse_down(e, w, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, w, cx| this.mouse_down(e, w, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _w, _cx| this.mouse_up(e)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, e: &MouseUpEvent, _w, _cx| this.mouse_up(e)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _w, _cx| this.mouse_up(e)),
            )
            .on_mouse_up_out(
                MouseButton::Right,
                cx.listener(|this, e: &MouseUpEvent, _w, _cx| this.mouse_up(e)),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, _cx| this.mouse_move(e)))
            .relative()
            .size_full()
            .bg(rgb(INK))
            .text_color(rgb(PEARL))
            .overflow_hidden()
            .cursor_crosshair()
            .child(scene::render(&self.game, aim, m.bw, m.bh))
            .child(self.hud_top_left())
            .children(self.hud_boss())
            .child(self.hud_top_right())
            .children(self.hud_drill(cx))
            .children(self.overlay(m, cx))
            .when(self.paused || self.game.phase == Phase::Ready, |root| {
                root.child(self.hud_bottom_controls())
            })
    }
}

impl Shell {
    fn doc_no(&self) -> String {
        format!(
            "UT-{:04}-{}",
            self.seed % 10_000,
            roman(self.game.chamber.max(1).min(9))
        )
    }

    /// Top-left HUD cluster: BotW-style Hull pips, ability cooldown gauges, and endorsements.
    fn hud_top_left(&self) -> Div {
        let p = &self.game.player;
        let hull = if p.max_hp > 0.0 { p.hp / p.max_hp } else { 0.0 };
        let low_hull = hull < 0.34;
        let hull_color = if low_hull { WAX } else { GLOW };

        let frac = |cd: f32, max: f32| {
            if cd <= 0.0 || max <= 0.0 {
                1.0
            } else {
                1.0 - cd / max
            }
        };
        let dash_ready = p.dash_cooldown <= 0.0;
        let cast_ready = p.cast_cooldown <= 0.0;
        let dash_frac = frac(p.dash_cooldown, p.dash_cooldown_max);
        let cast_frac = frac(p.cast_cooldown, p.cast_cooldown_max);
        let cd_text = |cd: f32| {
            if cd <= 0.0 {
                "READY".to_string()
            } else {
                format!("{:.1}s", cd)
            }
        };

        let hull_card = hud_card()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .justify_between()
                    .gap(px(12.0))
                    .child(mono("HULL INTEGRITY", 11.0, PEARL_DIM))
                    .child(
                        mono(
                            format!("{:.0} / {:.0}", p.hp.max(0.0).ceil(), p.max_hp),
                            11.0,
                            if low_hull { WAX } else { PEARL },
                        )
                        .font_weight(FontWeight::BOLD),
                    ),
            )
            .child(life_pips(hull, 12, hull_color));

        let ability_pill =
            |name: &'static str, status: String, f: f32, ready: bool, c: u32, dim_c: u32| {
                hud_card()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .w(px(104.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .justify_between()
                            .child(mono(name, 11.0, PEARL_DIM))
                            .child(
                                mono(status, 12.0, if ready { c } else { PEARL_DIM })
                                    .font_weight(FontWeight::BOLD),
                            ),
                    )
                    .child(gauge(f, if ready { c } else { dim_c }))
            };

        let abilities_row = div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(ability_pill(
                "DASH",
                cd_text(p.dash_cooldown),
                dash_frac,
                dash_ready,
                GLOW,
                TEAL_HI,
            ))
            .child(ability_pill(
                "CAST",
                cd_text(p.cast_cooldown),
                cast_frac,
                cast_ready,
                BRASS,
                0x7a6a2a,
            ));

        let mut endorsements = div().flex().flex_col().gap(px(4.0));
        for (i, b) in self.boons.iter().enumerate() {
            endorsements = endorsements.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(3.0))
                    .bg(rgba(HUD_BG_SOFT))
                    .border_1()
                    .border_color(rgba(HUD_BORDER))
                    .child(mono(roman(i + 1), 9.0, BRASS).font_weight(FontWeight::BOLD))
                    .child(mono(b.title(), 11.0, PEARL)),
            );
        }

        div()
            .absolute()
            .top(px(18.0))
            .left(px(22.0))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.0))
            .child(hull_card)
            .child(abilities_row)
            .when(!self.boons.is_empty(), |hud| hud.child(endorsements))
    }

    /// Top-center HUD: BotW-style cinematic boss health bar when the Postmaster is present.
    fn hud_boss(&self) -> Option<Div> {
        let b = self
            .game
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Postmaster)?;
        let f = if b.max_hp > 0.0 {
            (b.hp / b.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let subtitle = if b.phase2 {
            "POSTMASTER'S OFFICE \u{00b7} SECOND SHIFT"
        } else {
            "POSTMASTER'S OFFICE \u{00b7} CHIEF AUTOMATON"
        };

        Some(
            div()
                .absolute()
                .top(px(18.0))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    hud_card()
                        .flex()
                        .flex_col()
                        .gap(px(5.0))
                        .w(px(360.0))
                        .px(px(16.0))
                        .py(px(9.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_baseline()
                                .justify_between()
                                .child(mono(
                                    subtitle,
                                    9.0,
                                    if b.phase2 { BRASS } else { PEARL_DIM },
                                ))
                                .child(
                                    mono(format!("{:.0}%", f * 100.0), 10.0, WAX)
                                        .font_weight(FontWeight::BOLD),
                                ),
                        )
                        .child(
                            serif("The Postmaster", 19.0, PEARL)
                                .font_weight(FontWeight::BOLD)
                                .line_height(px(21.0)),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(6.0))
                                .rounded(px(2.0))
                                .overflow_hidden()
                                .bg(rgba(0x13282ce0))
                                .border_1()
                                .border_color(rgba(0xede8d838))
                                .child(div().h_full().w(relative(f)).bg(rgb(WAX))),
                        ),
                ),
        )
    }

    /// Top-right HUD cluster: Chamber location title, progress pips, and run telemetry.
    fn hud_top_right(&self) -> Div {
        let g = &self.game;
        let ch = g.chamber.max(1);
        let hostiles = g.enemies.len();

        let (phase_text, phase_color) = if self.paused {
            ("PAUSED", BRASS)
        } else {
            match g.phase {
                Phase::Ready => ("STANDBY", GLOW),
                Phase::Combat if g.lesson.is_some() => ("DRILL", GLOW),
                Phase::Combat => ("UNDER TOW", WAX),
                Phase::Choice => ("ENDORSEMENT DUE", BRASS),
                Phase::Won => ("DELIVERED", BRASS),
                Phase::Lost => ("RETURNED", WAX),
            }
        };

        let mut bays = div().flex().flex_row().gap(px(4.0)).w_full();
        for i in 1..=CHAMBERS {
            let c = if i < ch || g.phase == Phase::Won {
                BRASS
            } else if i == ch && g.phase != Phase::Ready {
                GLOW
            } else {
                TEAL_HI
            };
            bays = bays.child(div().flex_1().h(px(3.0)).rounded(px(1.5)).bg(rgb(c)));
        }

        let location_card = hud_card()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(5.0))
            .min_w(px(210.0))
            .px(px(13.0))
            .py(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .gap(px(10.0))
                    .child(mono(phase_text, 9.0, phase_color).font_weight(FontWeight::BOLD))
                    .child(mono(format!("DOC {}", self.doc_no()), 9.0, PEARL_DIM)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .justify_between()
                    .w_full()
                    .gap(px(12.0))
                    .child(
                        serif(engine::chamber_name(ch), 17.0, PEARL)
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(19.0)),
                    )
                    .child(
                        mono(
                            format!("CH {}/{}", roman(ch.min(9)), roman(CHAMBERS)),
                            10.0,
                            BRASS,
                        )
                        .font_weight(FontWeight::BOLD),
                    ),
            )
            .child(bays);

        let stat_cell = |label: &'static str, val: String, vc: u32| {
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(5.0))
                .child(mono(label, 9.0, PEARL_DIM))
                .child(mono(val, 11.0, vc).font_weight(FontWeight::BOLD))
        };

        let v_sep = || div().w(px(1.0)).h(px(11.0)).bg(rgba(HUD_RULE));

        let telemetry_card = hud_card()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(5.0))
            .child(stat_cell(
                "HOSTILES",
                format!("{}", hostiles),
                if hostiles > 0 { WAX } else { PEARL_DIM },
            ))
            .child(v_sep())
            .child(stat_cell("SORTED", format!("{}", g.kills), PEARL))
            .child(v_sep())
            .child(stat_cell("TIME", clock(g.time), PEARL));

        div()
            .absolute()
            .top(px(18.0))
            .right(px(22.0))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.0))
            .child(location_card)
            .child(telemetry_card)
    }

    /// Bottom-centre combat HUD during the opening drill: the action being
    /// asked for, both ways to perform it, and a skip control. Progress comes
    /// from `Game::lesson`.
    fn hud_drill(&self, cx: &mut Context<Self>) -> Option<Div> {
        let lesson = self.game.lesson?;
        if self.paused || self.game.phase != Phase::Combat {
            return None;
        }
        let (title, keys, detail): (&str, &[&str], &str) = match lesson {
            Lesson::Strike => (
                "Strike",
                &["LMB", "J"],
                "Swing the franking blade toward your aim. Aim with the mouse, or turn with \
                 WASD / arrows. Three swings in a row chain into a heavier finisher.",
            ),
            Lesson::Cast => (
                "Cast",
                &["RMB", "K"],
                "Throw a sealed letter toward your aim. It recharges after each throw.",
            ),
            Lesson::Dash => (
                "Dash",
                &["SPACE"],
                "Slip through danger: nothing can hurt the courier mid-dash. Dash the way you \
                 are moving.",
            ),
            Lesson::Complete => (
                "Drill complete",
                &[],
                "The Sorting Floor opens. Clerks inbound.",
            ),
        };

        let mut pips = div().flex().flex_row().gap(px(4.0));
        for i in 0..Lesson::STEPS.len() {
            let lit = i < lesson.done();
            let current = i == lesson.done();
            pips = pips.child(div().w(px(18.0)).h(px(4.0)).rounded(px(2.0)).bg(if lit {
                rgb(GLOW)
            } else if current {
                rgb(PEARL_DIM)
            } else {
                rgba(0x13282cc8)
            }));
        }

        let mut prompt = div().flex().flex_row().items_center().gap(px(6.0));
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                prompt = prompt.child(mono("OR", 9.0, PEARL_DIM));
            }
            prompt = prompt.child(keycap(*key));
        }

        let skip = div()
            .id("skip-drill")
            .px(px(10.0))
            .py(px(3.0))
            .rounded(px(3.0))
            .border_1()
            .border_color(rgba(HUD_BORDER))
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(GLOW)))
            // Handle the press here, not as a click, so the arena never sees
            // it as a strike.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    this.skip_drill();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(mono("TAB \u{00b7} SKIP DRILL", 9.0, PEARL_DIM));

        Some(
            div()
                .absolute()
                .bottom(px(28.0))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    hud_card()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(7.0))
                        .max_w(px(460.0))
                        .px(px(20.0))
                        .py(px(12.0))
                        .border_color(rgb(TEAL_HI))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    mono("OPENING DRILL", 9.0, GLOW).font_weight(FontWeight::BOLD),
                                )
                                .child(pips),
                        )
                        .child(
                            serif(title, 24.0, PEARL)
                                .font_weight(FontWeight::BOLD)
                                .line_height(px(26.0)),
                        )
                        .when(!keys.is_empty(), |card| card.child(prompt))
                        .child(
                            div()
                                .font_family(MONO)
                                .text_size(px(11.0))
                                .text_color(rgb(PEARL_DIM))
                                .text_center()
                                .child(detail),
                        )
                        .when(lesson != Lesson::Complete, |card| card.child(skip)),
                ),
        )
    }

    /// Bottom-center HUD: subtle BotW-style control button prompts.
    fn hud_bottom_controls(&self) -> Div {
        div()
            .absolute()
            .bottom(px(14.0))
            .left_0()
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .px(px(14.0))
                    .py(px(5.0))
                    .rounded(px(4.0))
                    .bg(rgba(HUD_BG_SOFT))
                    .border_1()
                    .border_color(rgba(HUD_BORDER))
                    .child(control_hint("WASD / ARROWS", "Move"))
                    .child(control_hint("MOUSE", "Aim"))
                    .child(control_hint("LMB / J", "Strike"))
                    .child(control_hint("RMB / K", "Cast"))
                    .child(control_hint("SPACE", "Dash"))
                    .child(control_hint("ESC", "Pause")),
            )
    }

    fn overlay(&self, m: Metrics, cx: &mut Context<Self>) -> Option<Div> {
        let veil = |d: Div| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(16.0))
                .bg(rgba(0x040b0dd8))
        };

        if self.paused {
            let p = &self.game.player;
            let mut panel = veil(div())
                .child(mono("HELD AT THE TIDELINE", 10.0, BRASS))
                .child(
                    serif("Paused", 38.0, PEARL)
                        .font_weight(FontWeight::BOLD)
                        .line_height(px(42.0)),
                )
                .child(rule_alpha(0xe5c14855).w(px(260.0)))
                .child(mono(
                    format!(
                        "CHAMBER {}/{} \u{00b7} HULL {:.0}/{:.0} \u{00b7} {} SORTED \u{00b7} {}",
                        self.game.chamber.max(1),
                        CHAMBERS,
                        p.hp.max(0.0).ceil(),
                        p.max_hp,
                        self.game.kills,
                        clock(self.game.time)
                    ),
                    11.0,
                    PEARL_DIM,
                ));

            if !self.boons.is_empty() {
                let mut list = div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(4.0))
                    .bg(rgba(HUD_BG))
                    .border_1()
                    .border_color(rgba(HUD_BORDER));
                for (i, b) in self.boons.iter().enumerate() {
                    list = list.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(8.0))
                            .child(mono(roman(i + 1), 10.0, BRASS).font_weight(FontWeight::BOLD))
                            .child(mono(b.title(), 11.0, PEARL).font_weight(FontWeight::BOLD))
                            .child(mono(
                                format!("\u{2014} {}", b.description()),
                                10.0,
                                PEARL_DIM,
                            )),
                    );
                }
                panel = panel.child(list);
            }

            return Some(
                panel.child(
                    div()
                        .mt(px(4.0))
                        .px(px(16.0))
                        .py(px(7.0))
                        .rounded(px(4.0))
                        .bg(rgba(HUD_BG))
                        .border_1()
                        .border_color(rgb(BRASS))
                        .child(
                            mono("PRESS ESC, ENTER OR CLICK TO RESUME", 11.0, PEARL)
                                .font_weight(FontWeight::BOLD),
                        ),
                ),
            );
        }

        match self.game.phase {
            Phase::Ready => Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_between()
                    .pt(px(64.0))
                    .pb(px(64.0))
                    .child(
                        hud_card()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(28.0))
                            .py(px(16.0))
                            .child(
                                mono("THE UNDERTOW POST \u{00b7} FORM 7-B", 10.0, BRASS)
                                    .font_weight(FontWeight::BOLD),
                            )
                            .child(
                                serif("DEAD LETTER", 44.0, PEARL)
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(46.0)),
                            )
                            .child(rule_alpha(0xe5c14866).w(px(220.0)))
                            .child(mono("NO ADDRESS BEYOND THE TIDE", 10.0, PEARL_DIM)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                hud_card()
                                    .px(px(20.0))
                                    .py(px(9.0))
                                    .border_color(rgb(BRASS))
                                    .child(
                                        mono(
                                            "PRESS ENTER, SPACE OR CLICK TO DISPATCH",
                                            11.0,
                                            PEARL,
                                        )
                                        .font_weight(FontWeight::BOLD),
                                    ),
                            )
                            .child(mono(
                                // The title screen only shows before the session's first
                                // dispatch, which always opens with the drill.
                                "A SHORT DRILL OPENS THE FIRST DELIVERY \u{00b7} TAB SKIPS IT",
                                10.0,
                                PEARL_DIM,
                            )),
                    ),
            ),
            Phase::Choice => {
                let card_w = ((m.bw - 120.0) / 3.0).clamp(180.0, 260.0);
                let mut cards = div().flex().flex_row().gap(px(16.0));
                for i in 0..3 {
                    let b = self.game.choices[i];
                    cards = cards.child(
                        div()
                            .id(("boon", i))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .w(px(card_w))
                            .p(px(16.0))
                            .rounded(px(4.0))
                            .bg(rgba(0x071a1ef0))
                            .border_1()
                            .border_color(rgb(BRASS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(TEAL)).border_color(rgb(GLOW)))
                            .on_click(cx.listener(move |this, _e, _w, cx| {
                                this.pick(i);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .child(mono(
                                        format!("ENDORSEMENT {}", roman(i + 1)),
                                        9.0,
                                        PEARL_DIM,
                                    ))
                                    .child(keycap(format!("{}", i + 1))),
                            )
                            .child(rule(TEAL_HI))
                            .child(serif(b.title(), 19.0, PEARL).font_weight(FontWeight::BOLD))
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_size(px(11.0))
                                    .text_color(rgb(PEARL_DIM))
                                    .child(b.description()),
                            ),
                    );
                }
                Some(
                    veil(div())
                        .child(
                            mono(
                                format!(
                                    "CHAMBER {} SORTED \u{00b7} +25 HULL RESTORED",
                                    roman(self.game.chamber.min(9))
                                ),
                                10.0,
                                BRASS,
                            )
                            .font_weight(FontWeight::BOLD),
                        )
                        .child(
                            serif("Select an Endorsement", 32.0, PEARL)
                                .font_weight(FontWeight::BOLD)
                                .line_height(px(36.0)),
                        )
                        .child(mono("PICK ONE STAMP FOR THE SATCHEL", 10.0, PEARL_DIM))
                        .child(cards)
                        .child(mono(
                            "PRESS 1 \u{00b7} 2 \u{00b7} 3 OR CLICK A CARD",
                            10.0,
                            PEARL_DIM,
                        )),
                )
            }
            Phase::Won => Some(self.end_card(true, cx)),
            Phase::Lost => Some(self.end_card(false, cx)),
            _ => None,
        }
    }

    fn end_card(&self, won: bool, cx: &mut Context<Self>) -> Div {
        let (badge, title, sub, color) = if won {
            (
                "DISPATCH COMPLETE",
                "Delivered",
                "THE LAST LETTER REACHED A HAND THAT COULD HOLD IT.",
                BRASS,
            )
        } else {
            (
                "DISPATCH FAILED",
                "Returned to Sender",
                "HULL BREACHED. THE TIDE KEEPS THE COURIER.",
                WAX,
            )
        };
        let mut card = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.0))
            .bg(rgba(0x040b0de0))
            .child(mono(badge, 10.0, color).font_weight(FontWeight::BOLD))
            .child(
                serif(title, 42.0, color)
                    .font_weight(FontWeight::BOLD)
                    .line_height(px(46.0)),
            )
            .child(mono(sub, 11.0, PEARL))
            .child(rule_alpha(0xede8d833).w(px(280.0)))
            .child(mono(
                format!(
                    "CHAMBER {}/{} \u{00b7} {} SORTED \u{00b7} {} \u{00b7} DOC {}",
                    self.game.chamber.max(1),
                    CHAMBERS,
                    self.game.kills,
                    clock(self.game.time),
                    self.doc_no()
                ),
                11.0,
                PEARL_DIM,
            ));

        if !self.boons.is_empty() {
            let mut row = div().flex().flex_row().gap(px(8.0));
            for (i, b) in self.boons.iter().enumerate() {
                row = row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(3.0))
                        .bg(rgba(HUD_BG))
                        .border_1()
                        .border_color(rgba(HUD_BORDER))
                        .child(mono(roman(i + 1), 9.0, BRASS).font_weight(FontWeight::BOLD))
                        .child(mono(b.title(), 10.0, PEARL)),
                );
            }
            card = card.child(row);
        }

        card.child(
            div()
                .id("restart-btn")
                .mt(px(6.0))
                .px(px(18.0))
                .py(px(8.0))
                .rounded(px(4.0))
                .bg(rgb(color))
                .cursor_pointer()
                .on_click(cx.listener(|this, _e, _w, cx| {
                    this.restart(false);
                    cx.notify();
                }))
                .child(
                    mono("ENTER / R \u{2014} SEND ANOTHER COURIER", 11.0, INK)
                        .font_weight(FontWeight::BOLD),
                ),
        )
        .child(
            div()
                .id("drill-btn")
                .px(px(14.0))
                .py(px(5.0))
                .rounded(px(4.0))
                .bg(rgba(HUD_BG))
                .border_1()
                .border_color(rgba(HUD_BORDER))
                .cursor_pointer()
                .hover(|s| s.border_color(rgb(GLOW)))
                .on_click(cx.listener(|this, _e, _w, cx| {
                    this.restart(true);
                    cx.notify();
                }))
                .child(mono("T \u{2014} RUN THE DRILL FIRST", 10.0, PEARL_DIM)),
        )
    }
}

// ------------------------------------------------------------------- main ---
fn install_panic_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let path = std::env::temp_dir().join("dead-letter-crash.log");
        let _ = std::fs::write(&path, format!("{info}\n"));
        default(info);
    }));
}

fn main() {
    install_panic_log();
    Application::new().run(|cx: &mut App| {
        // GPUI exposes only full display bounds (logical px, already DPI-scaled), not the
        // work area, so reserve a conservative margin for the taskbar and window chrome.
        let (win_w, win_h, origin) = match cx.primary_display() {
            Some(display) => {
                let db = display.bounds();
                let w = (f32::from(db.size.width) - 48.0).clamp(0.0, 1920.0);
                let h = (f32::from(db.size.height) - 150.0).clamp(0.0, 1200.0);
                let (w, h) = (
                    w.max(820.0).min(f32::from(db.size.width)),
                    h.max(560.0).min(f32::from(db.size.height)),
                );
                // Client origin: centered horizontally, vertically centered in the area above
                // a ~64px taskbar reserve, leaving room for the title bar above the client.
                let x = f32::from(db.origin.x) + (f32::from(db.size.width) - w) / 2.0;
                let usable_h = f32::from(db.size.height) - 64.0;
                let y = f32::from(db.origin.y) + ((usable_h - h) / 2.0 + 16.0).max(32.0);
                (w, h, Some(gpui::point(px(x), px(y))))
            }
            None => (1280.0, 800.0, None),
        };
        let bounds = match origin {
            Some(o) => Bounds::new(o, size(px(win_w), px(win_h))),
            None => Bounds::centered(None, size(px(win_w), px(win_h)), cx),
        };
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("DEAD LETTER \u{2014} The Undertow Post".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(820.0), px(560.0))),
                focus: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        );
        if let Err(err) = opened {
            panic!("dead-letter: could not open window: {err:#}");
        }
        cx.activate(true);
    });
}
