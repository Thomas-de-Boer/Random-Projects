//! Six sorting algorithms race on identical data, one operation at a time, then
//! run again at full speed on millions of numbers.
//!
//! Every algorithm is written once against the `Arr` trait. Running it on a
//! `Tracer` records each compare/swap/read/write for the animation; running it on
//! `Plain` compiles down to ordinary slice code for the full-speed timings.

use std::cmp::Ordering;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, pos2, vec2};
use rayon::prelude::*;

use crate::canvas::{BG, DIM, LINE, PANEL, RUST, RUST_SOFT, TEXT};
use crate::fmt;
use crate::rng::Rng;

pub trait Arr {
    fn len(&self) -> usize;
    fn get(&mut self, i: usize) -> u32;
    fn set(&mut self, i: usize, v: u32);
    fn swap(&mut self, i: usize, j: usize);
    /// a[i] < a[j]
    fn less(&mut self, i: usize, j: usize) -> bool;
    /// a[i] compared with a value
    fn cmp_v(&mut self, i: usize, v: u32) -> Ordering;
    /// A comparison between two values held outside the array.
    fn tick(&mut self);
}

pub struct Plain<'a>(pub &'a mut [u32]);

impl Arr for Plain<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn get(&mut self, i: usize) -> u32 {
        self.0[i]
    }
    #[inline]
    fn set(&mut self, i: usize, v: u32) {
        self.0[i] = v;
    }
    #[inline]
    fn swap(&mut self, i: usize, j: usize) {
        self.0.swap(i, j);
    }
    #[inline]
    fn less(&mut self, i: usize, j: usize) -> bool {
        self.0[i] < self.0[j]
    }
    #[inline]
    fn cmp_v(&mut self, i: usize, v: u32) -> Ordering {
        self.0[i].cmp(&v)
    }
    #[inline]
    fn tick(&mut self) {}
}

#[derive(Clone, Copy, Debug)]
pub enum Op {
    Read(u32),
    Write(u32, u32),
    Cmp(u32, u32),
    CmpV(u32),
    Swap(u32, u32),
    Tick,
}

pub struct Tracer {
    pub data: Vec<u32>,
    pub ops: Vec<Op>,
}

impl Arr for Tracer {
    fn len(&self) -> usize {
        self.data.len()
    }
    fn get(&mut self, i: usize) -> u32 {
        self.ops.push(Op::Read(i as u32));
        self.data[i]
    }
    fn set(&mut self, i: usize, v: u32) {
        self.ops.push(Op::Write(i as u32, v));
        self.data[i] = v;
    }
    fn swap(&mut self, i: usize, j: usize) {
        self.ops.push(Op::Swap(i as u32, j as u32));
        self.data.swap(i, j);
    }
    fn less(&mut self, i: usize, j: usize) -> bool {
        self.ops.push(Op::Cmp(i as u32, j as u32));
        self.data[i] < self.data[j]
    }
    fn cmp_v(&mut self, i: usize, v: u32) -> Ordering {
        self.ops.push(Op::CmpV(i as u32));
        self.data[i].cmp(&v)
    }
    fn tick(&mut self) {
        self.ops.push(Op::Tick);
    }
}

// ---------------------------------------------------------------- algorithms

pub fn quick_sort<A: Arr>(a: &mut A) {
    if a.len() > 1 {
        quick(a, 0, a.len() - 1);
    }
}

fn quick<A: Arr>(a: &mut A, mut lo: usize, mut hi: usize) {
    while lo < hi {
        // Median of three keeps sorted and reversed input fast.
        let mid = lo + (hi - lo) / 2;
        if a.less(mid, lo) {
            a.swap(mid, lo);
        }
        if a.less(hi, lo) {
            a.swap(hi, lo);
        }
        if a.less(hi, mid) {
            a.swap(hi, mid);
        }
        let p = a.get(mid);
        // Hoare partition.
        let (mut i, mut j) = (lo as isize - 1, hi as isize + 1);
        let split = loop {
            loop {
                i += 1;
                if a.cmp_v(i as usize, p) != Ordering::Less {
                    break;
                }
            }
            loop {
                j -= 1;
                if a.cmp_v(j as usize, p) != Ordering::Greater {
                    break;
                }
            }
            if i >= j {
                break j as usize;
            }
            a.swap(i as usize, j as usize);
        };
        // Recurse into the smaller half, loop on the larger: O(log n) stack.
        if split - lo < hi - split {
            quick(a, lo, split);
            lo = split + 1;
        } else {
            quick(a, split + 1, hi);
            hi = split;
        }
    }
}

pub fn merge_sort<A: Arr>(a: &mut A) {
    let n = a.len();
    let mut left = Vec::with_capacity(n / 2 + 1);
    let mut width = 1;
    while width < n {
        let mut lo = 0;
        while lo + width < n {
            let mid = lo + width;
            let hi = (mid + width).min(n);
            // Copy the left run out, then merge it with the right run in place.
            left.clear();
            for i in lo..mid {
                left.push(a.get(i));
            }
            let (mut i, mut j, mut k) = (0, mid, lo);
            while i < left.len() && j < hi {
                let r = a.get(j);
                a.tick();
                if r < left[i] {
                    a.set(k, r);
                    j += 1;
                } else {
                    a.set(k, left[i]);
                    i += 1;
                }
                k += 1;
            }
            while i < left.len() {
                a.set(k, left[i]);
                i += 1;
                k += 1;
            }
            lo = hi;
        }
        width *= 2;
    }
}

pub fn heap_sort<A: Arr>(a: &mut A) {
    let n = a.len();
    let sift = |a: &mut A, mut root: usize, end: usize| loop {
        let mut child = 2 * root + 1;
        if child >= end {
            return;
        }
        if child + 1 < end && a.less(child, child + 1) {
            child += 1;
        }
        if !a.less(root, child) {
            return;
        }
        a.swap(root, child);
        root = child;
    };
    for i in (0..n / 2).rev() {
        sift(a, i, n);
    }
    for end in (1..n).rev() {
        a.swap(0, end);
        sift(a, 0, end);
    }
}

pub fn radix_sort<A: Arr>(a: &mut A) {
    let n = a.len();
    let mut vals: Vec<u32> = (0..n).map(|i| a.get(i)).collect();
    let max = vals.iter().copied().max().unwrap_or(0);
    let mut out = vec![0u32; n];
    let mut shift = 0;
    // Least-significant byte first, a stable counting sort per byte.
    while shift < 32 && (max >> shift) > 0 {
        let mut counts = [0usize; 257];
        for &v in &vals {
            counts[((v >> shift) & 255) as usize + 1] += 1;
        }
        for d in 0..256 {
            counts[d + 1] += counts[d];
        }
        for &v in &vals {
            let d = ((v >> shift) & 255) as usize;
            out[counts[d]] = v;
            a.set(counts[d], v);
            counts[d] += 1;
        }
        std::mem::swap(&mut vals, &mut out);
        shift += 8;
    }
}

pub fn shell_sort<A: Arr>(a: &mut A) {
    let n = a.len();
    // Ciura's gaps, extended by ×2.25 for big inputs.
    let mut gaps = vec![1usize, 4, 10, 23, 57, 132, 301, 701];
    while *gaps.last().unwrap() < n / 2 {
        let g = (*gaps.last().unwrap() as f64 * 2.25) as usize;
        gaps.push(g);
    }
    for &gap in gaps.iter().rev() {
        for i in gap..n {
            let mut j = i;
            while j >= gap && a.less(j, j - gap) {
                a.swap(j, j - gap);
                j -= gap;
            }
        }
    }
}

pub fn insertion_sort<A: Arr>(a: &mut A) {
    for i in 1..a.len() {
        let mut j = i;
        while j > 0 && a.less(j, j - 1) {
            a.swap(j, j - 1);
            j -= 1;
        }
    }
}

type SortFn = fn(&mut Tracer);
type PlainFn = fn(&mut [u32]);

const ALGOS: [(&str, SortFn, PlainFn); 6] = [
    ("Quicksort", quick_sort::<Tracer>, |v| quick_sort(&mut Plain(v))),
    ("Merge sort", merge_sort::<Tracer>, |v| merge_sort(&mut Plain(v))),
    ("Heapsort", heap_sort::<Tracer>, |v| heap_sort(&mut Plain(v))),
    ("Radix sort (LSD)", radix_sort::<Tracer>, |v| radix_sort(&mut Plain(v))),
    ("Shell sort", shell_sort::<Tracer>, |v| shell_sort(&mut Plain(v))),
    ("Insertion sort", insertion_sort::<Tracer>, |v| insertion_sort(&mut Plain(v))),
];

// ---------------------------------------------------------------- the demo

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Random,
    NearlySorted,
    Reversed,
    FewUnique,
}

impl Input {
    const ALL: [(Input, &'static str); 4] = [
        (Input::Random, "Random"),
        (Input::NearlySorted, "Nearly sorted"),
        (Input::Reversed, "Reversed"),
        (Input::FewUnique, "Few unique"),
    ];

    fn make(self, n: usize, rng: &mut Rng) -> Vec<u32> {
        let mut v: Vec<u32> = (1..=n as u32).collect();
        let shuffle = |v: &mut Vec<u32>, rng: &mut Rng| {
            for i in (1..v.len()).rev() {
                v.swap(i, (rng.next_u64() % (i as u64 + 1)) as usize);
            }
        };
        match self {
            Input::Random => shuffle(&mut v, rng),
            Input::NearlySorted => {
                for _ in 0..n / 20 {
                    let i = (rng.next_u64() % n as u64) as usize;
                    let j = (i + 1 + (rng.next_u64() % 8) as usize).min(n - 1);
                    v.swap(i, j);
                }
            }
            Input::Reversed => v.reverse(),
            Input::FewUnique => {
                for x in &mut v {
                    *x = ((*x - 1) * 5 / n as u32 + 1) * n as u32 / 5;
                }
                shuffle(&mut v, rng);
            }
        }
        v
    }
}

struct Lane {
    name: &'static str,
    data: Vec<u32>,
    ops: Vec<Op>,
    pos: usize,
    touched: Vec<u32>,
    rank: Option<usize>,
}

struct FullResult {
    name: String,
    n: usize,
    time: Duration,
}

pub struct SortRace {
    n: usize,
    speed: usize,
    input: Input,
    lanes: Vec<Lane>,
    next_rank: usize,
    rng: Rng,
    paused: bool,
    full: Vec<FullResult>,
    full_rx: Option<Receiver<Option<FullResult>>>,
}

const FULL_N: usize = 10_000_000;
const INSERTION_N: usize = 100_000;

impl SortRace {
    pub fn new() -> Self {
        let mut s = Self {
            n: 300,
            speed: 25,
            input: Input::Random,
            lanes: vec![],
            next_rank: 0,
            rng: Rng::new(99),
            paused: false,
            full: vec![],
            full_rx: None,
        };
        s.restart();
        s
    }

    fn restart(&mut self) {
        let data = self.input.make(self.n, &mut self.rng);
        self.lanes = ALGOS
            .par_iter()
            .map(|&(name, traced, _)| {
                let mut t = Tracer { data: data.clone(), ops: vec![] };
                traced(&mut t);
                Lane { name, data: data.clone(), ops: t.ops, pos: 0, touched: vec![], rank: None }
            })
            .collect();
        self.next_rank = 0;
    }

    fn start_full_speed(&mut self) {
        let (tx, rx) = channel();
        self.full.clear();
        self.full_rx = Some(rx);
        std::thread::spawn(move || {
            let random = crate::bench::sort::random_data(FULL_N, 7);
            let data: Vec<u32> = random.iter().map(|&v| (v >> 32) as u32).collect();
            let mut expect = data.clone();
            expect.par_sort_unstable();
            let run = |name: String, n: usize, f: &dyn Fn(&mut [u32])| {
                let mut v = data[..n].to_vec();
                let start = Instant::now();
                f(&mut v);
                let time = start.elapsed();
                assert!(v.windows(2).all(|w| w[0] <= w[1]), "{name} failed to sort");
                tx.send(Some(FullResult { name, n, time })).is_ok()
            };
            for &(name, _, plain) in &ALGOS {
                let n = if name.starts_with("Insertion") { INSERTION_N } else { FULL_N };
                if !run(name.into(), n, &|v| plain(v)) {
                    return;
                }
            }
            run("Rust std sort_unstable".into(), FULL_N, &|v| v.sort_unstable());
            run(
                format!("rayon par_sort_unstable ({} threads)", rayon::current_num_threads()),
                FULL_N,
                &|v| v.par_sort_unstable(),
            );
            let _ = tx.send(None);
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let (mut restart, mut full) = (false, false);
        ui.horizontal(|ui| {
            ui.label("Elements");
            restart |= ui.add(egui::Slider::new(&mut self.n, 50..=1000)).drag_stopped();
            ui.label("Ops / frame");
            ui.add(egui::Slider::new(&mut self.speed, 1..=20_000).logarithmic(true));
            egui::ComboBox::from_id_salt("race-input")
                .selected_text(Input::ALL.iter().find(|i| i.0 == self.input).unwrap().1)
                .show_ui(ui, |ui| {
                    for (i, name) in Input::ALL {
                        restart |= ui.selectable_value(&mut self.input, i, name).changed();
                    }
                });
            if ui.button(if self.paused { "Play" } else { "Pause" }).clicked() {
                self.paused = !self.paused;
            }
            restart |= ui.button("Restart").clicked();
            ui.separator();
            full = ui
                .add_enabled(
                    self.full_rx.is_none(),
                    egui::Button::new(RichText::new(" Full speed: 10M numbers ").color(Color32::BLACK)).fill(RUST),
                )
                .clicked();
        });
        if restart {
            self.restart();
        }
        if full {
            self.start_full_speed();
        }
        if let Some(rx) = &self.full_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Some(r) => self.full.push(r),
                    None => {
                        self.full_rx = None;
                        break;
                    }
                }
            }
        }
        ui.add_space(6.0);

        // Advance every lane by the same number of operations.
        if !self.paused {
            for lane in &mut self.lanes {
                lane.touched.clear();
                let end = (lane.pos + self.speed).min(lane.ops.len());
                for op in &lane.ops[lane.pos..end] {
                    match *op {
                        Op::Swap(i, j) => {
                            lane.data.swap(i as usize, j as usize);
                            lane.touched.extend([i, j]);
                        }
                        Op::Write(i, v) => {
                            lane.data[i as usize] = v;
                            lane.touched.push(i);
                        }
                        Op::Cmp(i, j) => lane.touched.extend([i, j]),
                        Op::Read(i) | Op::CmpV(i) => lane.touched.push(i),
                        Op::Tick => {}
                    }
                }
                lane.touched.truncate(64);
                lane.pos = end;
                if lane.pos == lane.ops.len() && lane.rank.is_none() {
                    lane.rank = Some(self.next_rank);
                    self.next_rank += 1;
                }
            }
        }

        let results_h = if self.full.is_empty() && self.full_rx.is_none() { 0.0 } else { 250.0 };
        let avail = ui.available_size();
        let (area, _) = ui.allocate_exact_size(vec2(avail.x, avail.y - results_h), Sense::hover());
        let painter = ui.painter_at(area);
        let (cols, rows) = (3, 2);
        let gap = 10.0;
        let cell = vec2((area.width() - gap * (cols - 1) as f32) / cols as f32, (area.height() - gap) / rows as f32);
        for (k, lane) in self.lanes.iter().enumerate() {
            let (c, r) = (k % cols, k / cols);
            let min = area.min + vec2(c as f32 * (cell.x + gap), r as f32 * (cell.y + gap));
            draw_lane(&painter, Rect::from_min_size(min, cell), lane, self.n);
        }

        if results_h > 0.0 {
            ui.add_space(10.0);
            self.full_results(ui);
        }
        ui.ctx().request_repaint();
    }

    fn full_results(&self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Full speed, single thread unless noted").size(16.0).strong());
        let best = self.full.iter().map(|r| r.n as f64 / r.time.as_secs_f64()).fold(0.0, f64::max);
        egui::Grid::new("full-speed").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
            for r in &self.full {
                ui.label(RichText::new(&r.name).color(TEXT));
                let rate = r.n as f64 / r.time.as_secs_f64();
                let (rect, _) = ui.allocate_exact_size(vec2(260.0, 14.0), Sense::hover());
                ui.painter().rect_filled(rect, 3.0, LINE);
                let mut bar = rect;
                bar.set_width((rect.width() * (rate / best) as f32).max(2.0));
                ui.painter().rect_filled(bar, 3.0, RUST);
                let mut text = format!(
                    "{} numbers in {}  ·  {} numbers/s",
                    fmt::count(r.n as f64),
                    fmt::duration(r.time),
                    fmt::count(rate)
                );
                if r.n < FULL_N {
                    // Insertion sort is O(n²): scale up to estimate the full size.
                    let est = r.time.as_secs_f64() * (FULL_N as f64 / r.n as f64).powi(2);
                    text += &format!("  ·  10M would take ~{:.0} hours", est / 3600.0);
                }
                ui.label(RichText::new(text).color(DIM).monospace());
                ui.end_row();
            }
            if self.full_rx.is_some() {
                ui.label(RichText::new("running…").color(DIM));
                ui.end_row();
            }
        });
    }
}

fn draw_lane(p: &egui::Painter, rect: Rect, lane: &Lane, n: usize) {
    p.rect_filled(rect, 8.0, PANEL);
    let head = 30.0;
    p.text(
        rect.min + vec2(12.0, head / 2.0 + 4.0),
        Align2::LEFT_CENTER,
        lane.name,
        FontId::proportional(16.0),
        TEXT,
    );
    let status = match lane.rank {
        Some(r) => format!("{} · {} ops", ["1st", "2nd", "3rd", "4th", "5th", "6th"][r.min(5)], fmt::grouped(lane.ops.len() as u64)),
        None => format!(
            "{:.0}% · {} ops",
            lane.pos as f64 / lane.ops.len().max(1) as f64 * 100.0,
            fmt::grouped(lane.pos as u64)
        ),
    };
    let status_color = match lane.rank {
        Some(0) => RUST,
        Some(_) => RUST_SOFT,
        None => DIM,
    };
    p.text(
        pos2(rect.max.x - 12.0, rect.min.y + head / 2.0 + 4.0),
        Align2::RIGHT_CENTER,
        status,
        FontId::monospace(13.0),
        status_color,
    );

    let plot = Rect::from_min_max(rect.min + vec2(10.0, head + 6.0), rect.max - vec2(10.0, 10.0));
    p.rect_filled(plot, 4.0, BG);
    let bw = plot.width() / lane.data.len() as f32;
    let done = lane.rank.is_some();
    let mut touched = vec![false; lane.data.len()];
    for &i in &lane.touched {
        touched[i as usize] = true;
    }
    for (i, &v) in lane.data.iter().enumerate() {
        let t = v as f32 / n as f32;
        let h = plot.height() * t;
        let x = plot.min.x + i as f32 * bw;
        let color = if touched[i] && !done {
            Color32::WHITE
        } else if done {
            RUST_SOFT.gamma_multiply(0.5 + 0.5 * t)
        } else {
            // Plum → rust by value, so order (and disorder) is visible at a glance.
            Color32::from_rgb((60.0 + 184.0 * t) as u8, (30.0 + 76.0 * t) as u8, (60.0 - 23.0 * t) as u8)
        };
        let bar = Rect::from_min_max(pos2(x, plot.max.y - h), pos2(x + bw.max(1.0) - if bw > 3.0 { 1.0 } else { 0.0 }, plot.max.y));
        p.rect_filled(bar, 0.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Vec<Vec<u32>> {
        let mut rng = Rng::new(4);
        let mut v = vec![vec![], vec![5], vec![2, 1], vec![3, 3, 3, 1, 1]];
        for input in [Input::Random, Input::NearlySorted, Input::Reversed, Input::FewUnique] {
            v.push(input.make(777, &mut rng));
        }
        v.push((0..5000).map(|_| rng.next_u64() as u32).collect());
        v.push((0..3000).map(|_| (rng.next_u64() % 7) as u32).collect());
        v
    }

    #[test]
    fn every_algorithm_sorts_plain_and_traced() {
        for data in inputs() {
            let mut expect = data.clone();
            expect.sort();
            for (name, traced, plain) in ALGOS {
                let mut v = data.clone();
                plain(&mut v);
                assert_eq!(v, expect, "{name} (plain) on n={}", data.len());

                // Replaying the trace from the original data must also end sorted.
                let mut t = Tracer { data: data.clone(), ops: vec![] };
                traced(&mut t);
                let mut replay = data.clone();
                for op in &t.ops {
                    match *op {
                        Op::Swap(i, j) => replay.swap(i as usize, j as usize),
                        Op::Write(i, v) => replay[i as usize] = v,
                        _ => {}
                    }
                }
                assert_eq!(replay, expect, "{name} (trace replay) on n={}", data.len());
            }
        }
    }
}
