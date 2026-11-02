// SPDX-License-Identifier: Apache-2.0
//! The office scene: a tile-built room with carpet, a wall (window, clock, posters), furniture, a numbered
//! grid of floor tiles, an INBOX belt on the left and an OUTBOX belt on the right (with expected-output
//! ghost crates and tick/cross stamps) and the clerk, who physically walks to the tile an instruction uses.
//!
//! The scene is drawn in its own coordinates (270x176) through [`Gfx::off`]. The machine runs ahead of the
//! picture: every executed step is turned into *beats* (walk, grab, put down, sparkle ...) by `choreo` and
//! played by [`Office::tick`], so the animation speed is independent of the machine.

use crate::choreo::{Beat, Land};
use crate::clerk::{ClerkArt, ClerkState, Dir, Pose};
use crate::gfx::Gfx;
use algoclerk_media::{ArtBank, Outfit, Rect};
use std::collections::VecDeque;

/// Scene width in virtual pixels.
pub const SCENE_W: i32 = 270;
/// Scene height in virtual pixels.
pub const SCENE_H: i32 = 176;
/// Floor tile spot size.
pub const SPOT_W: i32 = 20;
/// Floor tile spot height.
pub const SPOT_H: i32 = 18;
/// Horizontal pitch of floor spots.
pub const PITCH_X: i32 = 22;
/// Vertical pitch of floor spots (spot + number label + aisle).
pub const PITCH_Y: i32 = 28;
/// Crate size.
pub const CRATE_W: i32 = 16;
/// Crate height.
pub const CRATE_H: i32 = 15;
/// Crates visible on a belt.
pub const BELT_SLOTS: usize = 7;
/// Largest number of floor tiles the scene can lay out.
pub const MAX_TILES: usize = 36;

/// Where everything is, for a given number of floor tiles.
#[derive(Debug, Clone)]
pub struct Geometry {
    /// Number of tiles.
    pub n: usize,
    /// Columns of the floor grid.
    pub cols: usize,
    /// Rows of the floor grid.
    pub rows: usize,
    /// Top-left of the grid.
    pub origin: (i32, i32),
    /// The inbox belt (surface only).
    pub in_belt: Rect,
    /// The outbox belt (surface only).
    pub out_belt: Rect,
}

impl Geometry {
    /// Lay out `n` floor tiles (1..=`MAX_TILES`).
    pub fn new(n: usize) -> Geometry {
        let n = n.clamp(1, MAX_TILES);
        let cols = if n <= 5 {
            n
        } else if n <= 12 {
            n.div_ceil(2)
        } else if n <= 24 {
            n.div_ceil(3)
        } else {
            n.div_ceil(4)
        }
        .min(9);
        let rows = n.div_ceil(cols);
        let gw = (cols as i32 - 1) * PITCH_X + SPOT_W;
        let gh = (rows as i32 - 1) * PITCH_Y + SPOT_H + 8;
        let x0 = 28 + (214 - gw) / 2;
        let y0 = 60 + (112 - gh) / 2;
        Geometry {
            n,
            cols,
            rows,
            origin: (x0, y0),
            in_belt: Rect::new(4, 52, 16, BELT_SLOTS as i32 * CRATE_W),
            out_belt: Rect::new(250, 52, 16, BELT_SLOTS as i32 * CRATE_W),
        }
    }

    /// The spot rectangle of a floor tile.
    pub fn spot(&self, slot: usize) -> Rect {
        let (c, r) = ((slot % self.cols) as i32, (slot / self.cols) as i32);
        Rect::new(self.origin.0 + c * PITCH_X, self.origin.1 + r * PITCH_Y, SPOT_W, SPOT_H)
    }

    /// Where the crate on a tile sits.
    pub fn tile_crate(&self, slot: usize) -> Rect {
        let s = self.spot(slot);
        Rect::new(s.x + (SPOT_W - CRATE_W) / 2, s.y + 2, CRATE_W, CRATE_H)
    }

    /// Where the clerk stands to work on a tile: `(centre x, feet y)`.
    pub fn stand(&self, slot: usize) -> (f32, f32) {
        let s = self.spot(slot);
        ((s.x + SPOT_W / 2) as f32, (s.y + 9) as f32)
    }

    /// Rectangle of the inbox crate `i` (0 is the one at the clerk's end, at the bottom).
    pub fn in_slot(&self, i: usize) -> Rect {
        Rect::new(
            self.in_belt.x,
            self.in_belt.bottom() - (i as i32 + 1) * CRATE_W,
            CRATE_W,
            CRATE_W,
        )
    }

    /// Rectangle of outbox slot `j` (0 is the far end, at the top).
    pub fn out_slot(&self, j: usize) -> Rect {
        Rect::new(self.out_belt.x, self.out_belt.y + j as i32 * CRATE_W, CRATE_W, CRATE_W)
    }

    /// The clerk's position at the inbox.
    pub fn inbox_station(&self) -> (f32, f32) {
        ((self.in_belt.right() + 11) as f32, (self.in_belt.bottom() - 4) as f32)
    }

    /// The clerk's position at the outbox.
    pub fn outbox_station(&self) -> (f32, f32) {
        ((self.out_belt.x - 11) as f32, (self.out_belt.bottom() - 4) as f32)
    }

    /// The clerk's home, in front of the desk.
    pub fn home(&self) -> (f32, f32) {
        (135.0, 57.0)
    }

    /// The tile slot under a scene point, if any.
    pub fn slot_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.n).find(|s| self.spot(*s).grow(2).contains(x, y))
    }
}

/// What the picture currently shows (it lags behind the machine while beats play).
#[derive(Debug, Clone, Default)]
pub struct Visual {
    /// All inbox values of the current test.
    pub inbox: Vec<i32>,
    /// How many were taken.
    pub taken: usize,
    /// Floor tiles.
    pub floor: Vec<Option<i32>>,
    /// Value in the hand.
    pub hand: Option<i32>,
    /// Delivered outbox values.
    pub outbox: Vec<i32>,
    /// Expected outbox values (empty in the sandbox).
    pub expected: Vec<i32>,
}

/// A crate in the air.
#[derive(Debug, Clone)]
pub struct Flyer {
    /// Path (crate top-left corners); the crate moves along it.
    pub path: Vec<(f32, f32)>,
    /// Value on the label.
    pub value: i32,
    /// Ticks elapsed.
    pub t: u32,
    /// Total ticks.
    pub total: u32,
    /// Ghost-less copy (tile copies are drawn lighter).
    pub copy: bool,
}

/// Small effects: sparkles with a symbol, the pointer arrow, tile flashes.
#[derive(Debug, Clone)]
pub struct Spark {
    /// Symbol drawn in the sparkle (`+ - x / %`).
    pub sym: char,
    /// Position (scene coordinates, centre).
    pub at: (f32, f32),
    /// Ticks left.
    pub ttl: u32,
}

/// The state of the beat currently playing.
#[derive(Debug, Clone)]
struct Active {
    beat: Beat,
    t: u32,
    total: u32,
}

/// The office: visual state, clerk, beat queue and effects.
#[derive(Debug, Clone)]
pub struct Office {
    /// Tile layout.
    pub geom: Geometry,
    /// What is shown.
    pub vis: Visual,
    /// The clerk.
    pub clerk: ClerkState,
    /// Values are letters.
    pub char_mode: bool,
    /// Wall clock: steps executed.
    pub steps: u64,
    /// Floor tile names (may be empty strings).
    pub names: Vec<String>,
    /// Tile highlighted as the operand being edited (editor selection).
    pub operand_focus: Option<usize>,
    /// Tiles that can be picked as an operand (highlighted).
    pub pickable: bool,
    /// Pointer arrow position (tile slot) and ticks left.
    pub pointer: Option<(usize, u32)>,
    /// Sparkles.
    pub sparks: Vec<Spark>,
    /// Crates in flight.
    pub flyers: Vec<Flyer>,
    /// Speech bubble text and ticks left (`u32::MAX` = until cleared).
    pub bubble: Option<(String, u32)>,
    /// Speed multiplier (1, 2, 4, 8) or `f32::INFINITY` for "no animation".
    pub speed: f32,
    /// Belt animation phase (advances while things move).
    belt_phase: u32,
    /// Slide animation of the inbox belt (ticks left).
    in_slide: u32,
    queue: VecDeque<Beat>,
    active: Option<Active>,
    walk_to: Option<(f32, f32)>,
    /// Tile flash timers.
    flash: Vec<u8>,
    art: ClerkArt,
    /// Clock for idle animation.
    pub clock: u64,
    /// Skip decorative animation.
    pub reduce_motion: bool,
    /// Pop-in timers for crates that just landed on a tile.
    tile_pop: Vec<u8>,
}

fn crate_variant(v: i32) -> &'static str {
    match v.rem_euclid(4) {
        0 => "tiles/crate_a",
        1 => "tiles/crate_b",
        2 => "tiles/crate_c",
        _ => "tiles/crate_d",
    }
}

/// The text printed on a crate: at most three characters.
pub fn crate_label(v: i32) -> String {
    match v {
        -99..=999 => v.to_string(),
        1000..=99_999 => format!("{}K", v / 1000),
        100_000.. => "BIG".to_string(),
        _ => "-BG".to_string(),
    }
}

/// Is `v` a printable ASCII code point (for character-mode levels)?
pub fn printable(v: i32) -> Option<char> {
    u8::try_from(v)
        .ok()
        .filter(|b| (32..127).contains(b))
        .map(|b| b as char)
}

impl Office {
    /// A fresh office for `floor_len` tiles.
    pub fn new(bank: &ArtBank, outfit: &Outfit, floor_len: usize) -> Office {
        let geom = Geometry::new(floor_len);
        let (hx, hy) = geom.home();
        Office {
            vis: Visual {
                floor: vec![None; floor_len],
                ..Visual::default()
            },
            clerk: ClerkState::new(hx, hy),
            char_mode: false,
            steps: 0,
            names: vec![String::new(); floor_len],
            operand_focus: None,
            pickable: false,
            pointer: None,
            sparks: Vec::new(),
            flyers: Vec::new(),
            bubble: None,
            speed: 1.0,
            belt_phase: 0,
            in_slide: 0,
            queue: VecDeque::new(),
            active: None,
            walk_to: None,
            flash: vec![0; floor_len],
            art: ClerkArt::new(bank, outfit),
            clock: 0,
            reduce_motion: false,
            tile_pop: vec![0; floor_len],
            geom,
        }
    }

    /// Change the clerk's outfit.
    pub fn set_outfit(&mut self, bank: &ArtBank, outfit: &Outfit) {
        self.art.set_outfit(bank, outfit);
    }

    /// Show a machine state without animation (after reset, rewind or start).
    pub fn sync(&mut self, inbox: &[i32], taken: usize, floor: &[Option<i32>], hand: Option<i32>, outbox: &[i32], steps: u64) {
        self.queue.clear();
        self.active = None;
        self.walk_to = None;
        self.flyers.clear();
        self.sparks.clear();
        self.pointer = None;
        self.bubble = None;
        self.in_slide = 0;
        self.vis.inbox = inbox.to_vec();
        self.vis.taken = taken;
        self.vis.floor = floor.to_vec();
        self.vis.hand = hand;
        self.vis.outbox = outbox.to_vec();
        self.steps = steps;
        let n = self.vis.floor.len();
        if self.geom.n != n.clamp(1, MAX_TILES) {
            self.geom = Geometry::new(n);
        }
        self.flash = vec![0; n];
        self.tile_pop = vec![0; n];
        self.names.resize(n, String::new());
        self.clerk.carrying = hand.is_some();
        self.clerk.pose = Pose::Idle;
        self.clerk.walking = false;
        self.clerk.dir = Dir::Down;
        let (hx, hy) = self.geom.home();
        self.clerk.x = hx;
        self.clerk.y = hy;
    }

    /// Set the expected outbox (empty for the sandbox).
    pub fn set_expected(&mut self, expected: &[i32]) {
        self.vis.expected = expected.to_vec();
    }

    /// True while beats are playing or queued.
    pub fn busy(&self) -> bool {
        self.active.is_some() || !self.queue.is_empty() || !self.flyers.is_empty()
    }

    /// Queue the beats of an executed step.
    pub fn enqueue(&mut self, beats: Vec<Beat>) {
        self.queue.extend(beats);
    }

    /// Instantly finish everything that is queued (used when the player steps faster than the picture).
    pub fn settle(&mut self) {
        if let Some(a) = self.active.take() {
            self.finish(a.beat);
        }
        while let Some(b) = self.queue.pop_front() {
            self.begin_instant(&b);
            self.finish(b);
        }
        self.flyers.clear();
        self.clerk.walking = false;
        self.walk_to = None;
        self.in_slide = 0;
    }

    fn mult(&self) -> f32 {
        if self.speed.is_finite() { self.speed.max(0.25) } else { 1000.0 }
    }

    fn scaled(&self, ticks: u32) -> u32 {
        ((ticks as f32 / self.mult()).round() as u32).max(1)
    }

    fn begin_instant(&mut self, b: &Beat) {
        if let Beat::Walk(x, y) = b {
            self.clerk.x = *x;
            self.clerk.y = *y;
        }
    }

    fn begin(&mut self, beat: Beat) {
        let total = match &beat {
            Beat::Walk(..) => 0,
            Beat::Pose(_, t) | Beat::Wait(t) | Beat::Bubble(_, t) => self.scaled(*t),
            Beat::Fly { ticks, .. } => self.scaled(*ticks),
            _ => 0,
        };
        match &beat {
            Beat::Walk(x, y) => {
                self.walk_to = Some((*x, *y));
            }
            Beat::Face(d) => self.clerk.dir = *d,
            Beat::Pose(p, _) => {
                self.clerk.pose = *p;
                self.clerk.clock = 0;
            }
            Beat::TakeInbox => {
                self.vis.taken += 1;
                self.in_slide = self.scaled(10);
            }
            Beat::Sparkle(sym, x, y) => self.sparks.push(Spark {
                sym: *sym,
                at: (*x, *y),
                ttl: 40,
            }),
            Beat::PointerAt(slot) => self.pointer = Some((*slot, self.scaled(40))),
            Beat::Bubble(text, t) => {
                let ttl = self.scaled(*t);
                self.bubble = Some((text.clone(), ttl));
            }
            Beat::StickyBubble(text) => self.bubble = Some((text.clone(), u32::MAX)),
            Beat::Fly { path, value, ticks, land, .. } => {
                let copy = matches!(land, Land::Tile(..));
                if matches!(land, Land::Out(..)) {
                    self.vis.hand = None;
                    self.clerk.carrying = false;
                }
                self.flyers.push(Flyer {
                    path: path.clone(),
                    value: *value,
                    t: 0,
                    total: self.scaled(*ticks),
                    copy,
                });
            }
            Beat::Hand(_) | Beat::Tile(..) | Beat::Wait(_) | Beat::Flash(_) | Beat::ClearBubble => {}
        }
        let instant = matches!(
            beat,
            Beat::Face(_)
                | Beat::TakeInbox
                | Beat::Sparkle(..)
                | Beat::PointerAt(_)
                | Beat::Hand(_)
                | Beat::Tile(..)
                | Beat::Flash(_)
                | Beat::ClearBubble
                | Beat::StickyBubble(_)
        );
        if instant {
            self.finish(beat);
        } else {
            self.active = Some(Active { beat, t: 0, total });
        }
    }

    fn finish(&mut self, beat: Beat) {
        match beat {
            Beat::Walk(x, y) => {
                self.clerk.x = x;
                self.clerk.y = y;
                self.clerk.walking = false;
                self.walk_to = None;
            }
            Beat::Pose(p, _) => {
                if matches!(p, Pose::Grab | Pose::Work | Pose::Think | Pose::Shrug) {
                    self.clerk.pose = Pose::Idle;
                }
            }
            Beat::Hand(v) => {
                self.vis.hand = v;
                self.clerk.carrying = v.is_some();
                if v.is_some() {
                    // the value changed in the hand: little pop
                }
            }
            Beat::Tile(slot, v) => {
                if slot < self.vis.floor.len() {
                    self.vis.floor[slot] = Some(v);
                    self.tile_pop[slot] = 10;
                }
            }
            Beat::Flash(slot) => {
                if slot < self.flash.len() {
                    self.flash[slot] = 24;
                }
            }
            Beat::ClearBubble => self.bubble = None,
            Beat::Fly { value, land, .. } => self.apply_land(value, &land),
            Beat::Bubble(..) => {
                self.bubble = None;
            }
            _ => {}
        }
    }

    /// Advance one tick (60 per second).
    pub fn tick(&mut self) {
        self.clock += 1;
        self.clerk.clock = self.clerk.clock.wrapping_add(1);
        let rm = self.reduce_motion;
        if self.in_slide > 0 {
            self.in_slide -= 1;
        }
        for f in &mut self.flash {
            *f = f.saturating_sub(1);
        }
        for p in &mut self.tile_pop {
            *p = p.saturating_sub(1);
        }
        self.sparks.retain_mut(|s| {
            s.ttl = s.ttl.saturating_sub(1);
            s.ttl > 0
        });
        if let Some((_, t)) = &mut self.pointer {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.pointer = None;
            }
        }
        if let Some((_, t)) = &mut self.bubble {
            if *t != u32::MAX {
                *t = t.saturating_sub(1);
                if *t == 0 {
                    self.bubble = None;
                }
            }
        }
        // flyers advance and land
        let mut landed = Vec::new();
        for (i, f) in self.flyers.iter_mut().enumerate() {
            f.t += 1;
            if f.t >= f.total {
                landed.push(i);
            }
        }
        for i in landed.into_iter().rev() {
            self.flyers.remove(i);
        }
        if self.busy() && !rm {
            self.belt_phase = self.belt_phase.wrapping_add(1);
        }
        // the beat in progress
        if let Some(mut a) = self.active.take() {
            a.t += 1;
            let done = match &a.beat {
                Beat::Walk(tx, ty) => {
                    let speed = 1.6 * self.mult();
                    let (dx, dy) = (tx - self.clerk.x, ty - self.clerk.y);
                    if dx.abs() < 0.01 && dy.abs() < 0.01 {
                        true
                    } else {
                        let mut left = speed;
                        // horizontal first, then vertical
                        if dx.abs() > 0.01 {
                            let m = dx.abs().min(left);
                            self.clerk.x += m * dx.signum();
                            left -= m;
                            self.clerk.dir = if dx > 0.0 { Dir::Right } else { Dir::Left };
                        }
                        if left > 0.0 && dy.abs() > 0.01 {
                            let m = dy.abs().min(left);
                            self.clerk.y += m * dy.signum();
                            self.clerk.dir = if dy > 0.0 { Dir::Down } else { Dir::Up };
                        }
                        self.clerk.walking = true;
                        (tx - self.clerk.x).abs() < 0.01 && (ty - self.clerk.y).abs() < 0.01
                    }
                }
                Beat::Pose(..) | Beat::Wait(_) | Beat::Bubble(..) => a.t >= a.total,
                Beat::Fly { .. } => a.t >= a.total,
                _ => true,
            };
            if done {
                if let Beat::Walk(..) = &a.beat {
                    self.clerk.walking = false;
                    if !matches!(self.queue.front(), Some(Beat::Walk(..))) {
                        self.clerk.dir = Dir::Down;
                    }
                }
                self.finish(a.beat);
            } else {
                self.active = Some(a);
            }
        }
        if self.active.is_none() {
            if let Some(b) = self.queue.pop_front() {
                self.begin(b);
            } else {
                self.clerk.walking = false;
            }
        }
    }

    fn apply_land(&mut self, value: i32, land: &Land) {
        match land {
            Land::Hand => {
                self.vis.hand = Some(value);
                self.clerk.carrying = true;
            }
            Land::Out(_) => {
                self.vis.outbox.push(value);
            }
            Land::Tile(slot, v) => {
                if *slot < self.vis.floor.len() {
                    self.vis.floor[*slot] = Some(*v);
                    self.tile_pop[*slot] = 10;
                }
            }
        }
    }

    /// Show a sticky speech bubble (errors) until cleared.
    pub fn set_bubble(&mut self, text: Option<String>) {
        self.bubble = text.map(|t| (t, u32::MAX));
    }

    // ------------------------------------------------------------------ drawing

    /// Draw the whole scene with its top-left at `origin`.
    pub fn draw(&self, g: &mut Gfx<'_>, origin: (i32, i32)) {
        let saved_off = g.off;
        let saved_clip = g.c.set_clip(Rect::new(origin.0 + saved_off.0, origin.1 + saved_off.1, SCENE_W, SCENE_H));
        g.off = (origin.0 + saved_off.0, origin.1 + saved_off.1);
        self.draw_room(g);
        self.draw_belts(g);
        self.draw_floor(g);
        self.draw_clerk_and_crates(g);
        self.draw_fx(g);
        g.off = saved_off;
        g.c.set_clip(saved_clip);
    }

    fn anim(&self, period: u64, frames: usize) -> usize {
        if self.reduce_motion { 0 } else { ((self.clock / period) % frames as u64) as usize }
    }

    fn draw_room(&self, g: &mut Gfx<'_>) {
        // carpet
        for ty in 0..9 {
            for tx in 0..17 {
                let k = if (tx + ty) % 2 == 0 { "tiles/carpet_a" } else { "tiles/carpet_b" };
                g.sprite(k, 0, tx * 16, 48 + ty * 16);
            }
        }
        // wall
        for tx in 0..17 {
            g.sprite("tiles/wall_top", 0, tx * 16, 0);
            g.sprite(if tx % 3 == 1 { "tiles/wall_b" } else { "tiles/wall_a" }, 0, tx * 16, 16);
            g.sprite("tiles/dado", 0, tx * 16, 32);
        }
        // wall decor
        let w = self.anim(80, 2);
        g.sprite("props/poster_a", 0, 66, 9);
        g.sprite("props/window", w, 88, 8);
        g.sprite("props/clock", 0, 127, 5);
        g.sprite("props/window", w, 156, 8);
        g.sprite("props/poster_b", 0, 194, 9);
        g.sprite("props/poster_c", 0, 216, 11);
        g.sprite("props/shelf", 0, 26, 8);
        self.draw_clock_hands(g);
        // furniture standing on the floor against the wall
        g.sprite("props/cabinet_a", 0, 26, 28);
        g.sprite("props/cabinet_b", 0, 44, 28);
        g.sprite("props/bin", 0, 66, 36);
        g.sprite("props/desk", self.anim(30, 2), 119, 24);
        g.sprite("props/mug", self.anim(14, 3), 137, 27);
        g.sprite("props/cooler", self.anim(45, 2), 188, 28);
        g.sprite("props/plant", self.anim(50, 2), 222, 26);
        // ceiling lamp and its light pool
        g.sprite("props/lamp", 0, 123, 0);
        self.light_pool(g, 135, 112, 96, 52);
        // signs above the belts
        for (x, label) in [(2, "IN"), (248, "OUT")] {
            g.sprite("props/sign", 0, x, 26);
            let w = label.len() as i32 * 6;
            g.text(x + (20 - w) / 2 + 1, 28, label, 'K');
        }
    }

    fn draw_clock_hands(&self, g: &mut Gfx<'_>) {
        let (cx, cy) = (127 + 8, 5 + 8);
        let m = (self.steps % 60) as f32 * std::f32::consts::TAU / 60.0;
        let h = (self.steps % 720) as f32 * std::f32::consts::TAU / 720.0;
        let end = |len: f32, a: f32| (cx + (a.sin() * len).round() as i32, cy - (a.cos() * len).round() as i32);
        let (hx, hy) = end(3.0, h);
        let (mx, my) = end(5.0, m);
        g.line(cx, cy, hx, hy, 'K');
        g.line(cx, cy, mx, my, 'R');
        g.px(cx, cy, 'K');
    }

    /// The lamp's light pool: inside the ellipse the carpet speckles disappear (flat, lit carpet) and the edge
    /// is dithered, so the effect stays inside the palette and never looks like noise.
    fn light_pool(&self, g: &mut Gfx<'_>, cx: i32, cy: i32, rx: i32, ry: i32) {
        let (t_dark, t_mid) = (g.col('t'), g.col('T'));
        let (ox, oy) = g.off;
        for y in (cy - ry).max(48)..(cy + ry).min(SCENE_H) {
            for x in (cx - rx).max(0)..(cx + rx).min(SCENE_W) {
                let (dx, dy) = ((x - cx) as f32 / rx as f32, (y - cy) as f32 / ry as f32);
                let d = dx * dx + dy * dy;
                if d >= 1.0 || (d > 0.72 && (x + y) & 1 == 1) {
                    continue;
                }
                let (px, py) = (x + ox, y + oy);
                if px < 0 || py < 0 || px as usize >= g.c.w || py as usize >= g.c.h {
                    continue;
                }
                if g.c.get(px as usize, py as usize) == t_dark {
                    g.c.set(px, py, t_mid);
                }
            }
        }
    }

    fn belt_frame(&self, up: bool) -> usize {
        let p = (self.belt_phase / 2) as usize % 4;
        if up { (4 - p) % 4 } else { p }
    }

    fn draw_belts(&self, g: &mut Gfx<'_>) {
        for (belt, up) in [(self.geom.in_belt, false), (self.geom.out_belt, true)] {
            g.sprite("tiles/belt_cap_top", self.anim(10, 2), belt.x, belt.y - 8);
            for i in 0..BELT_SLOTS as i32 {
                g.sprite("tiles/belt_v", self.belt_frame(up), belt.x, belt.y + i * 16);
            }
            g.sprite("tiles/belt_cap_bot", self.anim(10, 2), belt.x, belt.bottom());
            g.sprite("tiles/belt_feet", 0, belt.x, belt.bottom() + 8);
        }
    }

    fn label_for(&self, slot: usize) -> String {
        let name = self.names.get(slot).map_or("", String::as_str);
        if name.is_empty() {
            slot.to_string()
        } else {
            let mut s = format!("{slot} {name}");
            s.truncate(5);
            s.trim_end().to_string()
        }
    }

    fn draw_floor(&self, g: &mut Gfx<'_>) {
        for slot in 0..self.geom.n {
            let r = self.geom.spot(slot);
            let hot = self.pickable && (self.clock / 20) % 2 == 0 && !self.reduce_motion;
            let flashing = self.flash.get(slot).copied().unwrap_or(0) > 0 && (self.clock / 3) % 2 == 0;
            let focus = self.operand_focus == Some(slot);
            g.sprite("tiles/spot", usize::from(hot || flashing || focus), r.x, r.y);
            if focus {
                g.frame(r.grow(1), 'R');
                g.frame(r.grow(2), 'K');
            }
            // number label under the spot
            let label = self.label_for(slot);
            g.tiny_c(r.x + SPOT_W / 2, r.bottom() + 2, &label, 'W');
        }
        // pointer arrow
        if let Some((slot, _)) = self.pointer {
            let r = self.geom.spot(slot);
            let bob = if self.reduce_motion { 0 } else { ((self.clock / 6) % 2) as i32 };
            g.sprite("ui/pointer", 0, r.x + SPOT_W - 6, r.y - 8 + bob);
        }
    }

    fn draw_crate(&self, g: &mut Gfx<'_>, x: i32, y: i32, v: i32) {
        draw_crate_sprite(g, x, y, v, self.char_mode);
    }

    fn draw_clerk_and_crates(&self, g: &mut Gfx<'_>) {
        // crates on the inbox belt, with a slide when one was taken
        let slide = if self.in_slide > 0 { (self.in_slide as i32 * CRATE_W) / 10 } else { 0 };
        let remaining = self.vis.inbox.len().saturating_sub(self.vis.taken);
        for i in 0..remaining.min(BELT_SLOTS + 1) {
            let v = self.vis.inbox[self.vis.taken + i];
            let r = self.geom.in_slot(i);
            let y = r.y - slide;
            if y >= self.geom.in_belt.y - CRATE_W + 2 {
                self.draw_crate(g, r.x, y, v);
            }
        }
        if remaining > BELT_SLOTS {
            let r = self.geom.in_belt;
            let txt = format!("+{}", remaining - BELT_SLOTS);
            let w = g.text_w(&txt, 1) + 4;
            g.chip(Rect::new(r.x + (16 - w) / 2, r.y - 22, w, 11), &txt, 'W', 'K');
        }
        // outbox: ghost crates, delivered crates and stamps
        let n_exp = self.vis.expected.len();
        let delivered = self.vis.outbox.len();
        let w0 = delivered.saturating_sub(BELT_SLOTS - 1);
        for j in 0..BELT_SLOTS {
            let idx = w0 + j;
            let r = self.geom.out_slot(j);
            if idx < n_exp && idx >= delivered {
                g.sprite("tiles/crate_ghost", 0, r.x, r.y);
                let want = if self.char_mode {
                    printable(self.vis.expected[idx]).map_or_else(|| crate_label(self.vis.expected[idx]), |c| c.to_string())
                } else {
                    crate_label(self.vis.expected[idx])
                };
                g.tiny_c(r.x + 8, r.y + 7, &want, 'A');
            }
            if idx < delivered {
                let v = self.vis.outbox[idx];
                self.draw_crate(g, r.x, r.y, v);
                if idx < n_exp {
                    let ok = self.vis.expected[idx] == v;
                    let key = if ok { "icons/stamp_ok" } else { "icons/stamp_bad" };
                    g.sprite(key, 0, r.x - 10, r.y + 3);
                    if !ok {
                        let want = crate_label(self.vis.expected[idx]);
                        g.tiny_c(r.x - 6, r.y + 13, &want, 'R');
                    }
                }
            }
        }
        if w0 > 0 {
            g.tiny(self.geom.out_belt.x + 4, self.geom.out_belt.y - 14, "...", 'W');
        }
        // the clerk (behind the tile crates) ...
        self.draw_clerk(g);
        // crates on tiles
        for slot in 0..self.geom.n {
            if let Some(v) = self.vis.floor.get(slot).copied().flatten() {
                let r = self.geom.tile_crate(slot);
                let pop = if self.tile_pop[slot] > 0 && !self.reduce_motion { -((self.tile_pop[slot] as i32) / 3) } else { 0 };
                self.draw_crate(g, r.x, r.y + pop, v);
            }
        }
        // crate carried overhead
        if let Some(v) = self.vis.hand {
            let (x, y) = self.hand_pos();
            self.draw_crate(g, x, y, v);
        }
        for f in &self.flyers {
            let (x, y) = flyer_pos(f);
            self.draw_crate(g, x, y, f.value);
        }
    }

    /// Top-left of the crate held over the clerk's head.
    pub fn hand_pos(&self) -> (i32, i32) {
        let hop = self.clerk.hop();
        (self.clerk.x.round() as i32 - CRATE_W / 2, self.clerk.y.round() as i32 - 20 - CRATE_H + 2 + hop)
    }

    fn draw_clerk(&self, g: &mut Gfx<'_>) {
        let x = self.clerk.x.round() as i32;
        let y = self.clerk.y.round() as i32;
        // shadow
        g.rect(Rect::new(x - 6, y - 1, 12, 3), 't');
        g.rect(Rect::new(x - 4, y - 2, 8, 1), 't');
        let (frame, flip) = self.clerk.frame(!self.reduce_motion);
        let hop = self.clerk.hop();
        g.c.blit(
            self.art.sprite(),
            frame,
            x - 8 + g.off.0,
            y - 20 + hop + g.off.1,
            flip,
            g.pal,
        );
        // dust puffs while walking
        if self.clerk.walking && !self.reduce_motion && (self.clerk.clock / 6) % 2 == 0 {
            let dx = match self.clerk.dir {
                Dir::Left => 6,
                Dir::Right => -10,
                _ => -4,
            };
            g.sprite("fx/dust", ((self.clerk.clock / 6) % 3) as usize, x + dx, y - 5);
        }
    }

    fn draw_fx(&self, g: &mut Gfx<'_>) {
        for s in &self.sparks {
            let frame = ((40 - s.ttl) / 5) as usize % 4;
            let (x, y) = (s.at.0 as i32, s.at.1 as i32 - ((40 - s.ttl) / 6) as i32);
            g.sprite("fx/sparkle", frame, x - 4, y - 4);
            g.round_rect(Rect::new(x - 5, y - 17, 11, 11), 'W', 'K');
            g.text(x - 2, y - 15, &s.sym.to_string(), if s.sym == '-' || s.sym == '%' { 'R' } else { 'G' });
        }
        if let Some((text, _)) = &self.bubble {
            self.draw_bubble(g, text);
        }
    }

    fn draw_bubble(&self, g: &mut Gfx<'_>, text: &str) {
        let lines = algoclerk_media::Font::wrap(text, 24);
        let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32 * 6 + 10;
        let h = lines.len() as i32 * 10 + 8;
        let hx = self.clerk.x.round() as i32;
        let top = self.clerk.y.round() as i32 - 20 - if self.vis.hand.is_some() { CRATE_H } else { 0 } - h - 8;
        let x = (hx - w / 2).clamp(2, SCENE_W - w - 2);
        let y = top.max(2);
        g.bubble(Rect::new(x, y, w, h), hx - 3, 'W', 'K');
        for (i, l) in lines.iter().enumerate() {
            g.text(x + 5, y + 4 + i as i32 * 10, l, 'K');
        }
    }
}

/// Draw a crate with its label (used by the office, the memo and the victory screen).
pub fn draw_crate_sprite(g: &mut Gfx<'_>, x: i32, y: i32, v: i32, char_mode: bool) {
    g.sprite(crate_variant(v), 0, x, y);
    let plate = Rect::new(x + 2, y + 4, 12, 9);
    if char_mode {
        if let Some(c) = printable(v) {
            g.text(plate.x + 4, plate.y + 1, &c.to_string(), 'K');
            return;
        }
    }
    let s = crate_label(v);
    let color = if v < 0 { 'R' } else { 'K' };
    g.tiny_c(plate.x + plate.w / 2, plate.y + 2, &s, color);
}

fn flyer_pos(f: &Flyer) -> (i32, i32) {
    let n = f.path.len();
    if n == 0 {
        return (0, 0);
    }
    if n == 1 {
        return (f.path[0].0.round() as i32, f.path[0].1.round() as i32);
    }
    let t = (f.t as f32 / f.total.max(1) as f32).clamp(0.0, 1.0);
    // equal time per segment
    let seg = t * (n - 1) as f32;
    let i = (seg.floor() as usize).min(n - 2);
    let u = crate::gfx::ease_out(seg - i as f32);
    let (a, b) = (f.path[i], f.path[i + 1]);
    // a little arc on every segment
    let arc = -(u * (1.0 - u) * 4.0) * 6.0;
    ((a.0 + (b.0 - a.0) * u).round() as i32, (a.1 + (b.1 - a.1) * u + arc).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_lays_out_every_supported_floor_inside_the_scene() {
        for n in 1..=MAX_TILES {
            let g = Geometry::new(n);
            assert!(g.cols * g.rows >= n);
            let scene = Rect::new(0, 0, SCENE_W, SCENE_H);
            for s in 0..n {
                let r = g.spot(s);
                assert!(r.inside(&scene), "tile {s} of {n} is outside the scene: {r:?}");
                assert!(r.x >= g.in_belt.right() + 6 && r.right() <= g.out_belt.x - 6, "tile {s} of {n} touches a belt");
                let (sx, sy) = g.stand(s);
                assert!(sx > 0.0 && sy < SCENE_H as f32);
            }
            // tiles never overlap
            for a in 0..n {
                for b in a + 1..n {
                    assert!(g.spot(a).intersect(&g.spot(b)).w <= 0 || g.spot(a).intersect(&g.spot(b)).h <= 0);
                }
            }
        }
    }

    #[test]
    fn belt_slots_are_inside_the_belts() {
        let g = Geometry::new(8);
        for i in 0..BELT_SLOTS {
            assert!(g.in_slot(i).inside(&g.in_belt));
            assert!(g.out_slot(i).inside(&g.out_belt));
        }
    }

    #[test]
    fn crate_labels_fit_the_plate() {
        for v in [-100_000, -100, -99, -1, 0, 7, 99, 100, 999, 1000, 12_345, 99_999, 100_000, 999_999] {
            assert!(crate_label(v).chars().count() <= 3, "{v}");
        }
        assert_eq!(crate_label(42), "42");
        assert_eq!(crate_label(12_345), "12K");
        assert_eq!(printable(65), Some('A'));
        assert_eq!(printable(10), None);
    }
}
