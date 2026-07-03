// Ball Battle — gtk + Cairo, Rust
//
// Rules:
//  - Balls bounce inside a circular arena.
//  - Hitting the border speeds the ball up AND anchors a new "line"
//    (a leash) between that border point and the ball, which keeps
//    following the ball as it moves.
//  - Hitting another ball slows both balls down.
//  - If ball A's body crosses ball B's line segment, that specific
//    line vanishes and B's line count (shown as the number on the
//    ball) drops by one. The crossing ball A is unaffected.
//  - A ball with zero lines vanishes (eliminated).
//  - Last ball with lines > 0 wins.

use gtk::cairo::Context;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, DrawingArea};
use rand::Rng;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Duration;

// ---------- Tunable constants ----------

const ARENA_RADIUS: f64 = 380.0;
const WIN_W: f64 = 900.0;
const WIN_H: f64 = 900.0;
const ARENA_CENTER: (f64, f64) = (WIN_W / 2.0, WIN_H / 2.0);

const BALL_RADIUS: f64 = 22.0;

const INITIAL_SPEED: f64 = 4.5;
const SPEED_UP_ON_BORDER: f64 = 1.4; // multiplier when bouncing off the wall
const SPEED_DOWN_ON_HIT: f64 = 1.0; // multiplier when hitting another ball
const MAX_SPEED: f64 = 16.0;
const MIN_SPEED: f64 = 1.5;

const LINE_HIT_TOLERANCE: f64 = 3.0; // extra px added to ball radius for line crossing checks

const TICK_MS: u64 = 16; // ~60 FPS

// ---------- Small vector helpers ----------

type Vec2 = (f64, f64);

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 - b.0, a.1 - b.1)
}
fn add(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 + b.0, a.1 + b.1)
}
fn scale(a: Vec2, s: f64) -> Vec2 {
    (a.0 * s, a.1 * s)
}
fn dot(a: Vec2, b: Vec2) -> f64 {
    a.0 * b.0 + a.1 * b.1
}
fn len(a: Vec2) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: Vec2) -> Vec2 {
    let l = len(a);
    if l < 1e-9 {
        (1.0, 0.0)
    } else {
        (a.0 / l, a.1 / l)
    }
}
fn clamp_speed(v: Vec2, min_s: f64, max_s: f64) -> Vec2 {
    let l = len(v);
    if l < 1e-9 {
        return (min_s, 0.0);
    }
    let clamped = l.clamp(min_s, max_s);
    scale(v, clamped / l)
}

/// Shortest distance from point `p` to the segment `a -> b`.
fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = sub(b, a);
    let ab_len2 = dot(ab, ab);
    if ab_len2 < 1e-9 {
        return len(sub(p, a));
    }
    let t = (dot(sub(p, a), ab) / ab_len2).clamp(0.0, 1.0);
    let proj = add(a, scale(ab, t));
    len(sub(p, proj))
}

// ---------- Game data ----------

struct Line {
    /// Fixed point on the border where the owning ball bounced.
    anchor: Vec2,
}

struct Ball {
    pos: Vec2,
    vel: Vec2,
    color: (f64, f64, f64),
    lines: Vec<Line>,
    alive: bool,
    /// True once this ball has bounced off the border at least once.
    /// Used so a ball can't be eliminated before it ever earned a line.
    had_line: bool,
}

impl Ball {
    fn radius(&self) -> f64 {
        BALL_RADIUS
    }
}

struct GameState {
    balls: Vec<Ball>,
    winner: Option<usize>,
    frame: u64,
}

impl GameState {
    fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let colors = [
            (0.95, 0.15, 0.15), // red
            (0.15, 0.45, 0.95), // blue
            (0.98, 0.70, 0.10), // orange
            (0.15, 0.90, 0.75), // cyan
        ];

        let mut balls = Vec::new();
        for &color in colors.iter() {
            // spawn somewhere well inside the arena
            let r = rng.gen_range(0.0..(ARENA_RADIUS * 0.5));
            let theta = rng.gen_range(0.0..(2.0 * PI));
            let pos = (
                ARENA_CENTER.0 + r * theta.cos(),
                ARENA_CENTER.1 + r * theta.sin(),
            );
            let angle = rng.gen_range(0.0..(2.0 * PI));
            let vel = (INITIAL_SPEED * angle.cos(), INITIAL_SPEED * angle.sin());

            balls.push(Ball {
                pos,
                vel,
                color,
                lines: Vec::new(),
                alive: true,
                had_line: false,
            });
        }

        GameState {
            balls,
            winner: None,
            frame: 0,
        }
    }

    fn alive_count(&self) -> usize {
        self.balls.iter().filter(|b| b.alive).count()
    }

    fn step(&mut self) {
        if self.winner.is_some() {
            return;
        }
        self.frame += 1;

        // 1. Move alive balls.
        for ball in self.balls.iter_mut().filter(|b| b.alive) {
            ball.pos = add(ball.pos, ball.vel);
        }

        // 2. Border collisions (speed up + new line anchor).
        for ball in self.balls.iter_mut().filter(|b| b.alive) {
            let offset = sub(ball.pos, ARENA_CENTER);
            let dist = len(offset);
            let limit = ARENA_RADIUS - ball.radius();
            if dist >= limit {
                let normal = normalize(offset);
                // reflect velocity about the normal
                let vn = dot(ball.vel, normal);
                ball.vel = sub(ball.vel, scale(normal, 2.0 * vn));
                ball.vel = scale(ball.vel, SPEED_UP_ON_BORDER);
                ball.vel = clamp_speed(ball.vel, MIN_SPEED, MAX_SPEED);

                // push back inside the arena
                ball.pos = add(ARENA_CENTER, scale(normal, limit));

                // anchor a new line at the actual border contact point
                let contact = add(ARENA_CENTER, scale(normal, ARENA_RADIUS));
                ball.lines.push(Line { anchor: contact });
                ball.had_line = true;
            }
        }

        // 3. Ball vs ball collisions (slow down + separate).
        let n = self.balls.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if !self.balls[i].alive || !self.balls[j].alive {
                    continue;
                }
                let (pi, pj) = (self.balls[i].pos, self.balls[j].pos);
                let d = len(sub(pi, pj));
                let min_d = self.balls[i].radius() + self.balls[j].radius();
                if d < min_d && d > 1e-6 {
                    let normal = normalize(sub(pi, pj));

                    // swap normal-component velocities (equal-mass elastic-ish)
                    let v1n = dot(self.balls[i].vel, normal);
                    let v2n = dot(self.balls[j].vel, normal);

                    self.balls[i].vel = add(sub(self.balls[i].vel, scale(normal, v1n)), scale(normal, v2n));
                    self.balls[j].vel = add(sub(self.balls[j].vel, scale(normal, v2n)), scale(normal, v1n));

                    self.balls[i].vel = clamp_speed(scale(self.balls[i].vel, SPEED_DOWN_ON_HIT), MIN_SPEED, MAX_SPEED);
                    self.balls[j].vel = clamp_speed(scale(self.balls[j].vel, SPEED_DOWN_ON_HIT), MIN_SPEED, MAX_SPEED);

                    // separate overlapping balls
                    let overlap = min_d - d;
                    let push = scale(normal, overlap / 2.0 + 0.1);
                    self.balls[i].pos = add(self.balls[i].pos, push);
                    self.balls[j].pos = sub(self.balls[j].pos, push);
                }
            }
        }

        // 4. Line-crossing checks: does any alive ball's body cross
        //    another (different) alive ball's line?
        let mut to_remove: Vec<(usize, usize)> = Vec::new(); // (owner_idx, line_idx)
        for crosser_idx in 0..n {
            if !self.balls[crosser_idx].alive {
                continue;
            }
            let crosser_pos = self.balls[crosser_idx].pos;
            let crosser_r = self.balls[crosser_idx].radius();

            for owner_idx in 0..n {
                if owner_idx == crosser_idx || !self.balls[owner_idx].alive {
                    continue;
                }
                let owner_pos = self.balls[owner_idx].pos;
                for (line_idx, line) in self.balls[owner_idx].lines.iter().enumerate() {
                    let d = point_segment_distance(crosser_pos, line.anchor, owner_pos);
                    if d <= crosser_r + LINE_HIT_TOLERANCE {
                        to_remove.push((owner_idx, line_idx));
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            // remove highest line_idx first per owner so indices stay valid
            to_remove.sort_by(|a, b| b.1.cmp(&a.1));
            for (owner_idx, line_idx) in to_remove {
                if line_idx < self.balls[owner_idx].lines.len() {
                    self.balls[owner_idx].lines.remove(line_idx);
                }
            }
        }

        // 5. Eliminate balls with zero lines — but only once they've
        //    actually earned and lost at least one, so nobody dies
        //    before their very first bounce off the border.
        for ball in self.balls.iter_mut() {
            if ball.alive && ball.had_line && ball.lines.is_empty() {
                ball.alive = false;
            }
        }

        // 6. Check win condition.
        if self.winner.is_none() {
            let alive: Vec<usize> = self
                .balls
                .iter()
                .enumerate()
                .filter(|(_, b)| b.alive)
                .map(|(i, _)| i)
                .collect();
            if alive.len() == 1 {
                self.winner = Some(alive[0]);
            }
        }
    }
}

// ---------- Rendering ----------

fn draw(cr: &Context, state: &GameState) {
    // background
    cr.set_source_rgb(0.02, 0.02, 0.02);
    cr.paint().ok();

    // arena border
    cr.set_source_rgba(0.7, 0.7, 0.7, 0.6);
    cr.set_line_width(2.0);
    cr.arc(ARENA_CENTER.0, ARENA_CENTER.1, ARENA_RADIUS, 0.0, 2.0 * PI);
    cr.stroke().ok();

    // lines (drawn first so balls render on top)
    for ball in state.balls.iter().filter(|b| b.alive) {
        let (r, g, b) = ball.color;
        for line in &ball.lines {
            cr.set_source_rgba(r, g, b, 0.55);
            cr.set_line_width(1.6);
            cr.move_to(line.anchor.0, line.anchor.1);
            cr.line_to(ball.pos.0, ball.pos.1);
            cr.stroke().ok();
        }
    }

    // balls + line-count labels
    for ball in state.balls.iter().filter(|b| b.alive) {
        let (r, g, b_) = ball.color;

        // soft glow
        cr.set_source_rgba(r, g, b_, 0.25);
        cr.arc(ball.pos.0, ball.pos.1, ball.radius() * 1.8, 0.0, 2.0 * PI);
        cr.fill().ok();

        // solid body
        cr.set_source_rgb(r, g, b_);
        cr.arc(ball.pos.0, ball.pos.1, ball.radius(), 0.0, 2.0 * PI);
        cr.fill().ok();

        // line-count label
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Bold,
        );
        cr.set_font_size(16.0);
        let label = format!("{}", ball.lines.len());
        let extents = cr.text_extents(&label).unwrap();
        cr.move_to(
            ball.pos.0 - extents.width() / 2.0 - extents.x_bearing(),
            ball.pos.1 - extents.height() / 2.0 - extents.y_bearing(),
        );
        cr.show_text(&label).ok();
    }

    // status text
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.select_font_face(
        "Serif",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );

    cr.set_font_size(34.0);
    let title = "Normal battle";
    let te = cr.text_extents(title).unwrap();
    cr.move_to(WIN_W / 2.0 - te.width() / 2.0, 70.0);
    cr.show_text(title).ok();

    cr.set_font_size(26.0);
    let status = match state.winner {
        Some(idx) => format!("Ball {} wins!", idx + 1),
        None => format!("{} players left...", state.alive_count()),
    };
    let se = cr.text_extents(&status).unwrap();
    cr.move_to(WIN_W / 2.0 - se.width() / 2.0, WIN_H - 60.0);
    cr.show_text(&status).ok();
}

// ---------- App wiring ----------

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.example.ballbattle")
        .build();

    app.connect_activate(|app| {
        let state = Rc::new(RefCell::new(GameState::new_random()));

        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(WIN_W as i32);
        drawing_area.set_content_height(WIN_H as i32);

        {
            let state = state.clone();
            drawing_area.set_draw_func(move |_area, cr, _w, _h| {
                draw(cr, &state.borrow());
            });
        }

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Ball Battle")
            .child(&drawing_area)
            .default_width(WIN_W as i32)
            .default_height(WIN_H as i32)
            .build();
        window.present();

        // simulation tick
        {
            let state = state.clone();
            let drawing_area = drawing_area.clone();
            glib::timeout_add_local(Duration::from_millis(TICK_MS), move || {
                state.borrow_mut().step();
                drawing_area.queue_draw();
                glib::ControlFlow::Continue
            });
        }
    });

    app.run()
}
