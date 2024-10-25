use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use std::f64::consts::PI;

#[derive(Clone)]
#[wasm_bindgen]
pub struct Visualizer {
    ctx: CanvasRenderingContext2d,
    width: u32,
    height: u32,
    center_x: f64,
    center_y: f64,
    previous_values: Vec<f64>,
    hue: f64,
}

#[wasm_bindgen]
impl Visualizer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Visualizer, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;
        
        let width = canvas.width();
        let height = canvas.height();
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        
        Ok(Visualizer {
            ctx,
            width,
            height,
            center_x,
            center_y,
            previous_values: vec![0.0; 128],
            hue: 0.0,
        })
    }

    #[wasm_bindgen]
    pub fn draw(&mut self, audio_data: &[u8]) {
        let ctx = &self.ctx;
        
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.1)"));
        ctx.fill_rect(0.0, 0.0, self.width as f64, self.height as f64);
        
        ctx.save();
        ctx.translate(self.center_x, self.center_y).unwrap();
        
        {
            let previous_values = &mut self.previous_values;
            let hue = &mut self.hue;
            let width = self.width;
            let height = self.height;
            Visualizer::draw_circular_visualizer(ctx, audio_data, previous_values, hue, width, height);
        }
        
        self.draw_center_orb(audio_data);
        
        self.draw_particles(audio_data);
        
        ctx.restore();
        
        self.hue = (self.hue + 0.5) % 360.0;
    }

    fn draw_circular_visualizer(
        ctx: &CanvasRenderingContext2d,
        audio_data: &[u8],
        previous_values: &mut Vec<f64>,
        hue: &mut f64,
        _width: u32,
        height: u32,
    ) {
        let bars = 128;
        let radius = height as f64 * 0.3;

        for i in 0..bars {
            let value = audio_data[i] as f64;
            let smoothed_value = (value + previous_values[i]) / 2.0;
            previous_values[i] = smoothed_value;
            
            let normalized = smoothed_value / 255.0;
            let bar_height = normalized * (height as f64 * 0.15);
            
            let angle = (i as f64 / bars as f64) * PI * 2.0;
            let x = angle.cos();
            let y = angle.sin();
            
            ctx.set_fill_style(&JsValue::from_str(&format!("hsl({}, 100%, 50%)", (*hue + i as f64) % 360.0)));
            
            ctx.begin_path();
            ctx.move_to(x * radius, y * radius);
            ctx.line_to(x * (radius + bar_height), y * (radius + bar_height));
            
            let next_angle = ((i + 1) as f64 / bars as f64) * PI * 2.0;
            ctx.line_to(next_angle.cos() * (radius + bar_height), next_angle.sin() * (radius + bar_height));
            ctx.line_to(next_angle.cos() * radius, next_angle.sin() * radius);
            ctx.close_path();
            ctx.fill();
        }
    }

    fn draw_center_orb(&self, audio_data: &[u8]) {
        let ctx = &self.ctx;
        let avg = audio_data.iter().map(|&x| x as f64).sum::<f64>() / audio_data.len() as f64;
        let radius = (avg / 255.0) * (self.height as f64 * 0.1) + 5.0;
        
        ctx.set_fill_style(&JsValue::from_str(&format!("hsla({}, 100%, 50%, 0.8)", self.hue)));
        
        ctx.begin_path();
        ctx.arc(0.0, 0.0, radius, 0.0, PI * 2.0).unwrap();
        ctx.fill();
    }

    fn draw_particles(&self, audio_data: &[u8]) {
        let ctx = &self.ctx;
        let bass = audio_data.iter().take(4).map(|&x| x as f64).sum::<f64>() / 4.0;

        if bass > 200.0 {
            for i in 0..20 {
                let angle = (i as f64 / 20.0) * PI * 2.0;
                let distance = bass / 255.0 * (self.height as f64 * 0.2);
                let x = angle.cos() * distance;
                let y = angle.sin() * distance;

                ctx.set_fill_style(&JsValue::from_str(&format!(
                    "hsla({}, 100%, 50%, 0.8)", 
                    (self.hue + i as f64 * 3.0) % 360.0
                )));
                ctx.begin_path();
                ctx.arc(x, y, 2.0, 0.0, PI * 2.0).unwrap();
                ctx.fill();
            }
        }
    }
}
