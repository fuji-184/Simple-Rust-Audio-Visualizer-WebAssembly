use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use std::f64::consts::PI;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct Bg {
    ctx: CanvasRenderingContext2d,
    width: u32,
    height: u32,
    center_x: f64,
    center_y: f64,
    previous_values: Vec<f64>,
    hue: f64,
    brightness: f64,
    saturation: f64,
    particles: Vec<Particle>,
}

#[wasm_bindgen]
impl Bg {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Bg, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;
        
        let width = canvas.width();
        let height = canvas.height();
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;

        let particles = (0..100).map(|_| Particle::new(width, height)).collect();

        Ok(Bg {
            ctx,
            width,
            height,
            center_x,
            center_y,
            previous_values: vec![0.0; 64],
            hue: 0.0,
            brightness: 50.0,
            saturation: 100.0,
            particles,
        })
    }

    #[wasm_bindgen]
    pub fn draw(&mut self, audio_data: &[u8]) {
        let ctx = &self.ctx;

        let background_color = format!(
            "hsl({}, {}%, {}%)",
            self.hue as i32,
            self.saturation as i32,
            self.brightness as i32
        );
        ctx.set_fill_style(&JsValue::from_str(&background_color));
        ctx.fill_rect(0.0, 0.0, self.width as f64, self.height as f64);

        ctx.save();
        ctx.translate(self.center_x, self.center_y).unwrap();

        {
            let particles = &mut self.particles;
            let hue = self.hue;
            Bg::draw_particles(particles, hue, ctx, audio_data, self.width, self.height);
        }

        ctx.restore();

        self.hue = (self.hue + 1.0) % 360.0;
        self.brightness = (self.brightness + (random() * 10.0 - 5.0)) % 100.0;
    }

    fn draw_particles(
        particles: &mut Vec<Particle>,
        hue: f64,
        ctx: &CanvasRenderingContext2d,
        audio_data: &[u8],
        width: u32,
        height: u32,
    ) {
        let treble = audio_data.iter().skip(10).take(20).map(|&x| x as f64).sum::<f64>() / 20.0;

        for particle in particles.iter_mut() {
            particle.update(treble, width, height);

            ctx.set_fill_style(&JsValue::from_str(&format!(
                "hsla({}, 100%, 50%, 0.8)",
                (hue + particle.lifetime) % 360.0
            )));

            ctx.begin_path();
            ctx.arc(particle.x, particle.y, particle.size, 0.0, PI * 2.0).unwrap();
            ctx.fill();
        }
    }
}

#[derive(Clone)]
struct Particle {
    x: f64,
    y: f64,
    size: f64,
    lifetime: f64,
    speed_x: f64,
    speed_y: f64,
}

impl Particle {
    fn new(width: u32, height: u32) -> Particle {
        Particle {
            x: (random() * width as f64) - (width as f64 / 2.0),
            y: (random() * height as f64) - (height as f64 / 2.0),
            size: random() * 3.0 + 1.0,
            lifetime: 0.0,
            speed_x: random() * 2.0 - 1.0,
            speed_y: random() * 2.0 - 1.0,
        }
    }

    fn update(&mut self, treble: f64, width: u32, height: u32) {
        self.x += self.speed_x * treble / 255.0;
        self.y += self.speed_y * treble / 255.0;
        self.lifetime += 1.0;

        if self.x > width as f64 / 2.0 || self.x < -(width as f64 / 2.0) || self.y > height as f64 / 2.0 || self.y < -(height as f64 / 2.0) {
            *self = Particle::new(width, height);
        }
    }
}
