//! Gray–Scott reaction–diffusion: two chemicals that grow coral, spots and mazes.

use std::time::{Duration, Instant};

use eframe::egui::{self, Rect, Sense, TextureOptions, vec2};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;
use crate::rng::Rng;

const PRESETS: [(&str, f32, f32); 6] = [
    ("Coral", 0.0545, 0.062),
    ("Mitosis", 0.0367, 0.0649),
    ("Mazes", 0.029, 0.057),
    ("Worms", 0.078, 0.061),
    ("Holes", 0.039, 0.058),
    ("Chaos", 0.026, 0.051),
];

/// 9-point Laplacian from three 3-wide row windows: 0.2 for edges, 0.05 for corners, -1 centre.
#[inline(always)]
fn lap(u: [f32; 3], m: [f32; 3], d: [f32; 3]) -> f32 {
    0.2 * (u[1] + d[1] + m[0] + m[2]) + 0.05 * (u[0] + u[2] + d[0] + d[2]) - m[1]
}

/// The reaction itself, given both Laplacians. Returns the new (a, b).
#[inline(always)]
fn react(av: f32, bv: f32, la: f32, lb: f32, feed: f32, kill: f32) -> (f32, f32) {
    let abb = av * bv * bv;
    let na = (av + la - abb + feed * (1.0 - av)).clamp(0.0, 1.0);
    let nb = bv + 0.5 * lb + abb - (kill + feed) * bv;
    // Flush near-zero B to exactly zero: decaying values would otherwise
    // become denormal floats, which are ~100× slower on x86.
    (na, if nb < 1e-12 { 0.0 } else { nb.min(1.0) })
}

/// One Gray–Scott step on an `n`×`n` torus: reads (a, b), writes (a2, b2).
pub fn step(a: &[f32], b: &[f32], a2: &mut [f32], b2: &mut [f32], n: usize, feed: f32, kill: f32) {
    a2.par_chunks_mut(n).zip(b2.par_chunks_mut(n)).enumerate().for_each(|(y, (ra, rb))| {
        fn row(f: &[f32], r: usize, n: usize) -> &[f32] {
            &f[r * n..][..n]
        }
        let row = |f, r| row(f, r, n);
        let (yu, yd) = ((y + n - 1) % n, (y + 1) % n);
        let (au, am, ad) = (row(a, yu), row(a, y), row(a, yd));
        let (bu, bm, bd) = (row(b, yu), row(b, y), row(b, yd));
        let w3 = |s: &[f32]| -> [f32; 3] { s.try_into().unwrap() };

        // Interior: straight-line code over 3-wide windows, no bounds checks, so it vectorizes.
        let interior = au
            .windows(3)
            .zip(am.windows(3))
            .zip(ad.windows(3))
            .zip(bu.windows(3).zip(bm.windows(3)).zip(bd.windows(3)))
            .zip(ra[1..n - 1].iter_mut().zip(&mut rb[1..n - 1]));
        for ((((u, m), d), ((bu, bm), bd)), (oa, ob)) in interior {
            let (m3, bm3) = (w3(m), w3(bm));
            let la = lap(w3(u), m3, w3(d));
            let lb = lap(w3(bu), bm3, w3(bd));
            (*oa, *ob) = react(m3[1], bm3[1], la, lb, feed, kill);
        }

        // The two edge columns wrap around.
        for x in [0, n - 1] {
            let (l, r) = ((x + n - 1) % n, (x + 1) % n);
            let pick = |s: &[f32]| [s[l], s[x], s[r]];
            let la = lap(pick(au), pick(am), pick(ad));
            let lb = lap(pick(bu), pick(bm), pick(bd));
            (ra[x], rb[x]) = react(am[x], bm[x], la, lb, feed, kill);
        }
    });
}

pub struct Reaction {
    n: usize,
    a: Vec<f32>,
    b: Vec<f32>,
    a2: Vec<f32>,
    b2: Vec<f32>,
    preset: usize,
    iters: usize,
    canvas: Canvas,
    lut: [[u8; 3]; 256],
    rng: Rng,
    step_time: Duration,
    fps: FpsCounter,
    fps_now: usize,
}

impl Reaction {
    pub fn new() -> Self {
        let mut r = Self {
            n: 0,
            a: vec![],
            b: vec![],
            a2: vec![],
            b2: vec![],
            preset: 0,
            iters: 24,
            canvas: Canvas::new("reaction", TextureOptions::LINEAR),
            lut: lut(),
            rng: Rng::new(11),
            step_time: Duration::ZERO,
            fps: FpsCounter::default(),
            fps_now: 0,
        };
        r.reset(768);
        r
    }

    fn reset(&mut self, n: usize) {
        self.n = n;
        self.a = vec![1.0; n * n];
        self.b = vec![0.0; n * n];
        self.a2 = self.a.clone();
        self.b2 = self.b.clone();
        for _ in 0..24 {
            let x = (self.rng.f32() * n as f32) as usize;
            let y = (self.rng.f32() * n as f32) as usize;
            let r = 3.0 + self.rng.f32() * 8.0;
            self.seed(x as f32, y as f32, r);
        }
    }

    /// Drops a blob of chemical B centred on (cx, cy), wrapping around the edges.
    fn seed(&mut self, cx: f32, cy: f32, r: f32) {
        let n = self.n as i64;
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r {
                    let x = (cx as i64 + dx).rem_euclid(n) as usize;
                    let y = (cy as i64 + dy).rem_euclid(n) as usize;
                    let i = y * self.n + x;
                    self.b[i] = 1.0;
                    self.a[i] = 0.5;
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut size = self.n;
        ui.horizontal(|ui| {
            ui.label("Pattern");
            for (i, (name, _, _)) in PRESETS.iter().enumerate() {
                ui.selectable_value(&mut self.preset, i, *name);
            }
            ui.separator();
            ui.label("Steps / frame");
            ui.add(egui::Slider::new(&mut self.iters, 1..=64));
            egui::ComboBox::from_id_salt("rd-size")
                .selected_text(format!("{0}×{0}", self.n))
                .show_ui(ui, |ui| {
                    for n in [512, 768, 1024, 1536] {
                        ui.selectable_value(&mut size, n, format!("{n}×{n}"));
                    }
                });
            if ui.button("Reset").clicked() {
                size = 0;
            }
            ui.label(egui::RichText::new("Click or drag to seed").color(canvas::DIM));
        });
        if size != self.n {
            self.reset(if size == 0 { self.n } else { size });
        }
        ui.add_space(6.0);

        let (area, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let side = area.width().min(area.height());
        let rect = Rect::from_center_size(area.center(), vec2(side, side));
        let n = self.n;

        if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on()) {
            if rect.contains(pos) {
                let x = (pos.x - rect.min.x) / side * n as f32;
                let y = (pos.y - rect.min.y) / side * n as f32;
                self.seed(x, y, n as f32 / 120.0);
            }
        }

        let (_, feed, kill) = PRESETS[self.preset];
        let start = Instant::now();
        for _ in 0..self.iters {
            step(&self.a, &self.b, &mut self.a2, &mut self.b2, n, feed, kill);
            std::mem::swap(&mut self.a, &mut self.a2);
            std::mem::swap(&mut self.b, &mut self.b2);
        }
        self.step_time = start.elapsed() / self.iters as u32;

        // Colour by B, with a light-from-top-left highlight for an embossed look.
        self.canvas.resize(n, n);
        let (b, lut) = (&self.b, &self.lut);
        self.canvas.rgba.par_chunks_mut(n * 4).enumerate().for_each(|(y, row)| {
            let up = ((y + n - 1) % n) * n;
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let v = b[y * n + x];
                let l = if x == 0 { n - 1 } else { x - 1 };
                let slope = (v - b[up + l]) * 6.0;
                let t = (v * 2.6).min(1.0);
                let c = lut[(t * 255.0) as usize];
                let shade = |ch: u8| (ch as f32 * (1.0 + slope.clamp(-0.35, 0.5))).min(255.0) as u8;
                px.copy_from_slice(&[shade(c[0]), shade(c[1]), shade(c[2]), 255]);
            }
        });

        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, canvas::BG);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        let cells = (n * n) as f64;
        canvas::hud(
            &painter,
            area,
            &[
                ("grid", format!("{n}×{n}")),
                ("steps / frame", self.iters.to_string()),
                ("one step", fmt::duration(self.step_time)),
                ("cell updates", fmt::rate(cells, self.step_time)),
                ("feed · kill", format!("{feed:.4} · {kill:.4}")),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

/// Deep plum → rust → cream.
fn lut() -> [[u8; 3]; 256] {
    let stops: [(f32, [f32; 3]); 5] = [
        (0.0, [0.04, 0.03, 0.06]),
        (0.3, [0.25, 0.06, 0.12]),
        (0.6, [0.93, 0.38, 0.13]),
        (0.85, [1.0, 0.72, 0.42]),
        (1.0, [1.0, 0.95, 0.86]),
    ];
    let mut out = [[0u8; 3]; 256];
    for (i, c) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let k = stops.windows(2).position(|s| t <= s[1].0).unwrap_or(stops.len() - 2);
        let ((t0, a), (t1, b)) = (stops[k], stops[k + 1]);
        let f = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        for j in 0..3 {
            c[j] = ((a[j] + (b[j] - a[j]) * f) * 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_field_is_stable_and_values_bounded() {
        let n = 32;
        let (a, b) = (vec![1.0f32; n * n], vec![0.0f32; n * n]);
        let (mut a2, mut b2) = (a.clone(), b.clone());
        step(&a, &b, &mut a2, &mut b2, n, 0.0545, 0.062);
        assert_eq!(a, a2);
        assert_eq!(b, b2);

        let mut r = Reaction::new();
        for _ in 0..200 {
            step(&r.a, &r.b, &mut r.a2, &mut r.b2, r.n, 0.026, 0.051);
            std::mem::swap(&mut r.a, &mut r.a2);
            std::mem::swap(&mut r.b, &mut r.b2);
        }
        assert!(r.a.iter().chain(&r.b).all(|v| (0.0..=1.0).contains(v)));
        assert!(r.b.iter().any(|&v| v > 0.1), "pattern died out");
    }
}
