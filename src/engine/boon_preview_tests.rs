//! `Game::boon_preview` must state what `Game::choose` really does: each case
//! measures the live game before, and an identical game after, the choice.
use super::*;

const TOL: f32 = 1e-3;

fn near(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < TOL, "{what}: {a} vs {b}");
}

fn near3(a: [f32; 3], b: [f32; 3], what: &str) {
    for i in 0..3 {
        near(a[i], b[i], &format!("{what}[{i}]"));
    }
}

/// A game in `Phase::Choice` that already took `prior` through real choices,
/// then wounded to `hp` (never above its max hull).
fn staged(prior: &[Boon], hp: f32) -> Game {
    let mut g = Game::quiet_arena(11);
    for &b in prior {
        g.phase = Phase::Choice;
        g.choices = [b; 3];
        g.choose(0);
    }
    g.phase = Phase::Choice;
    g.player.hp = hp.min(g.player.max_hp);
    g
}

fn dummy(g: &mut Game, pos: Vec2) {
    let mut e = g.make_enemy(EnemyKind::Bailiff, pos);
    e.emerging = false;
    e.begin(S_MOVE, 0.0);
    e.cd = f32::INFINITY;
    e.hp = 1000.0;
    e.max_hp = 1000.0;
    g.enemies.clear();
    g.enemies.push(e);
}

/// Damage and reach of the three combo steps, from real strikes.
fn strikes(g: &mut Game) -> ([f32; 3], [f32; 3]) {
    let (mut dmg, mut reach) = ([0.0; 3], [0.0; 3]);
    g.player.combo_timer = 0.0;
    for step in 0..3 {
        g.player.pos = Vec2::ZERO;
        g.player.facing = Vec2::new(1.0, 0.0);
        g.effects.clear();
        dummy(g, Vec2::new(1.0, 0.0));
        g.do_strike();
        dmg[step] = 1000.0 - g.enemies[0].hp;
        reach[step] = g
            .effects
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EffectKind::Strike(_)))
            .expect("strike effect")
            .radius;
    }
    (dmg, reach)
}

/// Damage, piercing and installed cooldown of a real cast.
fn cast(g: &mut Game) -> (f32, bool, f32) {
    g.player.cast_cooldown = 0.0;
    g.shots.clear();
    g.do_cast();
    let s = g.shots.last().expect("cast shot");
    (s.damage, s.pierce, g.player.cast_cooldown)
}

/// Installed dash cooldown and invulnerability of a real dash.
fn dash(g: &mut Game) -> (f32, f32) {
    g.enemies.clear();
    g.player.dash_cooldown = 0.0;
    g.player.dash_time = 0.0;
    g.player.invuln = 0.0;
    g.prev_dash = false;
    g.update_player(&Input {
        dash: true,
        ..Input::default()
    });
    assert!(g.player.dashing());
    (g.player.dash_cooldown, g.player.invuln)
}

/// Units walked per second.
fn walk_speed(g: &mut Game) -> f32 {
    g.player.pos = Vec2::ZERO;
    g.player.dash_time = 0.0;
    g.update_player(&Input {
        movement: Vec2::new(1.0, 0.0),
        ..Input::default()
    });
    g.player.pos.x / DT
}

/// Hull restored by one real cancellation.
fn kill_heal(g: &mut Game) -> f32 {
    dummy(g, Vec2::new(2.0, 0.0));
    g.enemies[0].hp = 0.0;
    g.player.hp = 10.0;
    g.player.max_hp = g.player.max_hp.max(100.0);
    g.resolve_deaths();
    g.player.hp - 10.0
}

/// Whether a dash through an adjacent enemy rips it, and for how much.
fn lash(g: &mut Game) -> (bool, f32) {
    g.player.pos = Vec2::ZERO;
    dummy(g, Vec2::new(0.5, 0.0));
    g.player.dash_cooldown = 0.0;
    g.player.dash_time = 0.0;
    g.prev_dash = false;
    g.update_player(&Input {
        dash: true,
        ..Input::default()
    });
    let dealt = 1000.0 - g.enemies[0].hp;
    (dealt > 0.0, dealt)
}

/// Preview `b` on one game, choose it on an identical one, and compare.
fn check(prior: &[Boon], b: Boon, hp: f32) {
    let mut a = staged(prior, hp);
    let mut after = staged(prior, hp);
    a.choices[1] = b;
    after.choices[1] = b;
    let what = format!("{b:?} after {prior:?}");
    let preview = a.boon_preview(b);
    after.choose(1);
    assert_eq!(after.phase, Phase::Combat, "{what}");

    match (b, preview) {
        (Boon::FrankingHammer, BoonPreview::StrikeDamage(c)) => {
            near3(strikes(&mut a).0, c.before, &what);
            near3(strikes(&mut after).0, c.after, &what);
        }
        (Boon::TidewaterReach, BoonPreview::StrikeReach(c)) => {
            near3(strikes(&mut a).1, c.before, &what);
            near3(strikes(&mut after).1, c.after, &what);
        }
        (Boon::RegisteredMail, BoonPreview::Cast { damage, pierce }) => {
            let (before, after) = (cast(&mut a), cast(&mut after));
            near(before.0, damage.before, &what);
            near(after.0, damage.after, &what);
            assert_eq!((before.1, after.1), (pierce.before, pierce.after), "{what}");
        }
        (Boon::QuickDelivery, BoonPreview::CastCooldown(c)) => {
            near(cast(&mut a).2, c.before, &what);
            near(cast(&mut after).2, c.after, &what);
        }
        (
            Boon::SlipstreamTag,
            BoonPreview::Dash {
                cooldown,
                invulnerability,
            },
        ) => {
            let (before, after) = (dash(&mut a), dash(&mut after));
            near(before.0, cooldown.before, &what);
            near(after.0, cooldown.after, &what);
            near(before.1, invulnerability.before, &what);
            near(after.1, invulnerability.after, &what);
        }
        (Boon::PressureValve, BoonPreview::Hull { max, on_entry }) => {
            near(a.player.max_hp, max.before, &what);
            near(after.player.max_hp, max.after, &what);
            near(a.player.hp, on_entry.before, &what);
            near(after.player.hp, on_entry.after, &what);
        }
        (Boon::ReturnReceipt, BoonPreview::HullPerKill(c)) => {
            near(kill_heal(&mut a), c.before, &what);
            near(kill_heal(&mut after), c.after, &what);
        }
        (Boon::ExpressPost, BoonPreview::MoveSpeed(c)) => {
            let (before, after) = (walk_speed(&mut a), walk_speed(&mut after));
            assert!(c.before > 0.0, "{what}");
            // Holds whether the card shows units per second or a multiplier.
            near(after / before, c.after / c.before, &what);
        }
        (Boon::UndertowLash, BoonPreview::DashLash { enabled, damage }) => {
            let (before, after) = (lash(&mut a), lash(&mut after));
            assert_eq!(
                (before.0, after.0),
                (enabled.before, enabled.after),
                "{what}"
            );
            assert!(after.0);
            near(after.1, damage, &what);
        }
        (b, p) => panic!("{b:?} previewed as {p:?}"),
    }
}

/// Each boon alone, repeated, and on top of every other boon.
fn stacks_of(b: Boon) -> [Vec<Boon>; 4] {
    [
        vec![],
        vec![b],
        vec![b, b],
        Boon::ALL
            .into_iter()
            .filter(|&o| o != b)
            .chain([b])
            .collect(),
    ]
}

#[test]
fn every_preview_matches_what_choose_does_alone_and_stacked() {
    for b in Boon::ALL {
        for prior in stacks_of(b) {
            check(&prior, b, 40.0);
        }
    }
}

#[test]
fn stacked_multiplicative_and_additive_previews_start_from_the_live_value() {
    let mut g = staged(&[Boon::FrankingHammer], 100.0);
    let BoonPreview::StrikeDamage(c) = g.boon_preview(Boon::FrankingHammer) else {
        panic!()
    };
    near3(
        c.before,
        [14.0 * 1.35, 15.0 * 1.35, 28.0 * 1.35],
        "hammer before",
    );
    near3(
        c.after,
        [14.0 * 1.35 * 1.35, 15.0 * 1.35 * 1.35, 28.0 * 1.35 * 1.35],
        "hammer after",
    );

    g = staged(&[Boon::QuickDelivery], 100.0);
    let BoonPreview::CastCooldown(c) = g.boon_preview(Boon::QuickDelivery) else {
        panic!()
    };
    near(c.before, BASE_CAST_COOLDOWN * 0.65, "cast cd before");
    near(c.after, BASE_CAST_COOLDOWN * 0.65 * 0.65, "cast cd after");

    g = staged(&[Boon::SlipstreamTag], 100.0);
    let BoonPreview::Dash {
        cooldown,
        invulnerability,
    } = g.boon_preview(Boon::SlipstreamTag)
    else {
        panic!()
    };
    near(cooldown.after, BASE_DASH_COOLDOWN * 0.49, "dash cd");
    near(
        invulnerability.before,
        DASH_IFRAMES + 0.12,
        "iframes before",
    );
    near(invulnerability.after, DASH_IFRAMES + 0.24, "iframes after");

    g = staged(&[Boon::ReturnReceipt], 100.0);
    let BoonPreview::HullPerKill(c) = g.boon_preview(Boon::ReturnReceipt) else {
        panic!()
    };
    near(c.before, 6.0, "kill heal before");
    near(c.after, 12.0, "kill heal after");
}

#[test]
fn owned_bool_effects_preview_as_unchanged_and_never_double() {
    let mut g = staged(&[Boon::UndertowLash], 100.0);
    let BoonPreview::DashLash { enabled, damage } = g.boon_preview(Boon::UndertowLash) else {
        panic!()
    };
    assert_eq!((enabled.before, enabled.after, damage), (true, true, 14.0));
    g.choices[0] = Boon::UndertowLash;
    g.choose(0);
    near(lash(&mut g).1, 14.0, "lash after repeat");

    let g = staged(&[Boon::RegisteredMail], 100.0);
    let BoonPreview::Cast { damage, pierce } = g.boon_preview(Boon::RegisteredMail) else {
        panic!()
    };
    assert_eq!((pierce.before, pierce.after), (true, true));
    near(damage.before, CAST_DAMAGE * 1.25, "mail before");
    near(damage.after, CAST_DAMAGE * 1.25 * 1.25, "mail after");

    let fresh = staged(&[], 100.0).boon_preview(Boon::UndertowLash);
    let BoonPreview::DashLash { enabled, .. } = fresh else {
        panic!()
    };
    assert_eq!((enabled.before, enabled.after), (false, true));
}

#[test]
fn hull_entry_is_capped_by_the_new_maximum() {
    // (hp, expected hull on entering the next chamber)
    for (hp, entry) in [(100.0, 130.0), (90.0, 130.0), (60.0, 115.0), (20.0, 75.0)] {
        let mut g = staged(&[], hp);
        let BoonPreview::Hull { max, on_entry } = g.boon_preview(Boon::PressureValve) else {
            panic!()
        };
        near(max.before, 100.0, "max before");
        near(max.after, 130.0, "max after");
        near(on_entry.before, hp, "entry before");
        near(on_entry.after, entry, "entry after");
        g.choices[0] = Boon::PressureValve;
        g.choose(0);
        near(g.player.hp, entry, "real hull on entry");
    }
    // Stacked: 130 -> 160, still capped at the new maximum.
    let g = staged(&[Boon::PressureValve], 130.0);
    let BoonPreview::Hull { max, on_entry } = g.boon_preview(Boon::PressureValve) else {
        panic!()
    };
    near(max.before, 130.0, "stack max before");
    near(max.after, 160.0, "stack max after");
    near(on_entry.after, 160.0, "stack entry after");
}

#[test]
fn previewing_changes_nothing_and_offers_stay_seeded_and_exclude_owned() {
    let (mut plain, mut peeked) = (
        staged(&[Boon::ExpressPost], 70.0),
        staged(&[Boon::ExpressPost], 70.0),
    );
    let state = |g: &Game| {
        format!(
            "{:?} {:?} {} {} {}",
            g.mods, g.choices, g.player.hp, g.player.max_hp, g.rng
        )
    };
    let before = state(&peeked);
    for b in Boon::ALL {
        peeked.boon_preview(b);
    }
    assert_eq!(state(&peeked), before, "preview mutated the game");
    plain.roll_choices();
    peeked.roll_choices();
    assert_eq!(plain.choices, peeked.choices);
    for b in Boon::ALL {
        let mut g = staged(&[b], 100.0);
        for _ in 0..20 {
            g.roll_choices();
            assert!(!g.choices.contains(&b), "{b:?} offered while owned");
        }
    }
}
