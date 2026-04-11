use std::{env, hint::black_box, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).ok_or(
        "usage: cargo run --release --example bench_references -- <catalog-path> [iterations]",
    )?;
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
    let mut references = 0usize;

    for _ in 0..iterations {
        let index = xcassets::index_asset_references(&path)?;
        diagnostics = index.diagnostics.len();
        references = index.references.len();
        black_box(index);
    }

    let elapsed = start.elapsed();
    let average_ms = elapsed.as_secs_f64() * 1_000.0 / f64::from(iterations);

    println!("path: {}", path.display());
    println!("iterations: {iterations}");
    println!("total_ms: {:.3}", elapsed.as_secs_f64() * 1_000.0);
    println!("avg_ms: {:.3}", average_ms);
    println!("last_run_references: {references}");
    println!("last_run_diagnostics: {diagnostics}");

    Ok(())
}
