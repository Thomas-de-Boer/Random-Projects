//! A CPU-side RGBA framebuffer shown as an egui texture, plus the HUD overlay.

use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Painter, Pos2, Rect, StrokeKind, TextureHandle,
    TextureOptions, pos2, vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x0c, 0x0a, 0x0d);
pub const PANEL: Color32 = Color32::from_rgb(0x15, 0x12, 0x16);
pub const LINE: Color32 = Color32::from_rgb(0x2a, 0x24, 0x2b);
pub const TEXT: Color32 = Color32::from_rgb(0xef, 0xe9, 0xe4);
pub const DIM: Color32 = Color32::from_rgb(0x9a, 0x8f, 0x8a);
pub const RUST: Color32 = Color32::from_rgb(0xf4, 0x6a, 0x25);
pub const RUST_SOFT: Color32 = Color32::from_rgb(0xff, 0xb0, 0x7a);

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    tex: Option<TextureHandle>,
    options: TextureOptions,
    name: &'static str,
}

impl Canvas {
    pub fn new(name: &'static str, options: TextureOptions) -> Self {
        Self { w: 0, h: 0, rgba: Vec::new(), tex: None, options, name }
    }

    /// Resizes the buffer (clearing it) if the size changed. Returns true if it did.
    pub fn resize(&mut self, w: usize, h: usize) -> bool {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == (self.w, self.h) {
            return false;
        }
        self.w = w;
        self.h = h;
        self.rgba = vec![0; w * h * 4];
        true
    }

    /// Uploads the buffer to the GPU and paints it stretched over `rect`.
    pub fn paint(&mut self, ctx: &egui::Context, painter: &Painter, rect: Rect, uv: Rect) {
        let image = ColorImage::from_rgba_unmultiplied([self.w, self.h], &self.rgba);
        let tex = match &mut self.tex {
            Some(t) => {
                t.set(image, self.options);
                t
            }
            None => self.tex.insert(ctx.load_texture(self.name, image, self.options)),
        };
        painter.image(tex.id(), rect, uv, Color32::WHITE);
    }
}

/// The physical-pixel size of `rect`, times `scale`, capped at `max_w` wide.
pub fn pixel_size(ctx: &egui::Context, rect: Rect, scale: f32, max_w: usize) -> (usize, usize) {
    let ppp = ctx.pixels_per_point() * scale;
    let mut w = (rect.width() * ppp).round().max(16.0) as usize;
    let mut h = (rect.height() * ppp).round().max(16.0) as usize;
    if w > max_w {
        h = h * max_w / w;
        w = max_w;
    }
    (w, h.max(16))
}

/// Draws a translucent box of `label  value` rows in the top-left of `area`.
pub fn hud(painter: &Painter, area: Rect, rows: &[(&str, String)]) {
    let font = FontId::monospace(13.0);
    let line_h = 18.0;
    let width = 250.0;
    let box_rect = Rect::from_min_size(
        area.min + vec2(12.0, 12.0),
        vec2(width, rows.len() as f32 * line_h + 14.0),
    );
    painter.rect_filled(box_rect, 6.0, Color32::from_rgba_unmultiplied(12, 10, 13, 230));
    painter.rect_stroke(
        box_rect,
        6.0,
        (1.0, Color32::from_white_alpha(20)),
        StrokeKind::Inside,
    );
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = box_rect.min.y + 7.0 + i as f32 * line_h + line_h / 2.0;
        painter.text(pos2(box_rect.min.x + 10.0, y), Align2::LEFT_CENTER, label, font.clone(), DIM);
        painter.text(
            pos2(box_rect.max.x - 10.0, y),
            Align2::RIGHT_CENTER,
            value,
            font.clone(),
            RUST_SOFT,
        );
    }
}

/// Counts frames over a sliding one-second window.
#[derive(Default)]
pub struct FpsCounter {
    times: std::collections::VecDeque<f64>,
}

impl FpsCounter {
    pub fn tick(&mut self, now: f64) -> usize {
        self.times.push_back(now);
        while self.times.front().is_some_and(|&t| now - t > 1.0) {
            self.times.pop_front();
        }
        self.times.len()
    }
}

/// The full 0..1 UV rectangle.
pub fn full_uv() -> Rect {
    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))
}
