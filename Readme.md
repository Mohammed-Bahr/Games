

## 🎮 What The Game Actually Does (Big Picture)

Imagine a circular arena. 4 colored balls fly around inside it:
- When a ball hits the **border**, it speeds up and a **line** (leash) attaches from that border point to the ball
- When two balls **hit each other**, they both slow down
- If a ball **crosses another ball's line**, that line disappears. If a ball loses all its lines, it dies
- **Last ball alive wins**

---

## 📦 The Building Blocks (Data Structures)

### `Vec2 = (f64, f64)`
This is just a **tuple representing x and y coordinates**. Think of it as a point or a direction in 2D space.

### `struct Line`
```rust
struct Line {
    anchor: Vec2,  // A fixed point on the arena border
}
```
When a ball bounces off the border, we create a `Line`. The `anchor` stays stuck to the border forever, while the other end follows the ball as it moves.

### `struct Ball`
```rust
struct Ball {
    pos: Vec2,           // Where am I?
    vel: Vec2,           // Where am I going? (velocity)
    color: (f64,f64,f64), // RGB color
    lines: Vec<Line>,    // My collection of lines attached to the border
    alive: bool,         // Am I still in the game?
    had_line: bool,      // Have I ever bounced off the border?
}
```
**Critical line: `had_line`** — This is a clever protection. A ball can't die until it has earned at least one line and then lost it. This prevents a ball from being eliminated before it even had a chance to play.

### `struct GameState`
```rust
struct GameState {
    balls: Vec<Ball>,
    winner: Option<usize>,  // None = game ongoing, Some(2) = ball #2 won
    frame: u64,             // Frame counter (like a stopwatch tick)
}
```
`Option<usize>` means "maybe there's a winner, maybe not." `None` = no winner yet.

---

## 🔧 The Math Helpers (Don't Be Scared!)

These are just **high school geometry** written in Rust:

```rust
fn sub(a: Vec2, b: Vec2) -> Vec2 { (a.0 - b.0, a.1 - b.1) }
fn add(a: Vec2, b: Vec2) -> Vec2 { (a.0 + b.0, a.1 + b.1) }
fn dot(a: Vec2, b: Vec2) -> f64  { a.0 * b.0 + a.1 * b.1 }
fn len(a: Vec2) -> f64           { dot(a,a).sqrt() }
fn normalize(a: Vec2) -> Vec2    { ... } // Make length = 1, keep direction
```

**Why this matters:** Games need to know "how far apart are these balls?" and "which direction is this ball going?" These functions answer those questions.

### `point_segment_distance` — The Most Important Math Function
```rust
fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64
```
This answers: *"How close is point `p` to the line segment from `a` to `b`?"*

This is **critical** because it's how the game detects if a ball crossed another ball's line! If the distance from the ball's center to the line segment is smaller than the ball's radius, we have a hit.

---

## 🏃 The Game Loop: `step()`

This runs 60 times per second. Every frame, it does 6 things in order:

### 1. Move all balls
```rust
ball.pos = add(ball.pos, ball.vel);
```
Simple: New position = Old position + Velocity. If velocity is `(3, 4)`, the ball moves 3 pixels right and 4 pixels down.

### 2. Border collisions (Bounce + Speed Up + New Line)
```rust
let offset = sub(ball.pos, ARENA_CENTER);
let dist = len(offset);
let limit = ARENA_RADIUS - ball.radius();
if dist >= limit {
    // REFLECT: bounce off the circular wall
    let normal = normalize(offset);
    let vn = dot(ball.vel, normal);
    ball.vel = sub(ball.vel, scale(normal, 2.0 * vn));
    
    // SPEED UP: multiply velocity by 1.10
    ball.vel = scale(ball.vel, SPEED_UP_ON_BORDER);
    ball.vel = clamp_speed(ball.vel, MIN_SPEED, MAX_SPEED);
    
    // PUSH BACK inside arena (prevents getting stuck)
    ball.pos = add(ARENA_CENTER, scale(normal, limit));
    
    // CREATE LINE: anchor at exact border contact point
    let contact = add(ARENA_CENTER, scale(normal, ARENA_RADIUS));
    ball.lines.push(Line { anchor: contact });
    ball.had_line = true;
}
```

**Critical concept — Reflection:** `ball.vel = sub(ball.vel, scale(normal, 2.0 * vn))` is the **mirror formula**. Imagine throwing a ball at a wall — it bounces off at the same angle it came in. That's what this does mathematically using the "normal" (a vector perpendicular to the surface).

### 3. Ball vs Ball collisions
```rust
let normal = normalize(sub(pi, pj));  // Direction from ball j to ball i
// Swap the velocity components along the collision direction
let v1n = dot(self.balls[i].vel, normal);
let v2n = dot(self.balls[j].vel, normal);
// ... swap them ...
self.balls[i].vel = clamp_speed(scale(self.balls[i].vel, SPEED_DOWN_ON_HIT), ...);
```

**What happens:** When balls hit, they exchange energy along the collision line (like billiard balls), then both get **slower** (multiplied by 0.90). They also get pushed apart so they don't overlap.

### 4. Line Crossing Detection (The Core Mechanic!)
```rust
for crosser_idx in 0..n {
    for owner_idx in 0..n {
        for (line_idx, line) in self.balls[owner_idx].lines.iter().enumerate() {
            let d = point_segment_distance(crosser_pos, line.anchor, owner_pos);
            if d <= crosser_r + LINE_HIT_TOLERANCE {
                to_remove.push((owner_idx, line_idx));
            }
        }
    }
}
```

**How it works:** For every alive ball (`crosser`), check if it got close to every line segment owned by every other alive ball. The line goes from `line.anchor` (fixed on border) to `owner_pos` (the ball's current position).

**Important bug-prevention:**
```rust
to_remove.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by line index, HIGHEST first
```
Why highest first? Because when you remove item #3 from a list, items #0, #1, #2 stay valid. If you removed #0 first, everything shifts and your indices become wrong!

### 5. Elimination
```rust
if ball.alive && ball.had_line && ball.lines.is_empty() {
    ball.alive = false;
}
```
A ball dies **only if**:
- It's still alive
- It has bounced at least once (`had_line == true`)  
- It currently has zero lines left

### 6. Check Winner
```rust
if alive.len() == 1 {
    self.winner = Some(alive[0]);
}
```

---

## 🎨 Rendering (Drawing on Screen)

The `draw()` function uses **Cairo** (a 2D graphics library) to paint everything:

1. **Black background** (`cr.set_source_rgb(0.02, 0.02, 0.02)`)
2. **Gray arena circle** (the border)
3. **Lines** (drawn first so balls appear on top)
4. **Balls** with:
   - A soft glow (larger, semi-transparent circle)
   - Solid body
   - **White number** showing how many lines they have
5. **Status text** at top and bottom

---

## 🔌 The GTK App Wiring (How It All Connects)

```rust
fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.example.ballbattle")
        .build();
```

This creates a GTK application (a window with an event loop).

### The Tricky Part: `Rc<RefCell<GameState>>`
```rust
let state = Rc::new(RefCell::new(GameState::new_random()));
```

**Why this is critical:** GTK uses callbacks (functions that run later when events happen). Rust's borrow checker normally prevents multiple mutable references to data. But we need:
- The **draw callback** to *read* the state
- The **timer callback** to *modify* the state

`Rc<RefCell<T>>` is Rust's way of saying:
- `Rc` = "Multiple owners can share this data" (Reference Counted)
- `RefCell` = "I promise I'll only mutate it safely, check at runtime not compile time"

This is a common pattern in Rust GUI programming.

### The Timer (Game Loop)
```rust
glib::timeout_add_local(Duration::from_millis(TICK_MS), move || {
    state.borrow_mut().step();      // Mutably borrow and update game
    drawing_area.queue_draw();     // "Hey GTK, redraw the screen!"
    glib::ControlFlow::Continue    // Keep running forever
});
```
`TICK_MS = 16` means ~60 frames per second (1000ms / 16 ≈ 60 FPS).

---

## 🧠 Summary of Critical Lines for a Beginner

| Line/Concept | Why It Matters |
|---|---|
| `ball.vel = sub(ball.vel, scale(normal, 2.0 * vn))` | The physics of bouncing off walls |
| `ball.lines.push(Line { anchor: contact })` | The core game mechanic — creating a leash |
| `point_segment_distance(...)` | Detecting if a ball crossed a line |
| `to_remove.sort_by(\|a, b\| b.1.cmp(&a.1))` | Removing list items safely without breaking indices |
| `had_line` | Prevents unfair early elimination |
| `Rc<RefCell<...>>` | Rust's solution for shared mutable state in callbacks |
| `clamp_speed(...)` | Keeps balls from going too fast or stopping completely |

---

## 🐛 One Thing to Watch Out For

There's a subtle issue in the line removal: if two different balls cross the **same line** in the same frame, it might try to remove it twice. The code checks `if line_idx < self.balls[owner_idx].lines.len()` which helps, but duplicate entries in `to_remove` for the same `(owner, line_idx)` could still cause a panic in edge cases. A `HashSet` would be more robust, but for a simple game this works fine.

---
