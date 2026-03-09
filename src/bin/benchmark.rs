use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use amadeus::agent::Config;
use amadeus::benchmark::{BenchmarkMode, BenchmarkRunner, BenchmarkRunnerOptions, RunStatus};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options = parse_args(env::args().skip(1).collect())?;
    let workdir = env::current_dir()?;
    let base_config = Config::load_with_hierarchy(&workdir).unwrap_or_else(|_| Config {
        workdir,
        ..Config::default()
    });

    let runner = BenchmarkRunner::new(Arc::new(base_config), options);
    let summary = runner.run_all().await?;

    println!(
        "Benchmark run {} complete: {} passed, {} failed, {} errored",
        summary.run_id, summary.passed_cases, summary.failed_cases, summary.error_cases
    );

    for result in &summary.results {
        let status = match result.status {
            RunStatus::Passed => "PASS",
            RunStatus::Failed => "FAIL",
            RunStatus::Error => "ERROR",
        };
        println!("[{status}] {} ({})", result.id, result.suite);
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> anyhow::Result<BenchmarkRunnerOptions> {
    let mut options = BenchmarkRunnerOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--fixtures" => {
                index += 1;
                options.fixtures_dir = PathBuf::from(required_arg(&args, index, "--fixtures")?);
            }
            "--output" => {
                index += 1;
                options.output_dir = PathBuf::from(required_arg(&args, index, "--output")?);
            }
            "--suite" => {
                index += 1;
                options.suite_filter = Some(required_arg(&args, index, "--suite")?.to_string());
            }
            "--case" => {
                index += 1;
                options.case_filter = Some(required_arg(&args, index, "--case")?.to_string());
            }
            "--mode" => {
                index += 1;
                let value = required_arg(&args, index, "--mode")?;
                options.mode_override = Some(match value {
                    "mock" => BenchmarkMode::Mock,
                    "live" => BenchmarkMode::Live,
                    other => anyhow::bail!("unsupported mode: {other}"),
                });
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
        index += 1;
    }

    Ok(options)
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> anyhow::Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing value for {flag}"))
}

fn print_help() {
    println!("Usage: cargo run --bin benchmark -- [options]");
    println!("  --fixtures <dir>   Benchmark fixture directory");
    println!("  --output <dir>     Output directory for run artifacts");
    println!("  --suite <name>     Run only one suite");
    println!("  --case <id>        Run only one case");
    println!("  --mode <mock|live> Override case execution mode");
}
