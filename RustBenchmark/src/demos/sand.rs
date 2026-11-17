//! Falling sand: sand, water, stone, wood, fire and smoke, updated in parallel
//! with a Margolus neighbourhood (non-overlapping 2×2 blocks, alternating offsets).

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;
use crate::rng::Rng;

pub const EMPTY: u8 = 0;
pub const SAND: u8 = 1;
pub const WATER: u8 = 2;
pub const STONE: u8 = 3;
pub const WOOD: u8 = 4;
pub const FIRE: u8 = 5;
pub const SMOKE: u8 = 6;

/// A cell is one byte: material in the low 3 bits, a 5-bit "aux" value on top
/// (colour variation for solids and liquids, remaining life for fire and smoke).
#[inline]
fn mat(c: u8) -> u8 {
    c & 7
}
#[inline]
fn aux(c: u8) -> u8 {
    c >> 3
}
#[inline]
fn cell(m: u8, aux: u8) -> u8 {
    m | (aux.min(31) << 3)
}

#[inline]
fn density(m: u8) -> u8 {
    match m {
        FIRE | SMOKE => 0,
        EMPTY => 1,
        WATER => 2,
        SAND => 3,
        _ => u8::MAX, // stone and wood never move
    }
}

#[inline]
fn movable(m: u8) -> bool {
    !matches!(m, STONE | WOOD)
}

#[inline]
fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Applies all the rules to one 2×2 block: [top-left, top-right, bottom-left, bottom-right].
fn block(c: &mut [u8; 4], mut r: u32) {
    let mut bit = || {
        r = r.rotate_right(3) ^ 0x5bd1_e995;
        r
    };
    let ms = c.map(mat);
    if ms.iter().all(|&m| m == EMPTY) {
        return;
    }

    // ---- Fire and smoke ----
    let has_fire = ms.contains(&FIRE);
    let has_water = ms.contains(&WATER);
    for i in 0..4 {
        match mat(c[i]) {
            FIRE => {
                let life = aux(c[i]);
                if has_water {
                    c[i] = cell(SMOKE, 14);
                } else if bit() % 3 == 0 {
                    c[i] = if life <= 1 { cell(SMOKE, 18) } else { cell(FIRE, life - 1) };
                }
            }
            WOOD if has_fire && bit() % 10 == 0 => c[i] = cell(FIRE, 31),
            SMOKE if bit() % 4 == 0 => {
                let life = aux(c[i]);
                c[i] = if life <= 1 { EMPTY } else { cell(SMOKE, life - 1) };
            }
            _ => {}
        }
    }

    // ---- Gravity (and buoyancy): heavier above lighter swaps ----
    let mut moved = [false; 2];
    for col in 0..2 {
        let (t, b) = (col, col + 2);
        let (mt, mb) = (mat(c[t]), mat(c[b]));
        if movable(mt) && movable(mb) && density(mt) > density(mb) {
            // Sinking through water and rising gas are a little lazy, which looks right.
            let slow = mb == WATER || mt == EMPTY;
            if !slow || bit() % 2 == 0 {
                c.swap(t, b);
                moved[col] = true;
            }
        }
    }

    // ---- Sliding diagonally down off piles ----
    let order = if bit() % 2 == 0 { [(0, 3, 1), (1, 2, 0)] } else { [(1, 2, 0), (0, 3, 1)] };
    for (top, diag, beside) in order {
        if moved[top] {
            continue;
        }
        let (mt, md, mb) = (mat(c[top]), mat(c[diag]), mat(c[beside]));
        let below = mat(c[top + 2]);
        if matches!(mt, SAND | WATER)
            && movable(md)
            && density(mt) > density(md)
            && density(below) >= density(mt) // resting on something
            && density(mb) < density(mt) // and the way to the side is open
        {
            c.swap(top, diag);
            moved[top] = true;
        }
    }

    // ---- Liquids and gases spread sideways ----
    for (l, rr) in [(2, 3), (0, 1)] {
        let (ml, mr) = (mat(c[l]), mat(c[rr]));
        let lighter = |m: u8| movable(m) && density(m) < density(WATER);
        if ml == WATER || mr == WATER {
            // Water keeps flowing the way it was going (aux bit 0: 1 = right) and
            // turns around when blocked, so it levels out instead of random-walking.
            if ml == WATER && aux(c[l]) & 1 == 1 {
                if lighter(mr) {
                    c.swap(l, rr);
                } else {
                    c[l] ^= 1 << 3;
                }
            } else if mr == WATER && aux(c[rr]) & 1 == 0 {
                if lighter(ml) {
                    c.swap(l, rr);
                } else {
                    c[rr] ^= 1 << 3;
                }
            }
        } else {
            let gas = |m: u8| matches!(m, SMOKE | FIRE | EMPTY);
            if ml != mr && gas(ml) && gas(mr) && bit() % 2 == 0 {
                c.swap(l, rr);
            }
        }
    }
}

/// One Margolus pass over the whole grid with block offset (ox, oy).
pub fn pass(grid: &mut [u8], w: usize, ox: usize, oy: usize, seed: u32) {
    let rows = &mut grid[oy * w..];
    rows.par_chunks_mut(2 * w).enumerate().for_each(|(by, pair)| {
        if pair.len() < 2 * w {
            return;
        }
        let (top, bot) = pair.split_at_mut(w);
        let y = (oy + by * 2) as u32;
        let mut x = ox;
        while x + 1 < w {
            if (top[x] | top[x + 1] | bot[x] | bot[x + 1]) != 0 {
                let mut c = [top[x], top[x + 1], bot[x], bot[x + 1]];
                block(&mut c, hash(x as u32, y, seed));
                top[x] = c[0];
                top[x + 1] = c[1];
                bot[x] = c[2];
                bot[x + 1] = c[3];
            }
            x += 2;
        }
    });
}

const OFFSETS: [(usize, usize); 4] = [(0, 0), (1, 1), (1, 0), (0, 1)];

pub struct Sand {
    w: usize,
    h: usize,
    grid: Vec<u8>,
    canvas: Canvas,
    lut: [[u8; 4]; 256],
    rng: Rng,
    brush: u8,
    radius: f32,
    rain: bool,
    frame: u32,
    passes: usize,
    step_time: Duration,
    grains: usize,
    fps: FpsCounter,
    fps_now: usize,
}

impl Sand {
    pub fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            grid: vec![],
            canvas: Canvas::new("sand", TextureOptions::NEAREST),
            lut: palette(),
            rng: Rng::new(5),
            brush: SAND,
            radius: 6.0,
            rain: true,
            frame: 0,
            passes: 4,
            step_time: Duration::ZERO,
            grains: 0,
            fps: FpsCounter::default(),
            fps_now: 0,
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        let fresh = self.grid.is_empty();
        let mut g = vec![EMPTY; w * h];
        // Keep what's there, anchored to the bottom.
        for y in 0..h.min(self.h) {
            let (src, dst) = ((self.h - 1 - y) * self.w, (h - 1 - y) * w);
            let n = w.min(self.w);
            g[dst..dst + n].copy_from_slice(&self.grid[src..src + n]);
        }
        self.grid = g;
        self.w = w;
        self.h = h;
        if fresh {
            self.scene();
        }
    }

    fn put(&mut self, x: i64, y: i64, m: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let v = match m {
                FIRE => cell(FIRE, 16 + (self.rng.next_u64() % 16) as u8),
                EMPTY => EMPTY,
                _ => cell(m, (self.rng.next_u64() % 32) as u8),
            };
            self.grid[y as usize * self.w + x as usize] = v;
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, m: u8, density: f32) {
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dy * dy) as f32) <= r * r && self.rng.f32() < density {
                    self.put(cx as i64 + dx, cy as i64 + dy, m);
                }
            }
        }
    }

    /// Stone ramps and a wooden hut, so there's something to pour onto.
    fn scene(&mut self) {
        let (w, h) = (self.w as f32, self.h as f32);
        let thick = (h / 90.0).max(2.0) as i64;
        let ramp = |x0: f32, y0: f32, x1: f32, y1: f32, s: &mut Self| {
            let steps = ((x1 - x0).abs().max((y1 - y0).abs())) as i64;
            for i in 0..=steps {
                let t = i as f32 / steps.max(1) as f32;
                let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
                for k in 0..thick {
                    s.put(x as i64, y as i64 + k, STONE);
                }
            }
        };
        ramp(w * 0.05, h * 0.35, w * 0.4, h * 0.5, self);
        ramp(w * 0.95, h * 0.55, w * 0.6, h * 0.68, self);
        ramp(w * 0.2, h * 0.8, w * 0.45, h * 0.8, self);
        let (bx, by, bw, bh) = (w * 0.62, h * 0.82, w * 0.12, h * 0.17);
        for y in by as i64..(by + bh) as i64 {
            for x in bx as i64..(bx + bw) as i64 {
                self.put(x, y, WOOD);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut clear = false;
        ui.horizontal(|ui| {
            for (m, name) in [
                (SAND, "Sand"),
                (WATER, "Water"),
                (STONE, "Stone"),
                (WOOD, "Wood"),
                (FIRE, "Fire"),
                (EMPTY, "Eraser"),
            ] {
                let c = self.lut[cell(m, 16) as usize];
                let swatch = RichText::new("■ ").color(Color32::from_rgb(c[0], c[1], c[2]));
                let text = RichText::new(name);
                let mut job = egui::text::LayoutJob::default();
                swatch.append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                text.append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                if ui.selectable_label(self.brush == m, job).clicked() {
                    self.brush = m;
                }
            }
            ui.separator();
            ui.label("Brush");
            ui.add(egui::Slider::new(&mut self.radius, 1.0..=30.0).integer());
            ui.checkbox(&mut self.rain, "Rain");
            clear = ui.button("Clear").clicked();
            ui.label(RichText::new("Left-drag paints · right-drag erases").color(canvas::DIM));
        });
        if clear {
            self.grid.fill(EMPTY);
        }
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let (pw, ph) = canvas::pixel_size(&ctx, rect, 1.0, 4096);
        let (w, h) = ((pw / 2).max(32), (ph / 2).max(32));
        if (w, h) != (self.w, self.h) {
            self.resize(w, h);
        }

        // ---- Input ----
        if let Some(pos) = resp.hover_pos() {
            let (primary, secondary) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
            if primary || secondary {
                let x = (pos.x - rect.min.x) / rect.width() * w as f32;
                let y = (pos.y - rect.min.y) / rect.height() * h as f32;
                let m = if secondary { EMPTY } else { self.brush };
                let dens = if matches!(m, SAND | WATER | FIRE) { 0.5 } else { 1.0 };
                self.disc(x, y, self.radius, m, dens);
            }
        }
        if self.rain {
            let t = self.frame as f32 * 0.01;
            let (wf, hf) = (w as f32, h as f32);
            self.disc(wf * (0.25 + 0.1 * t.sin()), hf * 0.03, wf * 0.01, SAND, 0.4);
            self.disc(wf * 0.8, hf * 0.03, wf * 0.008, WATER, 0.5);
            if self.frame % 240 == 0 {
                self.disc(wf * 0.68, hf * 0.8, 3.0, FIRE, 1.0);
            }
        }

        // ---- Simulation ----
        let start = Instant::now();
        for k in 0..self.passes {
            let (ox, oy) = OFFSETS[(self.frame as usize * self.passes + k) % 4];
            pass(&mut self.grid, w, ox, oy, self.frame.wrapping_mul(7919) ^ k as u32);
        }
        self.step_time = start.elapsed();
        self.frame = self.frame.wrapping_add(1);

        // ---- Draw ----
        self.canvas.resize(w, h);
        let (grid, lut) = (&self.grid, &self.lut);
        self.grains = self
            .canvas
            .rgba
            .par_chunks_mut(w * 4)
            .zip(grid.par_chunks(w))
            .map(|(row, cells)| {
                let mut n = 0;
                for (px, &c) in row.chunks_exact_mut(4).zip(cells) {
                    px.copy_from_slice(&lut[c as usize]);
                    n += matches!(mat(c), SAND | WATER) as usize;
                }
                n
            })
            .sum();

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        let cells = (w * h) as f64;
        canvas::hud(
            &painter,
            rect,
            &[
                ("grid", format!("{w}×{h}")),
                ("grains", fmt::grouped(self.grains as u64)),
                ("passes / frame", self.passes.to_string()),
                ("simulation", fmt::duration(self.step_time)),
                ("cell updates", fmt::rate(cells * self.passes as f64, self.step_time)),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

/// Colour for every possible cell byte.
fn palette() -> [[u8; 4]; 256] {
    let mut lut = [[0u8; 4]; 256];
    for (i, out) in lut.iter_mut().enumerate() {
        let (m, a) = (mat(i as u8), aux(i as u8) as f32 / 31.0);
        let jitter = 0.88 + 0.24 * a;
        let rgb: [f32; 3] = match m {
            SAND => [0.88 * jitter, 0.70 * jitter, 0.42 * jitter],
            WATER => [0.12, 0.33 * jitter, 0.78 * jitter],
            STONE => [0.40 * jitter, 0.38 * jitter, 0.40 * jitter],
            WOOD => [0.45 * jitter, 0.27 * jitter, 0.13 * jitter],
            FIRE => [1.0, 0.25 + 0.6 * a, 0.05 + 0.25 * a * a],
            SMOKE => {
                let k = 0.06 + 0.22 * a;
                [k, k * 0.95, k]
            }
            _ => [0.047, 0.04, 0.05],
        };
        *out = [
            (rgb[0].min(1.0) * 255.0) as u8,
            (rgb[1].min(1.0) * 255.0) as u8,
            (rgb[2].min(1.0) * 255.0) as u8,
            255,
        ];
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(g: &[u8], m: u8) -> usize {
        g.iter().filter(|&&c| mat(c) == m).count()
    }

    #[test]
    fn grains_are_conserved_and_sand_sinks_below_water() {
        let (w, h) = (64, 64);
        let mut rng = Rng::new(3);
        // Water at the bottom, sand on top: the sand must end up underneath.
        let mut g: Vec<u8> = (0..w * h)
            .map(|i| {
                let y = i / w;
                let r = rng.f32();
                if y >= 40 && r < 0.8 {
                    cell(WATER, 3)
                } else if (20..40).contains(&y) && r < 0.8 {
                    cell(SAND, 3)
                } else {
                    EMPTY
                }
            })
            .collect();
        let (sand, water) = (count(&g, SAND), count(&g, WATER));
        for f in 0..4000u32 {
            let (ox, oy) = OFFSETS[f as usize % 4];
            pass(&mut g, w, ox, oy, f);
        }
        assert_eq!(count(&g, SAND), sand);
        assert_eq!(count(&g, WATER), water);
        let mean_y = |m: u8| {
            let ys: Vec<usize> = (0..w * h).filter(|&i| mat(g[i]) == m).map(|i| i / w).collect();
            ys.iter().sum::<usize>() as f64 / ys.len() as f64
        };
        assert!(mean_y(SAND) > mean_y(WATER), "sand {} water {}", mean_y(SAND), mean_y(WATER));
    }

    #[test]
    fn stone_never_moves() {
        let (w, h) = (16, 16);
        let mut g = vec![EMPTY; w * h];
        g[5 * w + 5] = cell(STONE, 0);
        for f in 0..100 {
            let (ox, oy) = OFFSETS[f % 4];
            pass(&mut g, w, ox, oy, f as u32);
        }
        assert_eq!(mat(g[5 * w + 5]), STONE);
    }
}
