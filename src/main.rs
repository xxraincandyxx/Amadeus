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
// - module: amadeus::agent::mesh::MeshManager
// - module: amadeus::agent::supervisor
// - module: amadeus::agent::worker::WorkerConfig
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
use amadeus::agent::mesh::MeshManager;
use amadeus::agent::supervisor::{Supervisor, SupervisorConfig};
use amadeus::agent::worker::WorkerConfig;
use amadeus::client::anthropic::AnthropicClient;
use amadeus::client::openai::OpenAIClient;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "api")]
use amadeus::api::http::run_server;

#[cfg(feature = "tui")]
use amadeus::agent::loop_agent::Agent;
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
            "--help" | "-h" => {
                println!("Amadeus - AI Agent SDK");
                println!();
                println!("Usage: amadeus [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --server [PORT]  Run HTTP API server (default: 3000)");
                println!("  --record [DIR]   Record session to JSON log (default: logs/testflow/sessions)");
                println!("  --help, -h       Show this help message");
                std::process::exit(0);
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
    let config = Arc::new(Config::load()?);
    let sdk_config = Arc::clone(&config);
    let mesh_manager = MeshManager::new(config.workdir.clone());

    // 2. Initialize Core LLM Client
    let provider = match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            ClientKind::Anthropic(client)
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            ClientKind::OpenAI(client)
        }
    };

    // 3. Mode Selection

    // --- MESH COORDINATION ---
    let supervisor_info = mesh_manager.get_supervisor_info();

    // --- SERVER MODE ---
    #[cfg(feature = "api")]
    if cli.server_mode {
        let port = cli.server_port;

        match provider {
            ClientKind::Anthropic(c) => {
                let mut supervisor =
                    Supervisor::new(c.clone(), SupervisorConfig::default(), sdk_config.clone());
                supervisor
                    .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                    .await?;
                let supervisor = Arc::new(supervisor);
                let s_clone = Arc::clone(&supervisor);
                tokio::spawn(async move {
                    let _ = s_clone.run().await;
                });
                mesh_manager.register_supervisor(&format!("http://localhost:{}", port));
                run_server(port, supervisor, sdk_config.clone()).await?;
            }
            ClientKind::OpenAI(c) => {
                let mut supervisor =
                    Supervisor::new(c.clone(), SupervisorConfig::default(), sdk_config.clone());
                supervisor
                    .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                    .await?;
                let supervisor = Arc::new(supervisor);
                let s_clone = Arc::clone(&supervisor);
                tokio::spawn(async move {
                    let _ = s_clone.run().await;
                });
                mesh_manager.register_supervisor(&format!("http://localhost:{}", port));
                run_server(port, supervisor, sdk_config.clone()).await?;
            }
        }
        mesh_manager.cleanup();
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
                let agent = Agent::new(c, sdk_config);
                let mut app = App::new(agent, workdir, model);
                if let Some(info) = supervisor_info {
                    app.set_mesh_mode(&info.supervisor_addr);
                }
                #[cfg(feature = "test-utils")]
                if let Some(rec) = recorder {
                    app.set_recorder(rec);
                }
                app.run().await?;
            }
            ClientKind::OpenAI(c) => {
                let agent = Agent::new(c, sdk_config);
                let mut app = App::new(agent, workdir, model);
                if let Some(info) = supervisor_info {
                    app.set_mesh_mode(&info.supervisor_addr);
                }
                #[cfg(feature = "test-utils")]
                if let Some(rec) = recorder {
                    app.set_recorder(rec);
                }
                app.run().await?;
            }
        }
        return Ok(());
    }

    // --- NO FEATURE ENABLED ---
    #[allow(unreachable_code)]
    {
        println!("Amadeus SDK - No features enabled.");
        println!("Enable 'tui' or 'api' feature to run.");
        Ok(())
    }
}

enum ClientKind {
    Anthropic(AnthropicClient),
    OpenAI(OpenAIClient),
}
