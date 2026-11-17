//! Human-friendly formatting for durations, counts and rates.

use std::time::Duration;

/// Formats a duration with a unit that keeps 2–4 significant digits.
pub fn duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 0.001 {
        format!("{:.0} ns", ms * 1e6)
    } else if ms < 1.0 {
        format!("{:.0} µs", ms * 1000.0)
    } else if ms < 100.0 {
        format!("{ms:.1} ms")
    } else if ms < 10_000.0 {
        format!("{:.2} s", ms / 1000.0)
    } else {
        format!("{:.1} s", ms / 1000.0)
    }
}

/// Formats a large number with a metric suffix: 1.23 K / M / B / T.
pub fn count(n: f64) -> String {
    let (v, s) = if n >= 1e12 {
        (n / 1e12, " T")
    } else if n >= 1e9 {
        (n / 1e9, " B")
    } else if n >= 1e6 {
        (n / 1e6, " M")
    } else if n >= 1e3 {
        (n / 1e3, " K")
    } else {
        return format!("{n:.0}");
    };
    if v >= 100.0 {
        format!("{v:.0}{s}")
    } else if v >= 10.0 {
        format!("{v:.1}{s}")
    } else {
        format!("{v:.2}{s}")
    }
}

/// `ops` per `d`, e.g. "1.23 B/s".
pub fn rate(ops: f64, d: Duration) -> String {
    let secs = d.as_secs_f64().max(1e-9);
    format!("{}/s", count(ops / secs))
}

/// A throughput with its unit. Bytes and FLOP get SI prefixes ("11.6 GB/s",
/// "73.7 GFLOP/s"); everything else reads as words ("1.29 B interactions/s").
pub fn throughput(per_sec: f64, unit: &str) -> String {
    let short = match unit {
        "bytes" => "B",
        "FLOP" => "FLOP",
        _ => return format!("{} {unit}/s", count(per_sec)),
    };
    let (v, prefix) = [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k")]
        .into_iter()
        .find(|&(scale, _)| per_sec >= scale)
        .map_or((per_sec, ""), |(scale, p)| (per_sec / scale, p));
    let digits = if v >= 100.0 { 0 } else if v >= 10.0 { 1 } else { 2 };
    format!("{v:.digits$} {prefix}{short}/s")
}

/// Number with thousands separators: 1234567 -> "1,234,567".
pub fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped(12), "12");
        assert_eq!(count(1_500_000.0), "1.50 M");
        assert_eq!(throughput(11.6e9, "bytes"), "11.6 GB/s");
        assert_eq!(throughput(73.7e9, "FLOP"), "73.7 GFLOP/s");
        assert_eq!(throughput(2.5e12, "FLOP"), "2.50 TFLOP/s");
        assert_eq!(throughput(1.29e9, "samples"), "1.29 B samples/s");
        assert_eq!(duration(Duration::from_micros(250)), "250 µs");
    }
}
