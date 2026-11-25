//! Isolated latency benchmark for the exact v2 six-class candidate.
//!
//! One process = one round: load one resident CPU session (1 intra-op thread),
//! verify the output against the reference, run the warm-ups, then time each
//! prediction with the in-process monotonic clock. Model, metadata, input,
//! reference and output paths are explicit arguments; nothing is hard-coded.
//!
//! Timed operation (one "prediction"): build the [1, 44] float32 input tensor,
//! run the session, and extract the label and the six class probabilities.
//! Session creation, file reading and process start are outside the timing.

use ndarray::Array2;
use ort::inputs;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const CRITERION_P99_MS: f64 = 200.0;

struct Args {
    model: PathBuf,
    metadata: PathBuf,
    input: PathBuf,
    reference: PathBuf,
    out_dir: PathBuf,
    round: u32,
    host: String,
    warmup: usize,
    iterations: usize,
}

fn parse_args() -> Args {
    let mut map = HashMap::new();
    let raw: Vec<String> = std::env::args().skip(1).collect();
    for pair in raw.chunks(2) {
        if pair.len() == 2 && pair[0].starts_with("--") {
            map.insert(pair[0].trim_start_matches("--").to_string(), pair[1].clone());
        } else {
            panic!("arguments must be --key value pairs");
        }
    }
    let get = |k: &str| map.get(k).cloned().unwrap_or_else(|| panic!("missing --{k}"));
    Args {
        model: get("model").into(),
        metadata: get("metadata").into(),
        input: get("input").into(),
        reference: get("reference").into(),
        out_dir: get("out-dir").into(),
        round: get("round").parse().expect("--round must be an integer"),
        host: get("host"),
        warmup: map.get("warmup").map(|v| v.parse().unwrap()).unwrap_or(50),
        iterations: map.get("iterations").map(|v| v.parse().unwrap()).unwrap_or(1000),
    }
}

/// One prediction: tensor build + run + output extraction. Returns (label, probabilities by class index).
fn predict(session: &mut Session, features: &[f32], n_classes: usize) -> (i64, Vec<f64>) {
    let array = Array2::from_shape_vec((1, features.len()), features.to_vec()).expect("input shape");
    let tensor = ort::value::Tensor::from_array(array).expect("input tensor");
    let outputs = session.run(inputs!["features" => tensor]).expect("session run");
    let label = outputs["label"].try_extract_tensor::<i64>().expect("label tensor").1[0];
    let seq = outputs["probabilities"]
        .try_extract_sequence::<ort::value::DynMapValueType>()
        .expect("probabilities sequence");
    let mut probs = vec![f64::NAN; n_classes];
    let kvs = seq[0].try_extract_key_values::<i64, f32>().expect("probability map");
    for (k, v) in kvs {
        let idx = k as usize;
        assert!(idx < n_classes, "class index {idx} outside the metadata class list");
        probs[idx] = v as f64;
    }
    (label, probs)
}

/// Nearest rank on a zero-indexed sorted vector: index = ceil(p * n) - 1.
fn nearest_rank(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let idx = ((p * n as f64).ceil() as usize).saturating_sub(1).min(n - 1);
    sorted[idx]
}

/// Peak resident set size from /proc/self/status (VmHWM, reported in kB = KiB).
fn peak_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn main() {
    let a = parse_args();
    fs::create_dir_all(&a.out_dir).expect("create output dir");
    let started = unix_seconds();

    // ---- inputs (read before any timing) ----
    let metadata: Value = serde_json::from_str(&fs::read_to_string(&a.metadata).unwrap()).unwrap();
    let classes: Vec<String> = metadata["classes"]
        .as_array()
        .expect("metadata.classes")
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    let n_classes = classes.len();
    let input: Value = serde_json::from_str(&fs::read_to_string(&a.input).unwrap()).unwrap();
    let features: Vec<f32> = input[0]
        .as_array()
        .expect("input [[44 values]]")
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    assert_eq!(features.len(), 44, "input must have 44 features");
    let reference: Value = serde_json::from_str(&fs::read_to_string(&a.reference).unwrap()).unwrap();
    let ref_probs: Vec<f64> = reference["probabilities"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let ref_label = reference["label"].as_i64().unwrap();
    let tolerance = reference["tolerance"].as_f64().unwrap_or(1e-5);

    // ---- one resident CPU session ----
    let mut session = Session::builder()
        .unwrap()
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .unwrap()
        .with_intra_threads(1)
        .unwrap()
        .with_inter_threads(1)
        .unwrap()
        .with_parallel_execution(false)
        .unwrap()
        .commit_from_file(&a.model)
        .expect("load model");

    // ---- correctness gate (before timing) ----
    let (label, probs) = predict(&mut session, &features, n_classes);
    let all_finite = probs.iter().all(|p| p.is_finite());
    let prob_sum: f64 = probs.iter().sum();
    let max_abs_diff = probs.iter().zip(&ref_probs).map(|(p, r)| (p - r).abs()).fold(0.0, f64::max);
    let argmax = probs
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.partial_cmp(y.1).unwrap())
        .map(|(i, _)| i as i64)
        .unwrap();
    let correctness = json!({
        "n_probabilities": probs.len(),
        "expected_classes": n_classes,
        "all_finite": all_finite,
        "probability_sum": prob_sum,
        "label": label,
        "label_class": classes.get(label as usize),
        "argmax_matches_label": argmax == label,
        "reference_label": ref_label,
        "label_matches_reference": label == ref_label,
        "max_abs_probability_difference": max_abs_diff,
        "tolerance": tolerance,
        "probabilities": probs,
    });
    let passed = probs.len() == n_classes
        && n_classes == 6
        && all_finite
        && (prob_sum - 1.0).abs() < 1e-4
        && argmax == label
        && label == ref_label
        && max_abs_diff < tolerance;
    if !passed {
        let fail = json!({"host": a.host, "round": a.round, "status": "correctness_failed", "correctness": correctness});
        fs::write(a.out_dir.join(format!("round_{}_FAILED.json", a.round)), serde_json::to_string_pretty(&fail).unwrap()).unwrap();
        eprintln!("correctness check FAILED: {}", fail);
        std::process::exit(2);
    }

    // ---- warm-up ----
    for _ in 0..a.warmup {
        let _ = predict(&mut session, &features, n_classes);
    }

    // ---- timed predictions ----
    let mut durations_us: Vec<f64> = Vec::with_capacity(a.iterations);
    for _ in 0..a.iterations {
        let t0 = Instant::now();
        let out = predict(&mut session, &features, n_classes);
        let elapsed = t0.elapsed();
        std::hint::black_box(out);
        durations_us.push(elapsed.as_nanos() as f64 / 1000.0);
    }

    // ---- after timing: memory, stats, files ----
    let rss_kib = peak_rss_kib();
    let mut sorted = durations_us.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let n = sorted.len() as f64;
    let mean = sorted.iter().sum::<f64>() / n;
    let (p50, p95, p99) = (nearest_rank(&sorted, 0.50), nearest_rank(&sorted, 0.95), nearest_rank(&sorted, 0.99));

    let mut csv = fs::File::create(a.out_dir.join(format!("round_{}_durations.csv", a.round))).unwrap();
    writeln!(csv, "host,round,index,duration_us").unwrap();
    for (i, d) in durations_us.iter().enumerate() {
        writeln!(csv, "{},{},{},{:.3}", a.host, a.round, i, d).unwrap();
    }

    let summary = json!({
        "host": a.host,
        "round": a.round,
        "status": "ok",
        "arch": std::env::consts::ARCH,
        "started_unix": started,
        "finished_unix": unix_seconds(),
        "model": a.model.display().to_string(),
        "ort_crate": "2.0.0-rc.13",
        "onnxruntime_build_info": ort::info(),
        "session": {"graph_optimization": "Level3", "intra_op_threads": 1, "inter_op_threads": 1, "parallel_execution": false, "execution_provider": "CPU (default)"},
        "protocol": {"warmup": a.warmup, "iterations": a.iterations, "clock": "std::time::Instant (monotonic)", "timed_operation": "input tensor build + session.run + label and probability extraction", "percentile_rule": "nearest rank, index ceil(p*n)-1 on the zero-indexed sorted vector"},
        "correctness": correctness,
        "latency_us": {"mean": mean, "min": sorted[0], "max": sorted[sorted.len() - 1], "p50": p50, "p95": p95, "p99": p99},
        "latency_ms": {"mean": mean / 1000.0, "min": sorted[0] / 1000.0, "max": sorted[sorted.len() - 1] / 1000.0, "p50": p50 / 1000.0, "p95": p95 / 1000.0, "p99": p99 / 1000.0},
        "peak_rss": {"source": "/proc/self/status VmHWM (kB, i.e. KiB), read after the timed loop", "kib": rss_kib, "decimal_mb": rss_kib.map(|k| k as f64 * 1024.0 / 1_000_000.0)},
        "criterion": {"rule": "p99 < 200 ms in this round", "p99_ms": p99 / 1000.0, "passed": p99 / 1000.0 < CRITERION_P99_MS},
    });
    fs::write(a.out_dir.join(format!("round_{}_summary.json", a.round)), serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    println!("{}", serde_json::to_string(&summary["latency_us"]).unwrap());
}
