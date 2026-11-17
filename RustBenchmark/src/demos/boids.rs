//! Tens of thousands of flocking boids, using a uniform grid for neighbour lookups.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;
use crate::rng::Rng;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Boid {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
}

/// Boids bucketed by grid cell (a counting sort), so a neighbour search only
/// looks at the 3×3 cells around a point instead of every boid.
pub struct Grid {
    pub sorted: Vec<Boid>,
    starts: Vec<u32>,
    gw: usize,
    gh: usize,
    cell: f32,
}

impl Grid {
    pub fn build(boids: &[Boid], w: f32, h: f32, cell: f32) -> Grid {
        let gw = ((w / cell).ceil() as usize).max(1);
        let gh = ((h / cell).ceil() as usize).max(1);
        let index = |b: &Boid| {
            let cx = ((b.x / cell) as usize).min(gw - 1);
            let cy = ((b.y / cell) as usize).min(gh - 1);
            cy * gw + cx
        };
        // 1. Count boids per cell (in parallel).
        let counts: Vec<AtomicU32> = (0..gw * gh).map(|_| AtomicU32::new(0)).collect();
        boids.par_iter().for_each(|b| {
            counts[index(b)].fetch_add(1, Ordering::Relaxed);
        });
        // 2. Prefix sum gives each cell's start offset.
        let mut starts = Vec::with_capacity(gw * gh + 1);
        let mut total = 0u32;
        for c in &counts {
            starts.push(total);
            total += c.load(Ordering::Relaxed);
        }
        starts.push(total);
        // 3. Scatter every boid into its cell's slot range (in parallel).
        let cursor: Vec<AtomicU32> = starts[..gw * gh].iter().map(|&s| AtomicU32::new(s)).collect();
        let slots: Vec<AtomicBoid> = (0..boids.len()).map(|_| AtomicBoid::default()).collect();
        boids.par_iter().for_each(|b| {
            let slot = cursor[index(b)].fetch_add(1, Ordering::Relaxed) as usize;
            slots[slot].store(*b);
        });
        let sorted = slots.into_iter().map(AtomicBoid::into_inner).collect();
        Grid { sorted, starts, gw, gh, cell }
    }

    /// Calls `f` with every boid in the 3×3 cells around (x, y); stops when `f` returns false.
    #[inline]
    pub fn near(&self, x: f32, y: f32, mut f: impl FnMut(&Boid) -> bool) {
        let cx = ((x / self.cell) as usize).min(self.gw - 1);
        let cy = ((y / self.cell) as usize).min(self.gh - 1);
        for yy in cy.saturating_sub(1)..=(cy + 1).min(self.gh - 1) {
            let row = yy * self.gw;
            let (a, b) = (cx.saturating_sub(1), (cx + 1).min(self.gw - 1));
            // Cells a..=b in one row are contiguous in `sorted`.
            let (s, e) = (self.starts[row + a] as usize, self.starts[row + b + 1] as usize);
            for o in &self.sorted[s..e] {
                if !f(o) {
                    return;
                }
            }
        }
    }
}

/// Four f32s stored as atomics so parallel scatter needs no unsafe code.
#[derive(Default)]
struct AtomicBoid([AtomicU32; 4]);

impl AtomicBoid {
    fn store(&self, b: Boid) {
        for (a, v) in self.0.iter().zip([b.x, b.y, b.vx, b.vy]) {
            a.store(v.to_bits(), Ordering::Relaxed);
        }
    }
    fn into_inner(self) -> Boid {
        let [x, y, vx, vy] = self.0.map(|a| f32::from_bits(a.into_inner()));
        Boid { x, y, vx, vy }
    }
}

pub struct Boids {
    boids: Vec<Boid>,
    target: usize,
    w: f32,
    h: f32,
    canvas: Canvas,
    palette: [[u8; 3]; 64],
    rng: Rng,
    radius: f32,
    trails: bool,
    checks: u64,
    step_time: Duration,
    fps: FpsCounter,
    fps_now: usize,
    time: f32,
}

const MAX_NEIGHBOURS: u32 = 40;

impl Boids {
    pub fn new() -> Self {
        Self {
            boids: vec![],
            target: 30_000,
            w: 0.0,
            h: 0.0,
            canvas: Canvas::new("boids", TextureOptions::LINEAR),
            palette: palette(),
            rng: Rng::new(17),
            radius: 14.0,
            trails: true,
            checks: 0,
            step_time: Duration::ZERO,
            fps: FpsCounter::default(),
            fps_now: 0,
            time: 0.0,
        }
    }

    fn spawn(&mut self) {
        let rng = &mut self.rng;
        self.boids.truncate(self.target);
        while self.boids.len() < self.target {
            let a = rng.f32() * std::f32::consts::TAU;
            self.boids.push(Boid {
                x: rng.f32() * self.w,
                y: rng.f32() * self.h,
                vx: a.cos() * 2.0,
                vy: a.sin() * 2.0,
            });
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Boids");
            ui.add(
                egui::Slider::new(&mut self.target, 5_000..=200_000)
                    .logarithmic(true)
                    .custom_formatter(|v, _| fmt::grouped(v as u64)),
            );
            ui.label("Vision");
            ui.add(egui::Slider::new(&mut self.radius, 6.0..=30.0).suffix(" px"));
            ui.checkbox(&mut self.trails, "Trails");
            if ui.button("Scatter").clicked() {
                self.boids.clear();
            }
            ui.label(egui::RichText::new("They flee the cursor · hold the button to attract").color(canvas::DIM));
        });
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let (w, h) = canvas::pixel_size(&ctx, rect, 1.0, 2560);
        if self.canvas.resize(w, h) {
            let (sx, sy) = (w as f32 / self.w.max(1.0), h as f32 / self.h.max(1.0));
            for b in &mut self.boids {
                b.x = (b.x * sx).min(w as f32 - 0.01);
                b.y = (b.y * sy).min(h as f32 - 0.01);
            }
            self.w = w as f32;
            self.h = h as f32;
        }
        if self.boids.len() != self.target {
            self.spawn();
        }
        let (wf, hf) = (self.w, self.h);
        let ppp = ctx.pixels_per_point();
        let r = self.radius * ppp;

        // Things to flee from (or fly to): the cursor, or two roaming predators when idle.
        self.time += ctx.input(|i| i.stable_dt).min(0.05);
        let threats: Vec<(f32, f32, bool)> = match resp.hover_pos() {
            Some(p) => {
                let attract = ctx.input(|i| i.pointer.primary_down());
                vec![((p.x - rect.min.x) * w as f32 / rect.width(), (p.y - rect.min.y) * h as f32 / rect.height(), attract)]
            }
            None => {
                let t = self.time * 0.35;
                vec![
                    (wf * (0.5 + 0.4 * (t * 1.3).sin()), hf * (0.5 + 0.4 * (t * 0.8).cos()), false),
                    (wf * (0.5 + 0.4 * (t * 0.7 + 2.0).cos()), hf * (0.5 + 0.4 * (t * 1.1).sin()), false),
                ]
            }
        };

        // ---- Simulation ----
        let start = Instant::now();
        let grid = Grid::build(&self.boids, wf, hf, r);
        let (r2, sep2) = (r * r, (r * 0.45).powi(2));
        let (min_v, max_v) = (1.2 * ppp, 3.2 * ppp);
        let checks: u64 = self
            .boids
            .par_iter_mut()
            .zip(grid.sorted.par_iter())
            .with_min_len(256)
            .map(|(out, me)| {
                let (mut n, mut checks) = (0u32, 0u64);
                let (mut ax, mut ay, mut cx, mut cy, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                grid.near(me.x, me.y, |o| {
                    checks += 1;
                    let (dx, dy) = (o.x - me.x, o.y - me.y);
                    let d2 = dx * dx + dy * dy;
                    if d2 < r2 && d2 > 0.0 {
                        n += 1;
                        ax += o.vx;
                        ay += o.vy;
                        cx += dx;
                        cy += dy;
                        if d2 < sep2 {
                            sx -= dx / d2;
                            sy -= dy / d2;
                        }
                    }
                    n < MAX_NEIGHBOURS
                });
                let mut b = *me;
                if n > 0 {
                    let inv = 1.0 / n as f32;
                    b.vx += (ax * inv - b.vx) * 0.06 + cx * inv * 0.004 + sx * 1.6 * ppp;
                    b.vy += (ay * inv - b.vy) * 0.06 + cy * inv * 0.004 + sy * 1.6 * ppp;
                }
                // A little wander, so the flock never locks into one global heading.
                let jitter = ((b.x * 12.9898 + b.y * 78.233).sin() * 43_758.547).fract() * 0.16;
                let (sin, cos) = jitter.sin_cos();
                (b.vx, b.vy) = (b.vx * cos - b.vy * sin, b.vx * sin + b.vy * cos);
                for &(mx, my, attract) in &threats {
                    let (dx, dy) = (b.x - mx, b.y - my);
                    let d = (dx * dx + dy * dy).sqrt() + 1.0;
                    let reach = 140.0 * ppp;
                    if attract {
                        b.vx -= dx / d * 0.15;
                        b.vy -= dy / d * 0.15;
                    } else if d < reach {
                        let k = (1.0 - d / reach) * 1.2;
                        b.vx += dx / d * k;
                        b.vy += dy / d * k;
                    }
                }
                let speed = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1e-4);
                let s = speed.clamp(min_v, max_v) / speed;
                b.vx *= s;
                b.vy *= s;
                b.x = (b.x + b.vx).rem_euclid(wf);
                b.y = (b.y + b.vy).rem_euclid(hf);
                if b.x >= wf {
                    b.x = 0.0;
                }
                if b.y >= hf {
                    b.y = 0.0;
                }
                *out = b;
                checks
            })
            .sum();
        self.checks = checks;
        self.step_time = start.elapsed();

        // ---- Draw: fade the last frame, then streak each boid along its heading ----
        let keep = if self.trails { 200u32 } else { 0 };
        self.canvas.rgba.par_chunks_mut(4096).for_each(|c| {
            for px in c.chunks_exact_mut(4) {
                for v in &mut px[..3] {
                    *v = ((*v as u32 * keep) >> 8).max(10) as u8;
                }
                px[3] = 255;
            }
        });
        let rgba = &mut self.canvas.rgba;
        for b in &self.boids {
            let angle = b.vy.atan2(b.vx);
            let hue = (((angle / std::f32::consts::TAU) + 0.5) * 64.0) as usize & 63;
            let col = self.palette[hue];
            let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
            let (ux, uy) = (b.vx / speed, b.vy / speed);
            for k in 0..3 {
                let x = (b.x - ux * k as f32 * ppp) as usize;
                let y = (b.y - uy * k as f32 * ppp) as usize;
                if x < w && y < h {
                    let i = (y * w + x) * 4;
                    let fade = 255 - k * 60;
                    for c in 0..3 {
                        let v = (col[c] as u32 * fade as u32 / 255) as u8;
                        rgba[i + c] = rgba[i + c].max(v);
                    }
                }
            }
        }

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        canvas::hud(
            &painter,
            rect,
            &[
                ("boids", fmt::grouped(self.boids.len() as u64)),
                ("neighbour checks", fmt::count(self.checks as f64)),
                ("vs brute force", fmt::count((self.boids.len() as f64).powi(2))),
                ("simulation", fmt::duration(self.step_time)),
                ("boid updates", fmt::rate(self.boids.len() as f64, self.step_time)),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

/// A muted rainbow keyed on heading, so flocks flying together share a colour.
fn palette() -> [[u8; 3]; 64] {
    let mut p = [[0u8; 3]; 64];
    for (i, c) in p.iter_mut().enumerate() {
        let t = i as f32 / 64.0 * std::f32::consts::TAU;
        *c = [
            (150.0 + 105.0 * t.cos()) as u8,
            (130.0 + 90.0 * (t + 2.1).cos()) as u8,
            (140.0 + 100.0 * (t + 4.2).cos()) as u8,
        ];
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_finds_exactly_the_brute_force_neighbours() {
        let mut rng = Rng::new(2);
        let (w, h, r) = (300.0, 200.0, 20.0);
        let boids: Vec<Boid> = (0..500)
            .map(|_| Boid { x: rng.f32() * w, y: rng.f32() * h, vx: rng.f32(), vy: rng.f32() })
            .collect();
        let grid = Grid::build(&boids, w, h, r);
        let mut sorted_input = boids.clone();
        let key = |b: &Boid| (b.x.to_bits(), b.y.to_bits());
        sorted_input.sort_by_key(key);
        let mut regrouped = grid.sorted.clone();
        regrouped.sort_by_key(key);
        assert_eq!(sorted_input, regrouped, "grid must hold every boid exactly once");

        for me in &boids {
            let close = |o: &Boid| (o.x - me.x).powi(2) + (o.y - me.y).powi(2) < r * r;
            let mut brute: Vec<_> = boids.iter().filter(|o| close(o)).map(key).collect();
            let mut found = vec![];
            grid.near(me.x, me.y, |o| {
                if close(o) {
                    found.push(key(o));
                }
                true
            });
            brute.sort();
            found.sort();
            assert_eq!(brute, found);
        }
    }
}
