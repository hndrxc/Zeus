//! Explorable halls: the layouts the courier walks before a chamber encounter.
//!
//! Layouts live in `assets/floors/<id>.floor` (the same files drive the Blender
//! bake). They are embedded and parsed once. This module also owns the hall
//! collision (a circle against axis-aligned boxes) and the camera clamp.

use std::sync::LazyLock;

use crate::engine::Vec2;

/// Largest distance a collision sub-step moves, below a railing's thickness.
const MAX_STEP: f32 = 0.1;
/// Design px per world unit along the projected axes (see `scene.rs`).
const U_PER_UNIT: f32 = 44.0;
const V_PER_UNIT: f32 = 24.2;
/// The design-space view the camera must keep inside the floor image.
const VIEW_W: f32 = 900.0;
const VIEW_H: f32 = 600.0;
/// Design px row where the camera point lands in the view.
const VIEW_ANCHOR_Y: f32 = 308.0;

#[derive(Clone, Debug)]
pub struct Block {
    pub min: Vec2,
    pub max: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorKind {
    Stop,
    Side,
}

#[derive(Clone, Debug)]
pub struct Door {
    pub kind: DoorKind,
    pub min: Vec2,
    pub max: Vec2,
}

impl Door {
    /// True when `p` lies inside the threshold rectangle.
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

#[derive(Clone, Debug)]
pub struct Floor {
    pub id: &'static str,
    /// Design px: baked image size, and where world (0, 0, 0) lands in it.
    pub image_size: (f32, f32),
    pub origin: (f32, f32),
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
    pub entrance: Vec2,
    pub entrance_facing: Vec2,
    pub blocks: Vec<Block>,
    pub doors: Vec<Door>,
    pub routes: Vec<(&'static str, Vec<Vec2>)>,
}

impl Floor {
    pub fn route(&self, name: &str) -> Option<&[Vec2]> {
        self.routes
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, pts)| pts.as_slice())
    }

    pub fn door(&self, kind: DoorKind) -> Option<&Door> {
        self.doors.iter().find(|d| d.kind == kind)
    }

    /// Move a circle of radius `r` from `from` by `delta`. The move is cut into
    /// steps of at most `MAX_STEP`; each step resolves x, then y, against every
    /// block expanded by `r` (square corners) and the bounds shrunk by `r`, so
    /// the circle slides along walls and cannot tunnel through a thin railing,
    /// however fast it travels.
    pub fn slide(&self, from: Vec2, delta: Vec2, r: f32) -> Vec2 {
        let steps = ((delta.len() / MAX_STEP).ceil() as usize).max(1);
        let step = delta * (1.0 / steps as f32);
        let mut p = from;
        for _ in 0..steps {
            p.x = self.resolve_x(p, step.x, r);
            p.y = self.resolve_y(p, step.y, r);
        }
        p
    }

    fn resolve_x(&self, p: Vec2, dx: f32, r: f32) -> f32 {
        if dx == 0.0 {
            return p.x;
        }
        let mut x = (p.x + dx).clamp(self.bounds_min.x + r, self.bounds_max.x - r);
        // A snap can land inside a neighbouring block; settle until stable.
        for _ in 0..=self.blocks.len() {
            let mut moved = false;
            for b in &self.blocks {
                let hit =
                    x > b.min.x - r && x < b.max.x + r && p.y > b.min.y - r && p.y < b.max.y + r;
                if hit {
                    x = if dx > 0.0 { b.min.x - r } else { b.max.x + r };
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        x
    }

    fn resolve_y(&self, p: Vec2, dy: f32, r: f32) -> f32 {
        if dy == 0.0 {
            return p.y;
        }
        let mut y = (p.y + dy).clamp(self.bounds_min.y + r, self.bounds_max.y - r);
        for _ in 0..=self.blocks.len() {
            let mut moved = false;
            for b in &self.blocks {
                let hit =
                    p.x > b.min.x - r && p.x < b.max.x + r && y > b.min.y - r && y < b.max.y + r;
                if hit {
                    y = if dy > 0.0 { b.min.y - r } else { b.max.y + r };
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        y
    }

    /// Clamp the camera point so the 900x600 design view stays inside the
    /// floor image. With `cu = 44(Cx + Cy)` and `cv = 24.2(Cx - Cy)` the view
    /// is a plain rectangle in `(cu, cv)`.
    pub fn clamp_camera(&self, c: Vec2) -> Vec2 {
        let (w, h) = self.image_size;
        let (ox, oy) = self.origin;
        let cu = clamp_span(
            U_PER_UNIT * (c.x + c.y),
            VIEW_W / 2.0 - ox,
            w - ox - VIEW_W / 2.0,
        );
        let cv = clamp_span(
            V_PER_UNIT * (c.x - c.y),
            VIEW_ANCHOR_Y - oy,
            h - oy - (VIEW_H - VIEW_ANCHOR_Y),
        );
        let sum = cu / U_PER_UNIT;
        let diff = cv / V_PER_UNIT;
        Vec2::new((sum + diff) * 0.5, (sum - diff) * 0.5)
    }
}

/// Clamp into `[lo, hi]`; an image smaller than the view centres on the span.
fn clamp_span(v: f32, lo: f32, hi: f32) -> f32 {
    if lo > hi {
        (lo + hi) * 0.5
    } else {
        v.clamp(lo, hi)
    }
}

/// The hall walked before `chamber` (1-based): 1 → sorting_hall, 2 → vault_stacks, otherwise None.
pub fn floor_for(chamber: usize) -> Option<&'static Floor> {
    match chamber {
        1 => Some(&SORTING_HALL),
        2 => Some(&VAULT_STACKS),
        _ => None,
    }
}

static SORTING_HALL: LazyLock<Floor> = LazyLock::new(|| {
    parse(
        "sorting_hall.floor",
        include_str!("../assets/floors/sorting_hall.floor"),
    )
});

static VAULT_STACKS: LazyLock<Floor> = LazyLock::new(|| {
    parse(
        "vault_stacks.floor",
        include_str!("../assets/floors/vault_stacks.floor"),
    )
});

fn fail(file: &str, line: usize, msg: &str) -> ! {
    panic!("{file}:{line}: {msg}")
}

fn missing(file: &str, what: &str) -> ! {
    panic!("{file}: missing `{what}` line")
}

fn parse(file: &str, src: &'static str) -> Floor {
    let mut id = None;
    let mut image = None;
    let mut bounds = None;
    let mut entrance = None;
    let mut blocks = Vec::new();
    let mut doors = Vec::new();
    let mut routes: Vec<(&'static str, Vec<Vec2>)> = Vec::new();

    for (n, raw) in src.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut tok = line.split_whitespace();
        let key = tok
            .next()
            .unwrap_or_else(|| fail(file, n + 1, "empty line"));
        let rest: Vec<&'static str> = tok.collect();
        let nums = |from: usize| -> Vec<f32> {
            rest[from..]
                .iter()
                .map(|t| {
                    t.parse::<f32>()
                        .unwrap_or_else(|_| fail(file, n + 1, &format!("`{t}` is not a number")))
                })
                .collect()
        };
        let need = |from: usize, count: usize| -> Vec<f32> {
            if rest.len() < from {
                fail(file, n + 1, &format!("`{key}` is missing fields"));
            }
            let v = nums(from);
            if v.len() != count {
                fail(
                    file,
                    n + 1,
                    &format!("`{key}` needs {count} numbers, found {}", v.len()),
                );
            }
            v
        };
        let name = |what: &str, allowed: &[&str]| -> &'static str {
            let Some(w) = rest.first() else {
                fail(file, n + 1, &format!("`{key}` needs a {what}"));
            };
            if !allowed.contains(w) {
                fail(file, n + 1, &format!("unknown {what} `{w}`"));
            }
            *w
        };
        match key {
            "floor" => {
                id = Some(
                    *rest
                        .first()
                        .unwrap_or_else(|| fail(file, n + 1, "`floor` needs an id")),
                );
            }
            "image" => {
                let v = need(0, 4);
                image = Some(((v[0], v[1]), (v[2], v[3])));
            }
            "bounds" => {
                let v = need(0, 4);
                if v[0] >= v[2] || v[1] >= v[3] {
                    fail(file, n + 1, "empty bounds");
                }
                bounds = Some((Vec2::new(v[0], v[1]), Vec2::new(v[2], v[3])));
            }
            "entrance" => {
                let v = need(0, 4);
                entrance = Some((Vec2::new(v[0], v[1]), Vec2::new(v[2], v[3])));
            }
            "landmark" => {
                // Read by the bake and the annotated plans.
                need(0, 2);
            }
            "block" => {
                name(
                    "block kind",
                    &[
                        "wall", "counter", "table", "rack", "stack", "railing", "landmark",
                    ],
                );
                let v = need(1, 5);
                if v[0] >= v[2] || v[1] >= v[3] {
                    fail(file, n + 1, "empty block");
                }
                blocks.push(Block {
                    min: Vec2::new(v[0], v[1]),
                    max: Vec2::new(v[2], v[3]),
                });
            }
            "door" => {
                let kind = match name("door kind", &["stop", "side"]) {
                    "stop" => DoorKind::Stop,
                    _ => DoorKind::Side,
                };
                let v = need(1, 4);
                if v[0] >= v[2] || v[1] >= v[3] {
                    fail(file, n + 1, "empty door");
                }
                doors.push(Door {
                    kind,
                    min: Vec2::new(v[0], v[1]),
                    max: Vec2::new(v[2], v[3]),
                });
            }
            "route" => {
                let label = name("route name", &["main", "loop", "side"]);
                let v = need(1, rest.len() - 1);
                if v.len() < 4 || v.len() % 2 != 0 {
                    fail(file, n + 1, "a route needs at least two x y pairs");
                }
                routes.push((label, v.chunks(2).map(|c| Vec2::new(c[0], c[1])).collect()));
            }
            "view" => {
                // Camera focus points for the annotated plans and captures.
                name("view name", &["arrival", "junction", "return"]);
                need(1, 2);
            }
            other => fail(file, n + 1, &format!("unknown line `{other}`")),
        }
    }

    let (image_size, origin) = image.unwrap_or_else(|| missing(file, "image"));
    let (bounds_min, bounds_max) = bounds.unwrap_or_else(|| missing(file, "bounds"));
    let (entrance, entrance_facing) = entrance.unwrap_or_else(|| missing(file, "entrance"));
    let floor = Floor {
        id: id.unwrap_or_else(|| missing(file, "floor")),
        image_size,
        origin,
        bounds_min,
        bounds_max,
        entrance,
        entrance_facing,
        blocks,
        doors,
        routes,
    };
    for required in ["main", "loop", "side"] {
        if floor.route(required).is_none() {
            panic!("{file}: missing `route {required}`");
        }
    }
    for kind in [DoorKind::Stop, DoorKind::Side] {
        if floor.door(kind).is_none() {
            panic!("{file}: missing {kind:?} door");
        }
    }
    floor
}
