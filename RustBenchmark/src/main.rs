#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bench;
mod bench_ui;
mod canvas;
mod demos;
mod fmt;
mod rng;

use eframe::egui::{self, Color32, RichText};

use canvas::{BG, DIM, LINE, PANEL, RUST, TEXT};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Particles,
    Fluid,
    Sand,
    Boids,
    Life,
    Reaction,
    RayTracer,
    SortRace,
    Benchmarks,
}

impl Tab {
    const ALL: [Tab; 9] = [
        Tab::Particles,
        Tab::Fluid,
        Tab::Sand,
        Tab::Boids,
        Tab::Life,
        Tab::Reaction,
        Tab::RayTracer,
        Tab::SortRace,
        Tab::Benchmarks,
    ];

    fn title(self) -> &'static str {
        match self {
            Tab::Particles => "Particle storm",
            Tab::Fluid => "Fluid smoke",
            Tab::Sand => "Falling sand",
            Tab::Boids => "Boids",
            Tab::Life => "Game of Life",
            Tab::Reaction => "Reaction–diffusion",
            Tab::RayTracer => "Ray tracer",
            Tab::SortRace => "Sorting race",
            Tab::Benchmarks => "Benchmarks",
        }
    }

    /// Name for `--tab <name>` on the command line.
    fn arg(self) -> &'static str {
        match self {
            Tab::Particles => "particles",
            Tab::Fluid => "fluid",
            Tab::Sand => "sand",
            Tab::Boids => "boids",
            Tab::Life => "life",
            Tab::Reaction => "reaction",
            Tab::RayTracer => "ray",
            Tab::SortRace => "race",
            Tab::Benchmarks => "bench",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Tab::Particles => {
                "A million particles with real physics, simulated and drawn on the CPU every frame."
            }
            Tab::Life => {
                "Millions of cells, many generations per frame. Every cell is updated every generation."
            }
            Tab::RayTracer => {
                "Every pixel is ray traced on the CPU, with shadows and reflections, every frame. No GPU shaders."
            }
            Tab::Fluid => {
                "Incompressible smoke: advection, vorticity and a pressure solve over every cell, every frame."
            }
            Tab::Sand => "Sand, water, wood, fire and smoke. Every grain is simulated, every frame, on every core.",
            Tab::Boids => {
                "Flocking from three simple rules. A spatial grid turns billions of distance checks into a few million."
            }
            Tab::Reaction => "Two chemicals feeding and killing each other grow coral, cells and mazes.",
            Tab::SortRace => {
                "Six algorithms, the same data, the same number of operations per frame. Then the real thing at full speed."
            }
            Tab::Benchmarks => "Classic CPU workloads, timed on one core and on every core.",
        }
    }
}

struct App {
    tab: Tab,
    particles: demos::particles::Particles,
    fluid: demos::fluid::Fluid,
    sand: demos::sand::Sand,
    boids: demos::boids::Boids,
    life: demos::life::Life,
    reaction: demos::reaction::Reaction,
    raytracer: demos::raytrace::RayTracer,
    race: demos::sort_race::SortRace,
    bench: bench_ui::BenchUi,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        // `rust-benchmark --tab life` opens straight onto a tab.
        let args: Vec<String> = std::env::args().collect();
        let wanted = args.iter().position(|a| a == "--tab").and_then(|i| args.get(i + 1));
        let tab = Tab::ALL
            .into_iter()
            .find(|t| wanted.is_some_and(|w| w == t.arg()))
            .unwrap_or(Tab::Particles);
        let mut bench = bench_ui::BenchUi::new();
        // `--run` starts the benchmark suite immediately.
        if args.iter().any(|a| a == "--run") {
            bench.run_all();
        }
        Self {
            tab,
            particles: demos::particles::Particles::new(),
            fluid: demos::fluid::Fluid::new(),
            sand: demos::sand::Sand::new(),
            boids: demos::boids::Boids::new(),
            life: demos::life::Life::new(),
            reaction: demos::reaction::Reaction::new(),
            raytracer: demos::raytrace::RayTracer::new(),
            race: demos::sort_race::SortRace::new(),
            bench,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(210.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(16.0))
            .show(ui, |ui| self.nav(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(16.0))
            .show(ui, |ui| {
                if cfg!(debug_assertions) {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0x7a, 0x12, 0x12))
                        .corner_radius(6.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(
                                    "DEBUG BUILD: numbers are meaningless. Run `cargo run --release`.",
                                )
                                .color(Color32::WHITE)
                                .strong(),
                            );
                        });
                    ui.add_space(8.0);
                }
                ui.label(RichText::new(self.tab.title()).size(26.0).strong().color(TEXT));
                ui.label(RichText::new(self.tab.blurb()).color(DIM));
                ui.add_space(10.0);

                if self.tab != Tab::Benchmarks && self.bench.busy() {
                    ui.label(
                        RichText::new(
                            "Paused while the benchmark suite runs, so the two don't fight over the CPU.",
                        )
                        .color(DIM),
                    );
                    return;
                }
                match self.tab {
                    Tab::Particles => self.particles.ui(ui),
                    Tab::Fluid => self.fluid.ui(ui),
                    Tab::Sand => self.sand.ui(ui),
                    Tab::Boids => self.boids.ui(ui),
                    Tab::Life => self.life.ui(ui),
                    Tab::Reaction => self.reaction.ui(ui),
                    Tab::RayTracer => self.raytracer.ui(ui),
                    Tab::SortRace => self.race.ui(ui),
                    Tab::Benchmarks => self.bench.ui(ui),
                }
            });
    }
}

impl App {
    fn nav(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("RUST").size(13.0).color(RUST).strong().monospace());
        ui.label(RichText::new("Benchmark").size(24.0).strong().color(TEXT));
        ui.add_space(18.0);

        for tab in Tab::ALL {
            if tab == Tab::Benchmarks {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
            }
            let selected = self.tab == tab;
            let text = RichText::new(tab.title()).size(16.0).color(if selected { RUST } else { TEXT });
            let resp = ui.add_sized([ui.available_width(), 34.0], egui::Button::selectable(selected, text));
            if resp.clicked() {
                self.tab = tab;
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let threads = rayon::current_num_threads();
            let build = if cfg!(debug_assertions) { "debug" } else { "release" };
            ui.label(RichText::new(format!("{build} build")).color(DIM).monospace());
            ui.label(RichText::new(format!("{threads} threads")).color(DIM).monospace());
            ui.label(
                RichText::new(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH))
                    .color(DIM)
                    .monospace(),
            );
        });
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x08, 0x07, 0x09);
    v.faint_bg_color = PANEL;
    v.selection.bg_fill = Color32::from_rgb(0x3a, 0x1c, 0x0e);
    v.selection.stroke = (1.0, RUST).into();
    v.widgets.noninteractive.bg_stroke = (1.0, LINE).into();
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x22, 0x1d, 0x23);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x22, 0x1d, 0x23);
    v.widgets.hovered.bg_stroke = (1.0, RUST).into();
    v.widgets.active.bg_fill = RUST;
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rust Benchmark")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("Rust Benchmark", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
