use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use actix_web::dev::Service;
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{App, HttpResponse, HttpServer, web};
use serde::Deserialize;

const INDEX_HTML: &str = include_str!("index.html");

static REQUESTS: AtomicU64 = AtomicU64::new(0);

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(INDEX_HTML)
}

async fn ping() -> HttpResponse {
    let n = REQUESTS.load(Ordering::Relaxed);
    HttpResponse::Ok()
        .content_type("application/json")
        .insert_header(("cache-control", "no-store"))
        .body(format!("{{\"pong\":true,\"served\":{n}}}"))
}

#[derive(Deserialize)]
struct Frame {
    cx: f64,
    cy: f64,
    span: f64,
    w: usize,
    h: usize,
    iter: u32,
}

async fn mandelbrot(q: web::Query<Frame>) -> actix_web::Result<HttpResponse> {
    let Frame { cx, cy, span, w, h, iter } = q.into_inner();
    let w = w.clamp(16, 1920);
    let h = h.clamp(16, 1200);
    let iter = iter.clamp(16, 5000);

    // CPU-bound work goes to the blocking pool so the async workers stay free.
    let frame = web::block(move || render(cx, cy, span, w, h, iter)).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header(("cache-control", "no-store"))
        .insert_header(("x-render-us", frame.elapsed.as_micros().to_string()))
        .insert_header(("x-iterations", frame.iterations.to_string()))
        .insert_header(("x-threads", frame.threads.to_string()))
        .body(frame.rgba))
}

struct Rendered {
    rgba: Vec<u8>,
    iterations: u64,
    threads: usize,
    elapsed: Duration,
}

/// Renders one RGBA frame, spreading rows across every core.
fn render(cx: f64, cy: f64, span: f64, w: usize, h: usize, max_iter: u32) -> Rendered {
    let start = Instant::now();
    let threads = thread::available_parallelism().map_or(4, |n| n.get());
    let mut rgba = vec![0u8; w * h * 4];

    let step = span / w as f64;
    let x0 = cx - step * w as f64 / 2.0;
    let y0 = cy - step * h as f64 / 2.0;

    let iterations = {
        // Threads pull rows from a shared queue, so expensive rows don't
        // leave one thread working while the others sit idle.
        let rows = Mutex::new(rgba.chunks_mut(w * 4).enumerate());
        thread::scope(|s| {
            let workers: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut done = 0u64;
                        loop {
                            let next = rows.lock().unwrap().next();
                            let Some((y, row)) = next else { break };
                            let ci = y0 + y as f64 * step;
                            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                                let cr = x0 + x as f64 * step;
                                let (n, mag2) = escape(cr, ci, max_iter);
                                done += n as u64;
                                px.copy_from_slice(&color(n, mag2, max_iter));
                            }
                        }
                        done
                    })
                })
                .collect();
            workers.into_iter().map(|w| w.join().unwrap()).sum()
        })
    };

    Rendered { rgba, iterations, threads, elapsed: start.elapsed() }
}

/// Returns (iterations actually performed, |z|^2 at exit).
#[inline]
fn escape(cr: f64, ci: f64, max_iter: u32) -> (u32, f64) {
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    let mut n = 0;
    while n < max_iter {
        let (r2, i2) = (zr * zr, zi * zi);
        if r2 + i2 > 65536.0 {
            return (n, r2 + i2);
        }
        zi = 2.0 * zr * zi + ci;
        zr = r2 - i2 + cr;
        n += 1;
    }
    (n, 0.0)
}

#[inline]
fn color(n: u32, mag2: f64, max_iter: u32) -> [u8; 4] {
    if n >= max_iter {
        return [9, 7, 12, 255];
    }
    // Smooth (fractional) iteration count removes the colour banding.
    let mu = n as f64 + 1.0 - (mag2.ln() * 0.5).ln() / std::f64::consts::LN_2;
    let t = mu * 0.022 + 0.62;
    let ch = |phase: f64| {
        let v = 0.5 + 0.5 * (std::f64::consts::TAU * (t + phase)).cos();
        (v * 255.0) as u8
    };
    [ch(0.0), ch(0.10), ch(0.20), 255]
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080u16);
    let addr = ("127.0.0.1", port);
    println!("Listening on http://{}:{}", addr.0, addr.1);
    if cfg!(debug_assertions) {
        println!("Debug build: run with `cargo run --release` for real numbers.");
    }

    HttpServer::new(|| {
        App::new()
            // Time every request inside the server and report it via the
            // standard Server-Timing header, which the page reads back.
            .wrap_fn(|req, srv| {
                let start = Instant::now();
                REQUESTS.fetch_add(1, Ordering::Relaxed);
                let fut = srv.call(req);
                async move {
                    let mut res = fut.await?;
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    if let Ok(v) = HeaderValue::from_str(&format!("app;dur={ms:.6}")) {
                        res.headers_mut()
                            .insert(HeaderName::from_static("server-timing"), v);
                    }
                    Ok(res)
                }
            })
            .route("/", web::get().to(index))
            .route("/api/ping", web::get().to(ping))
            .route("/api/mandelbrot", web::get().to(mandelbrot))
    })
    .bind(addr)?
    .run()
    .await
}
