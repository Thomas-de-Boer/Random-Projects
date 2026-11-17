//! Conway's Game of Life on a multi-million-cell torus, many generations per frame.

use std::time::{Duration, Instant};

use eframe::egui::{self, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;
use crate::rng::Rng;

/// Rows handled per parallel task.
const BAND: usize = 8;

/// Advances `cur` one generation into `next` on an `n`×`n` wrapping grid of 0/1 cells.
pub fn step(cur: &[u8], next: &mut [u8], n: usize) {
    next.par_chunks_mut(n * BAND).enumerate().for_each(|(band, out)| {
        let mut cs = vec![0u8; n];
        for (r, nx) in out.chunks_exact_mut(n).enumerate() {
            let y = band * BAND + r;
            let up = &cur[((y + n - 1) % n) * n..][..n];
            let mid = &cur[y * n..][..n];
            let down = &cur[((y + 1) % n) * n..][..n];
            for (((c, a), b), d) in cs.iter_mut().zip(up).zip(mid).zip(down) {
                *c = a + b + d;
            }
            let rule = |s: u8, alive: u8| (s == 3 || (s == 2 && alive == 1)) as u8;
            // Neighbours = 3x3 column sums minus the cell itself.
            for ((w, m), o) in cs.windows(3).zip(&mid[1..n - 1]).zip(&mut nx[1..n - 1]) {
                *o = rule(w[0] + w[1] + w[2] - m, *m);
            }
            nx[0] = rule(cs[n - 1] + cs[0] + cs[1] - mid[0], mid[0]);
            nx[n - 1] = rule(cs[n - 2] + cs[n - 1] + cs[0] - mid[n - 1], mid[n - 1]);
        }
    });
}

pub struct Life {
    n: usize,
    cur: Vec<u8>,
    next: Vec<u8>,
    heat: Vec<u8>,
    canvas: Canvas,
    lut: [[u8; 4]; 256],
    rng: Rng,
    paused: bool,
    gens_per_frame: usize,
    generation: u64,
    step_time: Duration,
    fps: FpsCounter,
    fps_now: usize,
    /// View centre in cells, and cells per screen pixel. `None` = fit.
    view: Option<(f64, f64, f64)>,
}

impl Life {
    pub fn new() -> Self {
        let mut life = Self {
            n: 0,
            cur: Vec::new(),
            next: Vec::new(),
            heat: Vec::new(),
            canvas: Canvas::new("life", TextureOptions::NEAREST),
            lut: life_lut(),
            rng: Rng::new(42),
            paused: false,
            gens_per_frame: 16,
            generation: 0,
            step_time: Duration::ZERO,
            fps: FpsCounter::default(),
            fps_now: 0,
            view: None,
        };
        life.reset(2048);
        life
    }

    fn reset(&mut self, n: usize) {
        self.n = n;
        let rng = &mut self.rng;
        self.cur = (0..n * n).map(|_| (rng.f32() < 0.3) as u8).collect();
        self.next = vec![0; n * n];
        self.heat = vec![0; n * n];
        self.generation = 0;
        self.view = None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut size = self.n;
        ui.horizontal(|ui| {
            ui.label("Grid");
            egui::ComboBox::from_id_salt("life-size")
                .selected_text(format!("{0}×{0}", self.n))
                .show_ui(ui, |ui| {
                    for n in [1024, 2048, 4096, 8192] {
                        ui.selectable_value(&mut size, n, format!("{n}×{n}"));
                    }
                });
            ui.separator();
            ui.label("Generations / frame");
            ui.add(egui::Slider::new(&mut self.gens_per_frame, 1..=64));
            ui.separator();
            if ui.button(if self.paused { "Play" } else { "Pause" }).clicked() {
                self.paused = !self.paused;
            }
            if ui.button("Randomize").clicked() {
                size = 0;
            }
            if ui.button("Fit").clicked() {
                self.view = None;
            }
            ui.label(egui::RichText::new("Scroll to zoom · drag to pan").color(canvas::DIM));
        });
        if size != self.n {
            self.reset(if size == 0 { self.n } else { size });
        }
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let (w, h) = canvas::pixel_size(&ctx, rect, 1.0, 2560);
        self.canvas.resize(w, h);
        let n = self.n;

        // ---- Simulation ----
        if !self.paused {
            let start = Instant::now();
            for _ in 0..self.gens_per_frame {
                step(&self.cur, &mut self.next, n);
                std::mem::swap(&mut self.cur, &mut self.next);
            }
            self.step_time = start.elapsed() / self.gens_per_frame as u32;
            self.generation += self.gens_per_frame as u64;
        }

        // ---- View: zoom around the cursor, drag to pan ----
        let ppp = ctx.pixels_per_point() as f64;
        let fit = (n as f64 / w as f64).max(n as f64 / h as f64);
        let (mut cx, mut cy, mut cpp) = self.view.unwrap_or((n as f64 / 2.0, n as f64 / 2.0, fit));
        if resp.dragged() {
            let d = resp.drag_delta();
            cx -= d.x as f64 * ppp * cpp;
            cy -= d.y as f64 * ppp * cpp;
            self.view = Some((cx, cy, cpp));
        }
        if let Some(pos) = resp.hover_pos() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y) as f64;
            if scroll != 0.0 {
                let next = (cpp * (-scroll * 0.003).exp()).clamp(1.0 / 32.0, fit * 1.5);
                let fx = (pos.x - rect.center().x) as f64 * ppp;
                let fy = (pos.y - rect.center().y) as f64 * ppp;
                cx += fx * (cpp - next);
                cy += fy * (cpp - next);
                cpp = next;
                self.view = Some((cx, cy, cpp));
            }
        }

        // ---- Draw: fade a heat trail, then sample the visible cells ----
        self.heat.par_iter_mut().zip(self.cur.par_iter()).with_min_len(1 << 14).for_each(|(h, &c)| {
            *h = if c == 1 { 255 } else { h.saturating_sub(*h / 10 + 2) };
        });
        let heat = &self.heat;
        let lut = &self.lut;
        let k = cpp.ceil().max(1.0) as usize; // cells covered by one pixel, per axis
        let x0 = cx - w as f64 / 2.0 * cpp;
        let y0 = cy - h as f64 / 2.0 * cpp;
        self.canvas.rgba.par_chunks_mut(w * 4).enumerate().for_each(|(py, row)| {
            let gy = (y0 + py as f64 * cpp).floor();
            for (px, out) in row.chunks_exact_mut(4).enumerate() {
                let gx = (x0 + px as f64 * cpp).floor();
                if gx < 0.0 || gy < 0.0 || gx >= n as f64 || gy >= n as f64 {
                    out.copy_from_slice(&[12, 10, 13, 255]);
                    continue;
                }
                let (gx, gy) = (gx as usize, gy as usize);
                // When zoomed out, show the brightest cell under the pixel.
                let mut v = 0u8;
                for y in gy..(gy + k).min(n) {
                    for &hv in &heat[y * n + gx..y * n + (gx + k).min(n)] {
                        v = v.max(hv);
                    }
                }
                out.copy_from_slice(&lut[v as usize]);
            }
        });

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        let cells = (n * n) as f64;
        let gens_s = if self.paused { 0 } else { self.fps_now * self.gens_per_frame };
        canvas::hud(
            &painter,
            rect,
            &[
                ("grid", format!("{n}×{n}")),
                ("cells", fmt::count(cells)),
                ("generation", fmt::grouped(self.generation)),
                ("one generation", fmt::duration(self.step_time)),
                ("cell updates", fmt::rate(cells, self.step_time)),
                ("generations/s", gens_s.to_string()),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

fn life_lut() -> [[u8; 4]; 256] {
    let mut lut = [[0u8; 4]; 256];
    for (i, c) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *c = if i == 255 {
            [255, 236, 210, 255]
        } else {
            // Dying cells fade from rust orange to near-black.
            let t = t * t;
            [
                (12.0 + 232.0 * t) as u8,
                (10.0 + 96.0 * t) as u8,
                (13.0 + 24.0 * t) as u8,
                255,
            ]
        };
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinker_oscillates() {
        let n = 16;
        let mut a = vec![0u8; n * n];
        for x in 4..7 {
            a[5 * n + x] = 1;
        }
        let mut b = vec![0u8; n * n];
        step(&a, &mut b, n);
        let vertical: Vec<usize> = (0..n * n).filter(|&i| b[i] == 1).collect();
        assert_eq!(vertical, vec![4 * n + 5, 5 * n + 5, 6 * n + 5]);
        let mut c = vec![0u8; n * n];
        step(&b, &mut c, n);
        assert_eq!(a, c);
    }

    #[test]
    fn glider_wraps_around_edges() {
        let n = 8;
        let mut a = vec![0u8; n * n];
        for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
            a[y * n + x] = 1;
        }
        let start = a.clone();
        let mut b = vec![0u8; n * n];
        // A glider returns to its shape shifted (1,1) every 4 generations: 32 gens = full lap.
        for _ in 0..4 * n {
            step(&a, &mut b, n);
            std::mem::swap(&mut a, &mut b);
        }
        assert_eq!(a, start);
    }
}
