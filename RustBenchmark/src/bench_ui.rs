//! The "Benchmarks" tab: run the suite and show single- vs multi-core results.

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, pos2, vec2};

use crate::bench::{self, Bench, Mode, Msg, Outcome, Progress};
use crate::canvas::{DIM, LINE, PANEL, RUST, RUST_SOFT, TEXT};
use crate::fmt;

pub struct BenchUi {
    benches: Vec<Bench>,
    results: Vec<[Option<Outcome>; 2]>,
    running: Option<(usize, Mode)>,
    rx: Option<Receiver<Msg>>,
    progress: Arc<Progress>,
    threads: usize,
}

impl BenchUi {
    pub fn new() -> Self {
        let benches = bench::all();
        Self {
            results: vec![[None, None]; benches.len()],
            benches,
            running: None,
            rx: None,
            progress: Arc::new(Progress::default()),
            threads: rayon::current_num_threads(),
        }
    }

    pub fn run_all(&mut self) {
        self.start((0..self.benches.len()).collect());
    }

    fn start(&mut self, which: Vec<usize>) {
        for &i in &which {
            self.results[i] = [None, None];
        }
        self.rx = Some(bench::spawn_runner(which, self.progress.clone()));
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Started(i, mode) => self.running = Some((i, mode)),
                Msg::Done(i, mode, out) => self.results[i][mode as usize] = Some(out),
                Msg::Finished => {
                    self.running = None;
                    self.rx = None;
                    return;
                }
            }
        }
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if self.busy() {
            ui.ctx().request_repaint();
        }

        ui.horizontal(|ui| {
            let run = ui.add_enabled(
                !self.busy(),
                egui::Button::new(RichText::new("  Run all  ").strong().color(Color32::BLACK))
                    .fill(RUST),
            );
            if run.clicked() {
                self.run_all();
            }
            ui.label(
                RichText::new(format!(
                    "Each test runs twice: on 1 core, then on all {} threads. Only the work itself is timed.",
                    self.threads
                ))
                .color(DIM),
            );
        });
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let mut run_one = None;
            for i in 0..self.benches.len() {
                if self.card(ui, i) {
                    run_one = Some(i);
                }
                ui.add_space(8.0);
            }
            if let Some(i) = run_one {
                self.start(vec![i]);
            }
            self.totals(ui);
        });
    }

    /// One benchmark card. Returns true if its "Run" button was clicked.
    fn card(&mut self, ui: &mut egui::Ui, i: usize) -> bool {
        let b = &self.benches[i];
        let [single, multi] = &self.results[i];
        let mut clicked = false;
        egui::Frame::new()
            .fill(PANEL)
            .stroke((1.0, LINE))
            .corner_radius(10.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(b.name).size(18.0).strong().color(TEXT));
                    if let (Some(s), Some(m)) = (single, multi) {
                        let x = s.elapsed.as_secs_f64() / m.elapsed.as_secs_f64();
                        ui.label(
                            RichText::new(format!("{x:.1}× faster on all cores"))
                                .color(RUST)
                                .strong(),
                        );
                        if s.check != m.check {
                            ui.label(RichText::new("results differ!").color(Color32::RED));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        clicked = ui.add_enabled(!self.busy(), egui::Button::new("Run")).clicked();
                    });
                });
                ui.label(RichText::new(b.about).color(DIM));
                ui.add_space(8.0);

                // Bars are scaled to throughput: longer = faster.
                let best = [single, multi]
                    .iter()
                    .filter_map(|o| o.as_ref())
                    .map(|o| o.ops / o.elapsed.as_secs_f64())
                    .fold(0.0, f64::max);
                for (mode, out) in [(Mode::Single, single), (Mode::All, multi)] {
                    let label = match mode {
                        Mode::Single => "1 core".to_string(),
                        Mode::All => format!("{} threads", self.threads),
                    };
                    let live = self.running == Some((i, mode));
                    bar(ui, &label, b.unit, out.as_ref(), best, live.then(|| self.progress.fraction()), mode);
                }
                if let Some(o) = multi.as_ref().or(single.as_ref()) {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&o.detail).color(DIM).monospace());
                }
            });
        clicked
    }

    fn totals(&self, ui: &mut egui::Ui) {
        let done: Vec<_> = self
            .results
            .iter()
            .filter_map(|[s, m]| Some((s.as_ref()?.elapsed, m.as_ref()?.elapsed)))
            .collect();
        if done.is_empty() {
            return;
        }
        let single: Duration = done.iter().map(|d| d.0).sum();
        let multi: Duration = done.iter().map(|d| d.1).sum();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Total").size(18.0).strong());
            ui.label(format!(
                "{} test{}: {} on 1 core, {} on all cores:",
                done.len(),
                if done.len() == 1 { "" } else { "s" },
                fmt::duration(single),
                fmt::duration(multi)
            ));
            ui.label(
                RichText::new(format!("{:.1}× speed-up", single.as_secs_f64() / multi.as_secs_f64()))
                    .color(RUST)
                    .strong(),
            );
        });
        ui.add_space(16.0);
    }
}

fn bar(
    ui: &mut egui::Ui,
    label: &str,
    unit: &str,
    out: Option<&Outcome>,
    best: f64,
    live: Option<f32>,
    mode: Mode,
) {
    let height = 26.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    let label_w = 90.0;
    p.text(
        pos2(rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::monospace(13.0),
        DIM,
    );
    let track = egui::Rect::from_min_max(pos2(rect.min.x + label_w, rect.min.y + 2.0), rect.max - vec2(0.0, 2.0));
    p.rect_filled(track, 5.0, LINE);
    let fill = if mode == Mode::All { RUST } else { RUST_SOFT.gamma_multiply(0.55) };

    if let Some(f) = live {
        let mut r = track;
        r.set_width(track.width() * f.max(0.01));
        p.rect_filled(r, 5.0, fill.gamma_multiply(0.6));
        p.text(
            pos2(track.min.x + 10.0, track.center().y),
            Align2::LEFT_CENTER,
            format!("running… {:.0}%", f * 100.0),
            FontId::proportional(13.0),
            TEXT,
        );
        return;
    }
    let Some(o) = out else {
        p.text(
            pos2(track.min.x + 10.0, track.center().y),
            Align2::LEFT_CENTER,
            "–",
            FontId::proportional(13.0),
            DIM,
        );
        return;
    };
    let rate = o.ops / o.elapsed.as_secs_f64();
    let mut r = track;
    r.set_width((track.width() * (rate / best) as f32).max(4.0));
    p.rect_filled(r, 5.0, fill);
    let text = format!("{}  ·  {}", fmt::throughput(rate, unit), fmt::duration(o.elapsed));
    let galley = p.layout_no_wrap(text.clone(), FontId::proportional(13.0), Color32::BLACK);
    // Put the text inside the bar if it fits, otherwise just after it.
    let (pos, color) = if galley.size().x + 20.0 < r.width() {
        (pos2(r.min.x + 10.0, r.center().y), Color32::from_rgb(0x1a, 0x0b, 0x02))
    } else {
        (pos2(r.max.x + 8.0, r.center().y), TEXT)
    };
    p.text(pos, Align2::LEFT_CENTER, text, FontId::proportional(13.0), color);
}
