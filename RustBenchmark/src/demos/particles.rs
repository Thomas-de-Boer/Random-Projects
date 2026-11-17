//! A million particles pulled around by the cursor, simulated on every core.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;
use crate::rng::Rng;

#[derive(Clone, Copy)]
struct P {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

pub struct Particles {
    ps: Vec<P>,
    density: Vec<AtomicU32>,
    canvas: Canvas,
    target_count: usize,
    lut: [[u8; 4]; LUT_LEN],
    rng: Rng,
    time: f32,
    fps: FpsCounter,
    fps_now: usize,
    sim: Duration,
    draw: Duration,
}

const LUT_LEN: usize = 128;
/// Sideways share of each attractor's pull.
const SWIRL: f32 = 0.45;

impl Particles {
    pub fn new() -> Self {
        Self {
            ps: Vec::new(),
            density: Vec::new(),
            canvas: Canvas::new("particles", TextureOptions::LINEAR),
            target_count: 1_000_000,
            lut: heat_lut(),
            rng: Rng::new(7),
            time: 0.0,
            fps: FpsCounter::default(),
            fps_now: 0,
            sim: Duration::ZERO,
            draw: Duration::ZERO,
        }
    }

    fn spawn(&mut self, n: usize) {
        let (w, h) = (self.canvas.w as f32, self.canvas.h as f32);
        let rng = &mut self.rng;
        self.ps.truncate(n);
        while self.ps.len() < n {
            let x = rng.f32() * w;
            let y = rng.f32() * h;
            // Start everything circling the centre so the first frame already moves.
            let (dx, dy) = (x - w / 2.0, y - h / 2.0);
            let s = 0.004;
            self.ps.push(P { x, y, vx: -dy * s, vy: dx * s });
        }
    }

    fn explode(&mut self) {
        let (cx, cy) = (self.canvas.w as f32 / 2.0, self.canvas.h as f32 / 2.0);
        let scale = self.canvas.w.min(self.canvas.h) as f32 * 0.02;
        let rng = &mut self.rng;
        for p in &mut self.ps {
            let a = rng.f32() * std::f32::consts::TAU;
            let v = rng.f32().sqrt() * scale;
            p.x = cx;
            p.y = cy;
            p.vx = a.cos() * v;
            p.vy = a.sin() * v;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut explode = false;
        ui.horizontal(|ui| {
            ui.label("Particles");
            ui.add(
                egui::Slider::new(&mut self.target_count, 100_000..=4_000_000)
                    .logarithmic(true)
                    .custom_formatter(|v, _| fmt::grouped(v as u64)),
            );
            explode = ui.button("Explode").clicked();
            if ui.button("Reset").clicked() {
                self.ps.clear();
            }
            ui.label(
                egui::RichText::new("Move the mouse to pull · hold the button to push")
                    .color(canvas::DIM),
            );
        });
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let (w, h) = canvas::pixel_size(&ctx, rect, 1.0, 1920);
        let (ow, oh) = (self.canvas.w as f32, self.canvas.h as f32);
        if self.canvas.resize(w, h) {
            self.density = (0..w * h).map(|_| AtomicU32::new(0)).collect();
            if ow > 0.0 {
                let (sx, sy) = (w as f32 / ow, h as f32 / oh);
                self.ps.par_iter_mut().for_each(|p| {
                    p.x *= sx;
                    p.y *= sy;
                });
            }
        }
        if self.ps.len() != self.target_count {
            self.spawn(self.target_count);
        }
        if explode {
            self.explode();
        }

        // Where the forces come from: the cursor, or two drifting attractors.
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        self.time += dt;
        let (wf, hf) = (w as f32, h as f32);
        let pointer = resp.hover_pos().filter(|_| resp.hovered() || resp.dragged());
        let attractors: Vec<(f32, f32, f32)> = match pointer {
            Some(pos) => {
                let x = (pos.x - rect.min.x) / rect.width() * wf;
                let y = (pos.y - rect.min.y) / rect.height() * hf;
                let push = ctx.input(|i| i.pointer.primary_down());
                vec![(x, y, if push { -2.5 } else { 1.0 })]
            }
            None => {
                let t = self.time;
                let r = wf.min(hf) * 0.28;
                vec![
                    (wf / 2.0 + r * (t * 0.7).cos(), hf / 2.0 + r * (t * 1.1).sin(), 0.6),
                    (wf / 2.0 - r * (t * 0.9).cos(), hf / 2.0 - r * (t * 0.6).sin(), 0.6),
                ]
            }
        };

        // ---- Physics ----
        let start = Instant::now();
        let g = wf.min(hf) * 0.1;
        let soft = (wf.min(hf) * 0.01).powi(2);
        self.ps.par_iter_mut().with_min_len(4096).for_each(|p| {
            let (mut ax, mut ay) = (0.0, 0.0);
            for &(qx, qy, s) in &attractors {
                let (dx, dy) = (qx - p.x, qy - p.y);
                let inv = s * g / (dx * dx + dy * dy + soft);
                // Pull in, plus a sideways swirl so particles orbit instead of clumping.
                ax += (dx - dy * SWIRL) * inv;
                ay += (dy + dx * SWIRL) * inv;
            }
            p.vx = (p.vx + ax) * 0.99;
            p.vy = (p.vy + ay) * 0.99;
            p.x += p.vx;
            p.y += p.vy;
            if p.x < 0.0 || p.x >= wf {
                p.vx = -p.vx * 0.6;
                p.x = p.x.clamp(0.0, wf - 1.0);
            }
            if p.y < 0.0 || p.y >= hf {
                p.vy = -p.vy * 0.6;
                p.y = p.y.clamp(0.0, hf - 1.0);
            }
        });
        self.sim = start.elapsed();

        // ---- Rendering: count particles per pixel, then map counts to colour ----
        let start = Instant::now();
        self.density.par_iter().with_min_len(8192).for_each(|d| d.store(0, Ordering::Relaxed));
        let density = &self.density;
        self.ps.par_iter().with_min_len(4096).for_each(|p| {
            let i = p.y as usize * w + p.x as usize;
            density[i].fetch_add(1, Ordering::Relaxed);
        });
        let lut = &self.lut;
        self.canvas
            .rgba
            .par_chunks_mut(w * 4)
            .zip(self.density.par_chunks(w))
            .for_each(|(row, dens)| {
                for (px, d) in row.chunks_exact_mut(4).zip(dens) {
                    let c = (d.load(Ordering::Relaxed) as usize).min(LUT_LEN - 1);
                    px.copy_from_slice(&lut[c]);
                }
            });
        self.draw = start.elapsed();

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        let n = self.ps.len() as f64;
        canvas::hud(
            &painter,
            rect,
            &[
                ("particles", fmt::grouped(self.ps.len() as u64)),
                ("threads", rayon::current_num_threads().to_string()),
                ("physics", fmt::duration(self.sim)),
                ("render", fmt::duration(self.draw)),
                ("updates", fmt::rate(n, self.sim)),
                ("resolution", format!("{w}×{h}")),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

/// Maps "particles in this pixel" to a black → ember → rust → white ramp.
fn heat_lut() -> [[u8; 4]; LUT_LEN] {
    let stops: [(f32, [f32; 3]); 5] = [
        (0.0, [0.03, 0.02, 0.04]),
        (0.25, [0.45, 0.07, 0.03]),
        (0.55, [0.96, 0.42, 0.15]),
        (0.8, [1.0, 0.75, 0.45]),
        (1.0, [1.0, 0.98, 0.92]),
    ];
    let mut lut = [[0u8; 4]; LUT_LEN];
    for (c, out) in lut.iter_mut().enumerate() {
        let t = 1.0 - (-(c as f32) / 6.0).exp();
        let k = stops.windows(2).position(|s| t <= s[1].0).unwrap_or(stops.len() - 2);
        let (t0, a) = stops[k];
        let (t1, b) = stops[k + 1];
        let f = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        for i in 0..3 {
            out[i] = ((a[i] + (b[i] - a[i]) * f) * 255.0) as u8;
        }
        out[3] = 255;
    }
    lut
}
