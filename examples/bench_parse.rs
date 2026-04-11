use std::{env, hint::black_box, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut parallel = false;
    let first_arg = args.next().ok_or(
        "usage: cargo run --release --example bench_parse -- [--parallel] <catalog-path> [iterations]",
    )?;
    let path_arg = if first_arg == "--parallel" {
        parallel = true;
        args.next().ok_or(
            "usage: cargo run --release --example bench_parse -- [--parallel] <catalog-path> [iterations]",
        )?
    } else {
        first_arg
    };
    let path = PathBuf::from(path_arg);
    let iterations = args
        .next()
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(10);

    if iterations == 0 {
        return Err("iterations must be greater than zero".into());
    }

    let start = Instant::now();
    let mut diagnostics = 0usize;

    for _ in 0..iterations {
        let report = if parallel {
            #[cfg(feature = "parallel")]
            {
                xcassets::parse_catalog_parallel(&path)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                return Err(
                    "bench_parse --parallel requires the crate's `parallel` feature".into(),
                );
            }
        } else {
            xcassets::parse_catalog(&path)?
        };
        diagnostics = report.diagnostics.len();
        black_box(report);
    }

    let elapsed = start.elapsed();
    let average_ms = elapsed.as_secs_f64() * 1_000.0 / f64::from(iterations);

    println!("path: {}", path.display());
    println!("parallel: {parallel}");
    println!("iterations: {iterations}");
    println!("total_ms: {:.3}", elapsed.as_secs_f64() * 1_000.0);
    println!("avg_ms: {:.3}", average_ms);
    println!("last_run_diagnostics: {diagnostics}");

    Ok(())
}
