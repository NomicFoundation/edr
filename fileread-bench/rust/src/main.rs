// Rust file-read benchmark. Zero external deps (std only).
// Reads every *.txt file in <dir> and sums total bytes, across several
// strategies. Unlike Node, Rust's concurrency is bounded only by the number
// of OS threads we choose to spawn.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Instant,
};

fn read_all_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "txt").unwrap_or(false))
        .collect();
    files.sort();
    files
}

// Read as raw bytes (analog of Node's `readFile(f)` -> Buffer).
fn read_bytes(p: &Path) -> usize {
    fs::read(p).expect("read").len()
}

// Read and validate/decode as UTF-8 into a String (analog of Node's
// `readFile(f, "utf-8")` -> string). Returns the byte length; for the ASCII
// test data this matches Node's reported length.
fn read_utf8(p: &Path) -> usize {
    fs::read_to_string(p).expect("read utf-8").len()
}

fn sequential(files: &[PathBuf], read: fn(&Path) -> usize) -> usize {
    let mut bytes = 0usize;
    for f in files {
        bytes += read(f);
    }
    bytes
}

// Spawn `n` worker threads that pull files off a shared atomic cursor.
fn threaded(files: &[PathBuf], n: usize, read: fn(&Path) -> usize) -> usize {
    let cursor = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);
    thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
                let mut local = 0usize;
                loop {
                    let i = cursor.fetch_add(1, Ordering::Relaxed);
                    if i >= files.len() {
                        break;
                    }
                    local += read(&files[i]);
                }
                total.fetch_add(local, Ordering::Relaxed);
            });
        }
    });
    total.load(Ordering::Relaxed)
}

fn median(mut times: Vec<f64>) -> (f64, f64) {
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (times[0], times[times.len() / 2])
}

fn bench<F: Fn() -> usize>(label: &str, iters: usize, f: F) {
    let mut times = Vec::new();
    let mut bytes = 0;
    for _ in 0..iters {
        let t0 = Instant::now();
        bytes = f();
        times.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let (min, med) = median(times);
    println!(
        "{label:<34} median={med:>8.1} ms  min={min:>8.1} ms  ({bytes} bytes)"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map(String::as_str).unwrap_or("../data");
    let iters: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    // Optional single-strategy mode. Used for cold-cache runs, where each file
    // must be read exactly once per process:
    //   seq | t4 | tN | t64 | seq_utf8 | tN_utf8 | all
    let mode = args.get(3).map(String::as_str).unwrap_or("all");

    let files = read_all_files(dir);
    let cpus = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    println!(
        "\n== Rust (std)  files={}  cpus={}  iters={}  mode={} ==",
        files.len(),
        cpus,
        iters,
        mode
    );

    if matches!(mode, "seq" | "all") {
        bench("sequential", iters, || sequential(&files, read_bytes));
    }
    if matches!(mode, "t4" | "all") {
        bench("threaded (4 threads)", iters, || {
            threaded(&files, 4, read_bytes)
        });
    }
    if matches!(mode, "tN" | "all") {
        bench(&format!("threaded ({cpus} threads)"), iters, || {
            threaded(&files, cpus, read_bytes)
        });
    }
    if matches!(mode, "t64" | "all") {
        bench("threaded (64 threads)", iters, || {
            threaded(&files, 64, read_bytes)
        });
    }
    if matches!(mode, "seq_utf8" | "all") {
        bench("sequential utf-8", iters, || {
            sequential(&files, read_utf8)
        });
    }
    if matches!(mode, "tN_utf8" | "all") {
        bench(&format!("threaded ({cpus} threads) utf-8"), iters, || {
            threaded(&files, cpus, read_utf8)
        });
    }
}
