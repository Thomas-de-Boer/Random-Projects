//! A real-time CPU ray tracer: reflective spheres, shadows, a checkered floor.

use std::ops::{Add, Mul, Neg, Sub};
use std::time::{Duration, Instant};

use eframe::egui::{self, Sense, TextureOptions};
use rayon::prelude::*;

use crate::canvas::{self, Canvas, FpsCounter};
use crate::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
struct V(f32, f32, f32);

impl V {
    fn dot(self, o: V) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
    fn cross(self, o: V) -> V {
        V(self.1 * o.2 - self.2 * o.1, self.2 * o.0 - self.0 * o.2, self.0 * o.1 - self.1 * o.0)
    }
    fn norm(self) -> V {
        self * (1.0 / self.dot(self).sqrt())
    }
}
impl Add for V {
    type Output = V;
    fn add(self, o: V) -> V {
        V(self.0 + o.0, self.1 + o.1, self.2 + o.2)
    }
}
impl Sub for V {
    type Output = V;
    fn sub(self, o: V) -> V {
        V(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }
}
impl Mul<f32> for V {
    type Output = V;
    fn mul(self, s: f32) -> V {
        V(self.0 * s, self.1 * s, self.2 * s)
    }
}
impl Neg for V {
    type Output = V;
    fn neg(self) -> V {
        V(-self.0, -self.1, -self.2)
    }
}

struct Sphere {
    c: V,
    r: f32,
    color: V,
    reflect: f32,
}

struct Scene {
    spheres: Vec<Sphere>,
    light: V,
}

enum Hit {
    Sphere(usize),
    Floor,
}

const MAX_BOUNCES: u32 = 3;

impl Scene {
    fn at(t: f32) -> Scene {
        let mut spheres = vec![Sphere {
            c: V(0.0, 1.3 + 0.15 * (t * 1.3).sin(), 0.0),
            r: 1.3,
            color: V(0.95, 0.42, 0.15),
            reflect: 0.55,
        }];
        // A ring of smaller spheres bouncing around the big one.
        let ring = 9;
        for i in 0..ring {
            let a = i as f32 / ring as f32 * std::f32::consts::TAU + t * 0.35;
            let bounce = ((t * 2.2 + i as f32 * 0.9).sin()).abs();
            let r = 0.45;
            let hue = i as f32 / ring as f32;
            spheres.push(Sphere {
                c: V(a.cos() * 3.4, r + bounce * 1.2, a.sin() * 3.4),
                r,
                color: V(
                    0.55 + 0.45 * (hue * 6.28).cos(),
                    0.55 + 0.45 * (hue * 6.28 + 2.1).cos(),
                    0.55 + 0.45 * (hue * 6.28 + 4.2).cos(),
                ),
                reflect: if i % 3 == 0 { 0.8 } else { 0.25 },
            });
        }
        Scene { spheres, light: V(-4.0, 9.0, -3.0) }
    }

    fn intersect(&self, o: V, d: V) -> Option<(f32, Hit)> {
        let mut best: Option<(f32, Hit)> = None;
        for (i, s) in self.spheres.iter().enumerate() {
            let oc = o - s.c;
            let b = oc.dot(d);
            let disc = b * b - (oc.dot(oc) - s.r * s.r);
            if disc > 0.0 {
                let t = -b - disc.sqrt();
                if t > 1e-3 && best.as_ref().is_none_or(|(bt, _)| t < *bt) {
                    best = Some((t, Hit::Sphere(i)));
                }
            }
        }
        if d.1 < -1e-4 {
            let t = -o.1 / d.1;
            if t > 1e-3 && best.as_ref().is_none_or(|(bt, _)| t < *bt) {
                best = Some((t, Hit::Floor));
            }
        }
        best
    }

    fn occluded(&self, o: V, d: V, max_t: f32) -> bool {
        self.spheres.iter().any(|s| {
            let oc = o - s.c;
            let b = oc.dot(d);
            let disc = b * b - (oc.dot(oc) - s.r * s.r);
            disc > 0.0 && {
                let t = -b - disc.sqrt();
                t > 1e-3 && t < max_t
            }
        })
    }

    /// Returns the colour seen along a ray, counting every ray cast into `rays`.
    fn trace(&self, mut o: V, mut d: V, rays: &mut u64) -> V {
        let mut color = V(0.0, 0.0, 0.0);
        let mut weight = 1.0;
        for _ in 0..=MAX_BOUNCES {
            *rays += 1;
            let Some((t, hit)) = self.intersect(o, d) else {
                let sky = V(0.05, 0.04, 0.07) + V(0.25, 0.12, 0.08) * (1.0 - d.1.max(0.0)).powi(4);
                return color + sky * weight;
            };
            let p = o + d * t;
            let (n, base, reflect) = match hit {
                Hit::Sphere(i) => {
                    let s = &self.spheres[i];
                    ((p - s.c) * (1.0 / s.r), s.color, s.reflect)
                }
                Hit::Floor => {
                    let check = ((p.0.floor() + p.2.floor()) as i32) & 1 == 0;
                    let fade = (-0.004 * (p.0 * p.0 + p.2 * p.2)).exp();
                    let c = if check { V(0.85, 0.82, 0.8) } else { V(0.12, 0.1, 0.12) };
                    (V(0.0, 1.0, 0.0), c * fade, 0.3)
                }
            };
            let to_light = self.light - p;
            let dist = to_light.dot(to_light).sqrt();
            let l = to_light * (1.0 / dist);
            *rays += 1;
            let lit = !self.occluded(p, l, dist);
            let diffuse = if lit { n.dot(l).max(0.0) } else { 0.0 };
            let r = d - n * (2.0 * d.dot(n));
            let spec = if lit { r.dot(l).max(0.0).powi(40) } else { 0.0 };
            let local = base * (0.12 + 0.88 * diffuse) + V(1.0, 0.9, 0.8) * spec;
            color = color + local * (weight * (1.0 - reflect));
            weight *= reflect;
            if weight < 0.02 {
                break;
            }
            o = p;
            d = r;
        }
        color
    }
}

pub struct RayTracer {
    canvas: Canvas,
    scale: f32,
    time: f32,
    yaw: f32,
    pitch: f32,
    auto_orbit: bool,
    fps: FpsCounter,
    fps_now: usize,
    render: Duration,
    rays: u64,
}

impl RayTracer {
    pub fn new() -> Self {
        Self {
            canvas: Canvas::new("raytrace", TextureOptions::LINEAR),
            scale: 0.75,
            time: 0.0,
            yaw: 0.6,
            pitch: 0.32,
            auto_orbit: true,
            fps: FpsCounter::default(),
            fps_now: 0,
            render: Duration::ZERO,
            rays: 0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Resolution");
            ui.add(
                egui::Slider::new(&mut self.scale, 0.25..=1.0)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            ui.checkbox(&mut self.auto_orbit, "Orbit camera");
            ui.label(egui::RichText::new("Drag to look around").color(canvas::DIM));
        });
        ui.add_space(6.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
        let ctx = ui.ctx().clone();
        let (w, h) = canvas::pixel_size(&ctx, rect, self.scale, 2560);
        self.canvas.resize(w, h);

        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        self.time += dt;
        if resp.dragged() {
            let d = resp.drag_delta();
            self.yaw -= d.x * 0.01;
            self.pitch = (self.pitch + d.y * 0.01).clamp(0.05, 1.3);
        } else if self.auto_orbit {
            self.yaw += dt * 0.25;
        }

        let scene = Scene::at(self.time);
        let dist = 9.5;
        let eye = V(
            self.yaw.cos() * self.pitch.cos() * dist,
            1.0 + self.pitch.sin() * dist,
            self.yaw.sin() * self.pitch.cos() * dist,
        );
        let target = V(0.0, 1.0, 0.0);
        let fwd = (target - eye).norm();
        let right = fwd.cross(V(0.0, 1.0, 0.0)).norm();
        let up = right.cross(fwd);
        let fov = 0.9f32;
        let aspect = w as f32 / h as f32;
        let tan = (fov / 2.0).tan();

        let start = Instant::now();
        self.rays = self
            .canvas
            .rgba
            .par_chunks_mut(w * 4)
            .enumerate()
            .map(|(y, row)| {
                let mut rays = 0u64;
                let sy = (1.0 - 2.0 * (y as f32 + 0.5) / h as f32) * tan;
                for (x, px) in row.chunks_exact_mut(4).enumerate() {
                    let sx = (2.0 * (x as f32 + 0.5) / w as f32 - 1.0) * tan * aspect;
                    let d = (fwd + right * sx + up * sy).norm();
                    let c = scene.trace(eye, d, &mut rays);
                    // Cheap filmic-ish tone curve + gamma.
                    let tone = |v: f32| ((v / (1.0 + v)).sqrt() * 300.0).min(255.0) as u8;
                    px.copy_from_slice(&[tone(c.0), tone(c.1), tone(c.2), 255]);
                }
                rays
            })
            .sum();
        self.render = start.elapsed();

        let painter = ui.painter_at(rect);
        self.canvas.paint(&ctx, &painter, rect, canvas::full_uv());
        self.fps_now = self.fps.tick(ctx.input(|i| i.time));

        canvas::hud(
            &painter,
            rect,
            &[
                ("resolution", format!("{w}×{h}")),
                ("pixels", fmt::count((w * h) as f64)),
                ("rays this frame", fmt::count(self.rays as f64)),
                ("frame render", fmt::duration(self.render)),
                ("rays", fmt::rate(self.rays as f64, self.render)),
                ("threads", rayon::current_num_threads().to_string()),
                ("fps", self.fps_now.to_string()),
            ],
        );
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_big_sphere_and_floor() {
        let scene = Scene::at(0.0);
        let (t, hit) = scene.intersect(V(0.0, 1.3, -10.0), V(0.0, 0.0, 1.0)).unwrap();
        assert!(matches!(hit, Hit::Sphere(0)));
        assert!((t - 8.7).abs() < 0.2);
        let (_, hit) = scene.intersect(V(20.0, 1.0, 20.0), V(0.0, -1.0, 0.0)).unwrap();
        assert!(matches!(hit, Hit::Floor));
    }
}
