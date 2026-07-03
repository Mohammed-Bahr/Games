use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, DrawingArea};
use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;
use rand::Rng;

// Define a Ball
#[derive(Clone)]
struct Ball {
    x: f64,
    y: f64,
    vx: f64, // velocity x
    vy: f64, // velocity y
    radius: f64,
    color: (f64, f64, f64),
    alive: bool,
}

struct GameState {
    balls: Vec<Ball>,
    width: f64,
    height: f64,
}

impl GameState {
    fn new(width: f64, height: f64) -> Self {
        let mut rng = rand::thread_rng();
        let colors = [
            (1.0, 0.0, 0.0), // Red
            (0.0, 1.0, 0.0), // Green
            (0.0, 0.0, 1.0), // Blue
            (1.0, 1.0, 0.0), // Yellow
        ];
        
        let mut balls = Vec::new();
        for i in 0..4 {
            balls.push(Ball {
                x: rng.gen_range(50.0..width - 50.0),
                y: rng.gen_range(50.0..height - 50.0),
                vx: rng.gen_range(-3.0..3.0),
                vy: rng.gen_range(-3.0..3.0),
                radius: 15.0,
                color: colors[i],
                alive: true,
            });
        }
        
        GameState { balls, width, height }
    }

    fn update(&mut self) {
        for ball in &mut self.balls {
            if !ball.alive { continue; }

            // Move ball
            ball.x += ball.vx;
            ball.y += ball.vy;

            // Border collision -> Get FASTER
            if ball.x - ball.radius < 0.0 {
                ball.x = ball.radius;
                ball.vx = -ball.vx * 1.1; // Increase speed by 10%
            } else if ball.x + ball.radius > self.width {
                ball.x = self.width - ball.radius;
                ball.vx = -ball.vx * 1.1;
            }

            if ball.y - ball.radius < 0.0 {
                ball.y = ball.radius;
                ball.vy = -ball.vy * 1.1;
            } else if ball.y + ball.radius > self.height {
                ball.y = self.height - ball.radius;
                ball.vy = -ball.vy * 1.1;
            }
            
            // Cap max speed so they don't glitch through walls
            let max_speed = 15.0;
            if ball.vx.abs() > max_speed { ball.vx = ball.vx.signum() * max_speed; }
            if ball.vy.abs() > max_speed { ball.vy = ball.vy.signum() * max_speed; }
        }

        // TODO: Ball-to-Ball collision -> Get SLOWER
        // TODO: Line drawing and line collision logic
    }
}

fn main() {
    let app = Application::builder().application_id("com.example.ballarena").build();
    
    app.connect_activate(|app| {
        let width = 800.0;
        let height = 600.0;
        
        let state = Rc::new(RefCell::new(GameState::new(width, height)));
        
        let drawing_area = DrawingArea::builder()
            .content_width(width as i32)
            .content_height(height as i32)
            .build();

        // Draw function
        {
            let state = state.clone();
            drawing_area.set_draw_func(move |_, cr, _, _| {
                let state = state.borrow();
                
                // Draw black background
                cr.set_source_rgb(0.0, 0.0, 0.0);
                cr.paint().expect("Failed to paint background");
                
                // Draw balls
                for ball in &state.balls {
                    if !ball.alive { continue; }
                    cr.set_source_rgb(ball.color.0, ball.color.1, ball.color.2);
                    cr.arc(ball.x, ball.y, ball.radius, 0.0, std::f64::consts::PI * 2.0);
                    cr.fill().expect("Failed to draw ball");
                }
                
                // TODO: Draw lines
            });
        }

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Ball Arena Battle")
            .child(&drawing_area)
            .build();

        // Game Loop (runs ~60 FPS)
        glib::source::timeout_add_local(std::time::Duration::from_millis(16), clone!(@strong window, @strong state => move || {
            {
                let mut state = state.borrow_mut();
                state.update();
            }
            drawing_area.queue_draw(); // Request redraw
            
            glib::ControlFlow::Continue
        }));

        window.present();
    });

    app.run();
}
