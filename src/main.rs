// @amadeus-header
// summary: CLI entrypoint for TUI mode, server mode, and session recording.
// layer: script
// status: active
// feature_flags:
// - api
// - test-utils
// - tui
// provides:
// - module: bin::amadeus
// uses:
// - module: amadeus::agent::config
// - module: amadeus::client::anthropic::AnthropicClient
// - module: amadeus::client::openai::OpenAIClient
// - module: amadeus::api::http::run_server
// - module: amadeus::agent::loop_agent::Agent
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Spawns asynchronous tasks.
// - Writes output to stdout or stderr.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Amadeus - AI Agent SDK
//!
//! Run with `cargo run` for TUI mode, or `cargo run -- --server` for HTTP mode.
//!
//! Flags:
//!   --server [PORT]  Run HTTP API server (default port 3000)
//!   --record [DIR]   Record session to JSON log (default: logs/testflow/sessions)

use amadeus::agent::config::{Config, Provider};
use amadeus::assessment::{
    default_prompt as default_assessment_prompt, AssessmentConfig, AssessmentRunner,
    ScriptedAssessmentClient,
};
use amadeus::client::anthropic::AnthropicClient;
use amadeus::client::openai::OpenAIClient;
use amadeus::permissions::PermissionMode;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "api")]
use amadeus::api::http::run_server;

#[cfg(feature = "tui")]
use amadeus::ui::App;

#[cfg(feature = "test-utils")]
use amadeus::test_utils::SessionRecorder;

fn parse_args(args: &[String]) -> CliArgs {
    let mut cli = CliArgs::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--server" => {
                cli.server_mode = true;
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    if let Ok(port) = args[i + 1].parse() {
                        cli.server_port = port;
                        i += 1;
                    }
                }
            }
            "--record" => {
                cli.record_session = true;
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    cli.record_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--export" => {
                if i + 1 < args.len() {
                    cli.export_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                } else {
                    eprintln!("--export requires a path argument");
                    std::process::exit(2);
                }
            }
            "--help" | "-h" => {
                println!("Amadeus - AI Agent SDK");
                println!();
                println!("Usage: amadeus [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --assess-features [DIR]  Run read-only feature assessment and write a report");
                println!("  --permission-mode MODE   Override permission mode (read-only|workspace-write|danger-full-access|prompt)");
                println!("  --server [PORT]  Run HTTP API server (default: 3000)");
                println!("  --record [DIR]   Record session to JSON log (default: logs/testflow/sessions)");
                println!(
                    "  --export PATH    Export the TUI conversation to PATH on exit (.md or .json)"
                );
                println!("  --llm-trace [DIR]  Log full LLM request/response payloads per turn (default: logs/llm_trace)");
                println!("  --help, -h       Show this help message");
                std::process::exit(0);
            }
            "--assess-features" => {
                cli.assess_features = true;
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    cli.assessment_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--permission-mode" => {
                if i + 1 < args.len() {
                    cli.permission_mode = PermissionMode::parse(&args[i + 1]);
                    i += 1;
                }
            }
            "--llm-trace" => {
                cli.llm_trace = true;
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    cli.llm_trace_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    cli
}

#[derive(Default)]
struct CliArgs {
    server_mode: bool,
    server_port: u16,
    record_session: bool,
    record_dir: Option<PathBuf>,
    assess_features: bool,
    assessment_dir: Option<PathBuf>,
    permission_mode: Option<PermissionMode>,
    export_path: Option<PathBuf>,
    llm_trace: bool,
    llm_trace_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_ok() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(false)
            .try_init();
    }

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let cli = parse_args(&args);

    // 1. Initial Setup
    let mut resolved_config = if cli.assess_features {
        Config::load_for_assessment()?
    } else {
        Config::load()?
    };
    if let Some(permission_mode) = cli.permission_mode {
        resolved_config.permission_mode = permission_mode;
    }
    let config = Arc::new(resolved_config);

    // 3. Mode Selection

    if cli.assess_features {
        let report_dir = cli
            .assessment_dir
            .unwrap_or_else(|| config.workdir.join("logs").join("assessments"));
        let sdk_config = Arc::clone(&config);
        if config.api_key.is_empty() {
            let runner = AssessmentRunner::new(
                ScriptedAssessmentClient::new(config.workdir.clone()),
                sdk_config.clone(),
            );
            let result = runner
                .run(AssessmentConfig::new(
                    report_dir,
                    default_assessment_prompt(&config.workdir),
                ))
                .await?;
            println!(
                "Assessment report written to {}",
                result.report_path.display()
            );
        } else {
            match build_client(&config) {
                ClientKind::Anthropic(c) => {
                    let runner = AssessmentRunner::new(c, sdk_config.clone());
                    let result = runner
                        .run(AssessmentConfig::new(
                            report_dir,
                            default_assessment_prompt(&config.workdir),
                        ))
                        .await?;
                    println!(
                        "Assessment report written to {}",
                        result.report_path.display()
                    );
                }
                ClientKind::OpenAI(c) => {
                    let runner = AssessmentRunner::new(c, sdk_config.clone());
                    let result = runner
                        .run(AssessmentConfig::new(
                            report_dir,
                            default_assessment_prompt(&config.workdir),
                        ))
                        .await?;
                    println!(
                        "Assessment report written to {}",
                        result.report_path.display()
                    );
                }
            }
        }
        return Ok(());
    }

    let sdk_config = Arc::clone(&config);
    let provider = build_client(&config);

    // Build an LLM I/O trace sink when explicitly requested or when session
    // logging is already configured. Captures full request/response payloads.
    let llm_trace_dir = cli
        .llm_trace_dir
        .clone()
        .or_else(|| config.session_log_dir.clone())
        .unwrap_or_else(|| config.workdir.join("logs").join("llm_trace"));
    let llm_trace = if cli.llm_trace || config.session_log_dir.is_some() {
        let session_id = uuid::Uuid::new_v4().to_string();
        match amadeus::agent::llm_trace::LlmTraceSink::open(&llm_trace_dir, &session_id) {
            Ok(sink) => {
                println!("📝 Logging LLM I/O trace to {}", sink.path().display());
                Some(std::sync::Arc::new(sink))
            }
            Err(error) => {
                eprintln!("Failed to open llm trace log: {error}");
                None
            }
        }
    } else {
        None
    };

    // --- SERVER MODE ---
    #[cfg(feature = "api")]
    if cli.server_mode {
        let port = cli.server_port;

        match provider {
            ClientKind::Anthropic(c) => {
                run_server(port, c.clone(), sdk_config.clone(), llm_trace.clone()).await?;
            }
            ClientKind::OpenAI(c) => {
                run_server(port, c.clone(), sdk_config.clone(), llm_trace.clone()).await?;
            }
        }
        return Ok(());
    }

    // --- TUI MODE ---
    #[cfg(feature = "tui")]
    {
        let workdir = config.workdir.clone();
        let model = config.model.clone();

        // Initialize recorder if --record flag is set
        #[cfg(feature = "test-utils")]
        let recorder = if cli.record_session {
            let record_dir = cli
                .record_dir
                .unwrap_or_else(|| workdir.join("logs").join("testflow").join("sessions"));
            let recorder = SessionRecorder::new(record_dir);
            recorder.set_config_snapshot(config.as_ref()).await;
            Some(recorder)
        } else {
            None
        };

        match provider {
            ClientKind::Anthropic(c) => {
                let agent = build_agent(c, Arc::clone(&sdk_config), &llm_trace);
                let mut app = App::new(agent, workdir, model);
                #[cfg(feature = "test-utils")]
                if let Some(rec) = recorder {
                    app.set_recorder(rec);
                }
                if let Some(ref export_path) = cli.export_path {
                    app.set_export_on_exit(Some(export_path.clone()));
                }
                app.run().await?;
            }
            ClientKind::OpenAI(c) => {
                let agent = build_agent(c, Arc::clone(&sdk_config), &llm_trace);
                let mut app = App::new(agent, workdir, model);
                #[cfg(feature = "test-utils")]
                if let Some(rec) = recorder {
                    app.set_recorder(rec);
                }
                if let Some(ref export_path) = cli.export_path {
                    app.set_export_on_exit(Some(export_path.clone()));
                }
                app.run().await?;
            }
        }
        return Ok(());
    }

    // --- NO FEATURE ENABLED ---
    #[allow(unreachable_code)]
    {
        let _ = (sdk_config, llm_trace);
        println!("Amadeus SDK - No features enabled.");
        println!("Enable 'tui' or 'api' feature to run.");
        Ok(())
    }
}

enum ClientKind {
    Anthropic(AnthropicClient),
    OpenAI(OpenAIClient),
}

fn build_client(config: &Config) -> ClientKind {
    match config.provider {
        Provider::Anthropic => ClientKind::Anthropic(AnthropicClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        )),
        Provider::OpenAI => ClientKind::OpenAI(OpenAIClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        )),
    }
}

/// Build a default-tools agent, attaching the LLM trace sink when present.
#[cfg(feature = "tui")]
fn build_agent<C: amadeus::client::LLMClient + Clone + 'static>(
    client: C,
    config: Arc<Config>,
    llm_trace: &Option<Arc<amadeus::agent::llm_trace::LlmTraceSink>>,
) -> amadeus::agent::loop_agent::Agent<C> {
    let mut builder =
        amadeus::agent::loop_agent::Agent::builder(client, config).with_default_tools();
    if let Some(trace) = llm_trace {
        builder = builder.with_llm_trace(Some(Arc::clone(trace)));
    }
    builder.build()
}
