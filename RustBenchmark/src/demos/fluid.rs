//! "Stable fluids" smoke: semi-Lagrangian advection, vorticity confinement and a
//! Jacobi pressure solve, all on the CPU, every frame.

use std::time::{Duration, Instant};

use eframe::egui::{self, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;

pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub dye: [Vec<f32>; 3],
    p: Vec<f32>,
    div: Vec<f32>,
    tmp: Vec<f32>,
    tmp2: Vec<f32>,
}

/// Writes `f(x, y)` into every cell of `dst`, one row per task.
fn fill(dst: &mut [f32], w: usize, f: impl Fn(usize, usize) -> f32 + Sync) {
    dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            *o = f(x, y);
        }
    });
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Self {
        let z = vec![0.0; w * h];
        Self {
            w,
            h,
            u: z.clone(),
            v: z.clone(),
            dye: [z.clone(), z.clone(), z.clone()],
            p: z.clone(),
            div: z.clone(),
            tmp: z.clone(),
            tmp2: z,
        }
    }

    #[inline]
    fn at(&self, f: &[f32], x: isize, y: isize) -> f32 {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        f[y * self.w + x]
    }

    /// Bilinear sample with clamped edges.
    #[inline]
    fn sample(&self, f: &[f32], x: f32, y: f32) -> f32 {
        let x = x.clamp(0.0, (self.w - 1) as f32);
        let y = y.clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let w = self.w;
        let top = f[y0 * w + x0] * (1.0 - fx) + f[y0 * w + x1] * fx;
        let bot = f[y1 * w + x0] * (1.0 - fx) + f[y1 * w + x1] * fx;
        top * (1.0 - fy) + bot * fy
    }

    /// Moves `field` along the velocity: each cell looks back to where its contents came from.
    fn advect(&mut self, which: Field, decay: f32) {
        let mut out = std::mem::take(&mut self.tmp);
        {
            let src = self.field(which);
            fill(&mut out, self.w, |x, y| {
                let i = y * self.w + x;
                let (px, py) = (x as f32 - self.u[i], y as f32 - self.v[i]);
                self.sample(src, px, py) * decay
            });
        }
        self.tmp = std::mem::replace(self.field_mut(which), out);
    }

    /// Adds swirl back in where numerical damping removed it.
    fn vorticity(&mut self, strength: f32) {
        let mut curl = std::mem::take(&mut self.tmp);
        fill(&mut curl, self.w, |x, y| {
            let (x, y) = (x as isize, y as isize);
            0.5 * ((self.at(&self.v, x + 1, y) - self.at(&self.v, x - 1, y))
                - (self.at(&self.u, x, y + 1) - self.at(&self.u, x, y - 1)))
        });
        let mut fx = std::mem::take(&mut self.tmp2);
        let mut fy = std::mem::take(&mut self.div);
        let w = self.w;
        {
            let this = &*self;
            let force = |x: usize, y: usize| {
                let (xi, yi) = (x as isize, y as isize);
                let gx = this.at(&curl, xi + 1, yi).abs() - this.at(&curl, xi - 1, yi).abs();
                let gy = this.at(&curl, xi, yi + 1).abs() - this.at(&curl, xi, yi - 1).abs();
                let len = (gx * gx + gy * gy).sqrt() + 1e-5;
                let c = curl[y * w + x];
                (gy / len * c * strength, -gx / len * c * strength)
            };
            fill(&mut fx, w, |x, y| force(x, y).0);
            fill(&mut fy, w, |x, y| force(x, y).1);
        }
        self.u.par_iter_mut().zip(&fx).for_each(|(u, f)| *u += f);
        self.v.par_iter_mut().zip(&fy).for_each(|(v, f)| *v += f);
        self.tmp = curl;
        self.tmp2 = fx;
        self.div = fy;
    }

    /// Makes the velocity field divergence-free (incompressible) with `iters` Jacobi sweeps.
    pub fn project(&mut self, iters: usize) {
        let w = self.w;
        // Solid walls: no flow through the border.
        let (wi, hi) = (self.w, self.h);
        for f in [&mut self.u, &mut self.v] {
            for x in 0..wi {
                f[x] = 0.0;
                f[(hi - 1) * wi + x] = 0.0;
            }
            for y in 0..hi {
                f[y * wi] = 0.0;
                f[y * wi + wi - 1] = 0.0;
            }
        }
        let mut div = std::mem::take(&mut self.div);
        fill(&mut div, w, |x, y| {
            let (x, y) = (x as isize, y as isize);
            -0.5 * (self.at(&self.u, x + 1, y) - self.at(&self.u, x - 1, y) + self.at(&self.v, x, y + 1)
                - self.at(&self.v, x, y - 1))
        });
        // Pressure from last frame is a good first guess.
        let mut p = std::mem::take(&mut self.p);
        let mut p2 = std::mem::take(&mut self.tmp);
        // The divergence and gradient use central differences (±1), so the matching
        // Laplacian reaches ±2 cells; using it makes the projection exact at convergence.
        for _ in 0..iters {
            fill(&mut p2, w, |x, y| {
                let (xi, yi) = (x as isize, y as isize);
                (self.at(&p, xi - 2, yi)
                    + self.at(&p, xi + 2, yi)
                    + self.at(&p, xi, yi - 2)
                    + self.at(&p, xi, yi + 2))
                    * 0.25
                    + div[y * w + x]
            });
            std::mem::swap(&mut p, &mut p2);
        }
        let mut nu = std::mem::take(&mut self.tmp2);
        fill(&mut nu, w, |x, y| {
            let (xi, yi) = (x as isize, y as isize);
            self.u[y * w + x] - 0.5 * (self.at(&p, xi + 1, yi) - self.at(&p, xi - 1, yi))
        });
        let mut nv = std::mem::take(&mut div);
        fill(&mut nv, w, |x, y| {
            let (xi, yi) = (x as isize, y as isize);
            self.v[y * w + x] - 0.5 * (self.at(&p, xi, yi + 1) - self.at(&p, xi, yi - 1))
        });
        self.tmp2 = std::mem::replace(&mut self.u, nu);
        self.div = std::mem::replace(&mut self.v, nv);
        self.p = p;
        self.tmp = p2;
    }

    #[cfg(test)]
    pub fn divergence(&self) -> f32 {
        let mut s = 0.0;
        for y in 1..self.h as isize - 1 {
            for x in 1..self.w as isize - 1 {
                let d = 0.5
                    * (self.at(&self.u, x + 1, y) - self.at(&self.u, x - 1, y) + self.at(&self.v, x, y + 1)
                        - self.at(&self.v, x, y - 1));
                s += d * d;
            }
        }
        s.sqrt()
    }

    /// Adds a soft blob of velocity and dye.
    fn splat(&mut self, cx: f32, cy: f32, vx: f32, vy: f32, color: [f32; 3], radius: f32) {
        let r = (radius * 3.0) as isize;
        let (cxi, cyi) = (cx as isize, cy as isize);
        for y in (cyi - r).max(1)..(cyi + r).min(self.h as isize - 1) {
            for x in (cxi - r).max(1)..(cxi + r).min(self.w as isize - 1) {
                let d2 = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
                let k = (-d2 / (radius * radius)).exp();
                let i = y as usize * self.w + x as usize;
                self.u[i] += vx * k;
                self.v[i] += vy * k;
                for c in 0..3 {
                    self.dye[c][i] += color[c] * k;
                }
            }
        }
    }

    pub fn step(&mut self, iters: usize, vorticity: f32) {
        self.vorticity(vorticity);
        self.project(iters);
        self.advect(Field::U, 0.999);
        self.advect(Field::V, 0.999);
        for c in 0..3 {
            self.advect(Field::Dye(c), 0.994);
        }
    }

    fn field(&self, f: Field) -> &[f32] {
        match f {
            Field::U => &self.u,
            Field::V => &self.v,
            Field::Dye(c) => &self.dye[c],
        }
    }

    fn field_mut(&mut self, f: Field) -> &mut Vec<f32> {
        match f {
            Field::U => &mut self.u,
            Field::V => &mut self.v,
            Field::Dye(c) => &mut self.dye[c],
        }
    }
}

#[derive(Clone, Copy)]
enum Field {
    U,
    V,
    Dye(usize),
}

pub struct Fluid {
    grid: Grid,
    resolution: usize,
    iters: usize,
    vorticity: f32,
    jets: bool,
    time: f32,
    canvas: Canvas,
    step_time: Duration,
    fps: FpsCounter,
    fps_now: usize,
}

impl Fluid {
    pub fn new() -> Self {
        Self {
            grid: Grid::new(1, 1),
            resolution: 512,
            iters: 40,
            vorticity: 0.25,
            jets: true,
            time: 0.0,
            canvas: Canvas::new("fluid", TextureOptions::LINEAR),
            step_time: Duration::ZERO,
            fps: FpsCounter::default(),
            fps_now: 0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut reset = false;
        ui.horizontal(|ui| {
            ui.label("Grid width");
            egui::ComboBox::from_id_salt("fluid-res")
                .selected_text(self.resolution.to_string())
                .show_ui(ui, |ui| {
                    for r in [256, 384, 512, 768, 1024] {
                        reset |= ui.selectable_value(&mut self.resolution, r, r.to_string()).changed();
                    }
                });
            ui.separator();
            ui.label("Pressure iterations");
            ui.add(egui::Slider::new(&mut self.iters, 5..=120));
            ui.label("Swirl");
            ui.add(egui::Slider::new(&mut self.vorticity, 0.0..=1.0));
            ui.checkbox(&mut self.jets, "Jets");
            reset |= ui.button("Clear").clicked();
            ui.label(egui::RichText::new("Drag to stir").color(canvas::DIM));
        });
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let gw = self.resolution;
        let gh = ((gw as f32 * rect.height() / rect.width()).round() as usize).max(16);
        if reset || (gw, gh) != (self.grid.w, self.grid.h) {
            self.grid = Grid::new(gw, gh);
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        self.time += dt;
        let scale = gw as f32 / 512.0;
        let hue = |t: f32| {
            [
                0.6 + 0.4 * (t).cos(),
                0.35 + 0.3 * (t + 2.0).cos(),
                0.3 + 0.3 * (t + 4.0).cos(),
            ]
        };

        if self.jets {
            let t = self.time;
            let speed = 4.0 * scale;
            for (k, (fx, color)) in [(0.3, [1.0, 0.42, 0.12]), (0.7, [0.15, 0.55, 0.9])].into_iter().enumerate() {
                let a = -std::f32::consts::FRAC_PI_2 + (t * 0.9 + k as f32 * 2.0).sin() * 0.6;
                self.grid.splat(
                    gw as f32 * fx,
                    gh as f32 * 0.92,
                    a.cos() * speed,
                    a.sin() * speed,
                    color.map(|c| c * 0.25),
                    6.0 * scale,
                );
            }
        }
        if let (Some(pos), true) = (resp.interact_pointer_pos(), resp.dragged()) {
            let d = resp.drag_delta();
            let sx = gw as f32 / rect.width();
            let x = (pos.x - rect.min.x) * sx;
            let y = (pos.y - rect.min.y) * sx;
            let color = hue(self.time * 1.5);
            self.grid.splat(x, y, d.x * sx * 0.8, d.y * sx * 0.8, color.map(|c| c * 0.6), 9.0 * scale);
        }

        let start = Instant::now();
        self.grid.step(self.iters, self.vorticity);
        self.step_time = start.elapsed();

        self.canvas.resize(gw, gh);
        let dye = &self.grid.dye;
        self.canvas.rgba.par_chunks_mut(gw * 4).enumerate().for_each(|(y, row)| {
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let i = y * gw + x;
                let tone = |d: f32, bg: f32| ((bg + (1.0 - (-d * 1.4).exp()) * (1.0 - bg)) * 255.0) as u8;
                px.copy_from_slice(&[tone(dye[0][i], 0.047), tone(dye[1][i], 0.04), tone(dye[2][i], 0.05), 255]);
            }
        });

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        // Per frame: vorticity (2 passes), divergence, N Jacobi sweeps, gradient (2), advection (5).
        let cells = (gw * gh) as f64;
        let passes = (self.iters + 10) as f64;
        canvas::hud(
            &painter,
            rect,
            &[
                ("grid", format!("{gw}×{gh}")),
                ("cells", fmt::count(cells)),
                ("solver passes", format!("{passes:.0} / frame")),
                ("simulation", fmt::duration(self.step_time)),
                ("cell updates", fmt::rate(cells * passes, self.step_time)),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn projection_removes_most_divergence() {
        let mut g = Grid::new(48, 40);
        let mut rng = Rng::new(1);
        for i in 0..g.u.len() {
            g.u[i] = rng.f32() - 0.5;
            g.v[i] = rng.f32() - 0.5;
        }
        g.project(0); // just applies the walls
        let before = g.divergence();
        g.project(400);
        let after = g.divergence();
        assert!(after < before * 0.05, "divergence {before} -> {after}");
    }

    #[test]
    fn stays_finite_and_bounded() {
        let mut g = Grid::new(64, 64);
        for _ in 0..200 {
            g.splat(32.0, 56.0, 0.0, -3.0, [0.3, 0.2, 0.1], 4.0);
            g.step(20, 0.3);
        }
        let all = g.u.iter().chain(&g.v).chain(&g.dye[0]);
        assert!(all.clone().all(|v| v.is_finite()));
        assert!(g.dye[0].iter().all(|&d| d < 200.0));
    }
}
