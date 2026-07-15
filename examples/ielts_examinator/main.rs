// @amadeus-header
// summary: Runnable IELTS speaking and writing examiner example built on the core agent loop.
// layer: example
// status: experimental
// feature_flags:
// - full
// provides:
// - module: example::ielts_examinator
// - cmd: cargo run --example ielts_examinator --features full
// uses:
// - type: amadeus::agent::config::Config
// - type: amadeus::agent::loop_agent::Agent
// - type: amadeus::tools::ToolRegistry
// - cmd: cargo run --example ielts_examinator --features full
// - protocol: OpenAI-compatible or Anthropic chat completion
// - protocol: React browser module
// invariants:
// - Domain prompt profiles replace the default coding-agent prompt.
// - The example keeps the model tool catalog empty for examiner-only turns.
// side_effects:
// - Performs network or HTTP operations.
// - Writes session logs to disk.
// tests:
// - cmd: cargo check --example ielts_examinator --features full
// @end-amadeus-header

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use amadeus::{
    agent::{
        config::{Config, PromptMergeMode, PromptProfileConfig, PromptSectionConfig, Provider},
        loop_agent::Agent,
    },
    client::{anthropic::AnthropicClient, openai::OpenAIClient},
    tools::ToolRegistry,
};

const PROFILE_NAME: &str = "ielts-examinator";

const SCORING_POLICY: &str = r#"You are an IELTS practice examiner. You provide realistic practice feedback, not an official IELTS score.

Use these assessment dimensions:
- Speaking: fluency and coherence, lexical resource, grammatical range and accuracy, pronunciation.
- Writing: task achievement or task response, coherence and cohesion, lexical resource, grammatical range and accuracy.

Be fair, evidence-led, and specific. Estimate bands in 0.5 increments only when there is enough evidence. If the sample is too short, say that the estimate has low confidence.

Never invent official test administration details. Do not claim to be affiliated with IELTS, Cambridge, the British Council, IDP, or any exam board.

Output format:
1. Overall estimate
2. Criterion breakdown
3. Evidence from the candidate answer
4. Highest-impact corrections
5. Next practice task"#;

const SPEAKING_PROFILE: &str = r#"You run IELTS Speaking practice.

Act as the examiner during the live exchange. Ask one question at a time, keep the tone professional and natural, and adapt follow-up questions to the candidate's previous answer.

If the user asks for a full evaluation, give feedback using the scoring policy. If the user gives only a short answer, ask a follow-up instead of over-scoring. Do not use tools."#;

const WRITING_PROFILE: &str = r#"You evaluate IELTS Writing practice.

Read the candidate response carefully, identify whether it is Task 1 or Task 2, and assess it against the scoring policy. Give practical corrections and a short rewrite sample for the weakest paragraph or sentence cluster.

Do not rewrite the whole essay unless the user explicitly asks. Do not use tools."#;

const WEB_INDEX: &str = include_str!("web/index.html");
const WEB_STYLES: &str = include_str!("web/styles.css");
const WEB_APP: &str = include_str!("web/app.js");
const WEB_CSS_TOKENS: &str = include_str!("web/css/tokens.css");
const WEB_CSS_BASE: &str = include_str!("web/css/base.css");
const WEB_CSS_SHELL: &str = include_str!("web/css/shell.css");
const WEB_CSS_COMPOSER: &str = include_str!("web/css/composer.css");
const WEB_CSS_RESULT: &str = include_str!("web/css/result.css");
const WEB_CSS_RESPONSIVE: &str = include_str!("web/css/responsive.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Speaking,
    Writing,
    Rubric,
}

impl Mode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "speaking" | "speak" => Some(Self::Speaking),
            "writing" | "write" => Some(Self::Writing),
            "rubric" | "score" => Some(Self::Rubric),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Speaking => "speaking",
            Self::Writing => "writing",
            Self::Rubric => "rubric",
        }
    }

    fn default_prompt(self) -> &'static str {
        match self {
            Self::Speaking => {
                "Start an IELTS Speaking Part 2 practice session. Give me a cue card, one minute of preparation guidance, and then ask me to answer."
            }
            Self::Writing => {
                "Evaluate this IELTS Writing Task 2 response and give band-style feedback:\n\nSome people believe that university students should study whatever they like, while others believe they should only study subjects useful for the future. Discuss both views and give your opinion.\n\nIn modern society, students have many choices in university. I think they should choose subjects they like, because interest can help them study harder. However, useful subjects are also important because they can help students find a good job after graduation. In my opinion, students should balance their interest and future career."
            }
            Self::Rubric => "Print the IELTS practice scoring rubric you will use.",
        }
    }
}

#[derive(Debug)]
struct Cli {
    mode: Mode,
    prompt: Option<String>,
    provider: Option<Provider>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    log_dir: Option<PathBuf>,
    max_turns: usize,
    web: bool,
    port: u16,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            mode: Mode::Speaking,
            prompt: None,
            provider: None,
            base_url: None,
            model: None,
            api_key: None,
            log_dir: None,
            max_turns: 4,
            web: false,
            port: 7878,
        }
    }
}

fn main_usage() -> &'static str {
    "Usage: cargo run --example ielts_examinator --features full -- [speaking|writing|rubric] [OPTIONS]\n\nOptions:\n  --web               Serve the IELTS web UI\n  --port PORT         Web UI port, default 7878\n  --prompt TEXT       Prompt or candidate response to send\n  --provider NAME     anthropic or openai\n  --base-url URL      Provider base URL\n  --model ID          Model identifier\n  --api-key KEY       Provider API key, or any placeholder for local OpenAI-compatible servers\n  --log-dir PATH      Session log directory\n  --max-turns N       Maximum model turns before stopping, default 4\n  --help, -h          Show this help"
}

fn parse_cli(args: &[String]) -> Result<Cli> {
    let mut cli = Cli::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("{}", main_usage());
                std::process::exit(0);
            }
            "--web" => {
                cli.web = true;
            }
            "--port" => {
                i += 1;
                cli.port = required_arg(args, i, "--port")?
                    .parse()
                    .context("--port must be a valid TCP port")?;
            }
            "--prompt" => {
                i += 1;
                cli.prompt = Some(required_arg(args, i, "--prompt")?.to_string());
            }
            "--provider" => {
                i += 1;
                cli.provider = Some(parse_provider(required_arg(args, i, "--provider")?)?);
            }
            "--base-url" => {
                i += 1;
                cli.base_url = Some(required_arg(args, i, "--base-url")?.to_string());
            }
            "--model" => {
                i += 1;
                cli.model = Some(required_arg(args, i, "--model")?.to_string());
            }
            "--api-key" => {
                i += 1;
                cli.api_key = Some(required_arg(args, i, "--api-key")?.to_string());
            }
            "--log-dir" => {
                i += 1;
                cli.log_dir = Some(PathBuf::from(required_arg(args, i, "--log-dir")?));
            }
            "--max-turns" => {
                i += 1;
                cli.max_turns = required_arg(args, i, "--max-turns")?
                    .parse()
                    .context("--max-turns must be a positive integer")?;
            }
            value if !value.starts_with('-') => {
                cli.mode =
                    Mode::parse(value).with_context(|| format!("unknown IELTS mode '{value}'"))?;
            }
            value => bail!("unknown option '{value}'\n\n{}", main_usage()),
        }
        i += 1;
    }

    if cli.max_turns == 0 {
        bail!("--max-turns must be greater than zero");
    }

    Ok(cli)
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str> {
    args.get(index)
        .map(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{flag} requires a value"))
}

fn parse_provider(value: &str) -> Result<Provider> {
    match value.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Ok(Provider::Anthropic),
        "openai" | "openai-compatible" | "vllm" => Ok(Provider::OpenAI),
        other => bail!("unknown provider '{other}'"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_cli(&std::env::args().collect::<Vec<_>>())?;
    let mut config = Config::load_for_assessment()?;
    apply_cli_overrides(&mut config, &cli);

    if cli.web {
        serve_web_ui(config, cli.port, cli.max_turns).await?;
        return Ok(());
    }

    let prompt = cli
        .prompt
        .as_deref()
        .unwrap_or_else(|| cli.mode.default_prompt());
    let log_dir = config
        .session_log_dir
        .clone()
        .unwrap_or_else(|| config.workdir.join("logs").join("ielts_examinator"));

    println!("IELTS Examinator mode: {}", cli.mode.as_str());
    println!("Model: {}", config.model);
    println!("Session logs: {}", log_dir.display());
    println!();

    let output = run_examiner_for_mode(config, cli.mode, prompt, cli.max_turns, &log_dir).await?;
    println!("{}", output.text.trim());

    if let Some(path) = output.session_log {
        println!();
        println!("Latest session log: {}", path.display());
    }

    Ok(())
}

fn apply_cli_overrides(config: &mut Config, cli: &Cli) {
    if let Some(provider) = cli.provider.clone() {
        config.provider = provider;
    }
    if let Some(base_url) = cli.base_url.clone() {
        config.base_url = Some(base_url);
    }
    if let Some(model) = cli.model.clone() {
        config.model = model;
    }
    if let Some(api_key) = cli.api_key.clone() {
        config.api_key = api_key;
    }

    config.session_log_dir = Some(
        cli.log_dir
            .clone()
            .unwrap_or_else(|| config.workdir.join("logs").join("ielts_examinator")),
    );
    config.session_log_compress = false;
    config.auto_compact = false;
}

fn install_prompt_profile(config: &mut Config, mode: Mode) {
    let profile_content = match mode {
        Mode::Speaking => SPEAKING_PROFILE,
        Mode::Writing | Mode::Rubric => WRITING_PROFILE,
    };
    let sections = vec![
        PromptSectionConfig {
            id: "ielts-role".to_string(),
            title: Some("IELTS Examinator".to_string()),
            content: profile_content.to_string(),
        },
        PromptSectionConfig {
            id: "ielts-scoring-policy".to_string(),
            title: Some("Scoring Policy".to_string()),
            content: SCORING_POLICY.to_string(),
        },
    ];

    let mut profiles = HashMap::new();
    profiles.insert(
        PROFILE_NAME.to_string(),
        PromptProfileConfig {
            mode: PromptMergeMode::Replace,
            sections,
            files: Vec::new(),
            include_project_context: false,
        },
    );
    config.prompts.active_profile = PROFILE_NAME.to_string();
    config.prompts.profiles = profiles;
}

#[derive(Debug, Clone)]
struct ExaminerOutput {
    text: String,
    session_log: Option<PathBuf>,
}

async fn run_examiner_for_mode(
    mut config: Config,
    mode: Mode,
    prompt: &str,
    max_turns: usize,
    log_dir: &Path,
) -> Result<ExaminerOutput> {
    install_prompt_profile(&mut config, mode);

    match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            execute_examiner(client, config, prompt, max_turns, log_dir).await
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            execute_examiner(client, config, prompt, max_turns, log_dir).await
        }
    }
}

async fn execute_examiner<C>(
    client: C,
    config: Config,
    prompt: &str,
    max_turns: usize,
    log_dir: &Path,
) -> Result<ExaminerOutput>
where
    C: amadeus::client::LLMClient + Clone + 'static,
{
    let agent = Agent::builder(client, Arc::new(config))
        .with_tools(ToolRegistry::new())
        .build();
    let result = agent.run_with_turn_limit(prompt, max_turns).await?;

    Ok(ExaminerOutput {
        text: result.text,
        session_log: latest_session_log(log_dir)?,
    })
}

#[derive(Debug, Deserialize)]
struct ExamRequest {
    mode: String,
    prompt: String,
    max_turns: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ExamResponse {
    mode: String,
    model: String,
    text: String,
    session_log: Option<String>,
    duration_ms: u128,
}

async fn serve_web_ui(config: Config, port: u16, default_max_turns: usize) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("failed to bind web UI to 127.0.0.1:{port}"))?;
    let config = Arc::new(config);

    println!("IELTS Examinator web UI: http://127.0.0.1:{port}");
    println!("Model: {}", config.model);
    if let Some(log_dir) = &config.session_log_dir {
        println!("Session logs: {}", log_dir.display());
    }

    loop {
        let (stream, _) = listener.accept().await?;
        let config = Arc::clone(&config);
        tokio::spawn(async move {
            if let Err(error) = handle_http_connection(stream, config, default_max_turns).await {
                eprintln!("web request failed: {error}");
            }
        });
    }
}

async fn handle_http_connection(
    mut stream: TcpStream,
    config: Arc<Config>,
    default_max_turns: usize,
) -> Result<()> {
    let request = read_http_request(&mut stream).await?;
    let response = route_http_request(&request, config, default_max_turns).await;
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

async fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);

        if header_end.is_none() {
            header_end = find_header_end(&buffer);
            if let Some(end) = header_end {
                let headers = String::from_utf8_lossy(&buffer[..end]);
                content_length = parse_content_length(&headers);
            }
        }

        if let Some(end) = header_end {
            if buffer.len() >= end + 4 + content_length {
                break;
            }
        }

        if buffer.len() > 1_000_000 {
            bail!("HTTP request is too large");
        }
    }

    let end = header_end.context("invalid HTTP request")?;
    let headers = String::from_utf8_lossy(&buffer[..end]);
    let request_line = headers.lines().next().context("missing request line")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or("/").to_string();
    let body_bytes = &buffer[end + 4..end + 4 + content_length];
    let body = String::from_utf8(body_bytes.to_vec()).context("request body must be UTF-8")?;

    Ok(HttpRequest { method, path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

async fn route_http_request(
    request: &HttpRequest,
    config: Arc<Config>,
    default_max_turns: usize,
) -> String {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => http_response("200 OK", "text/html", WEB_INDEX),
        ("GET", "/favicon.ico") => http_response("204 No Content", "image/x-icon", ""),
        ("GET", "/styles.css") => http_response("200 OK", "text/css", WEB_STYLES),
        ("GET", "/css/tokens.css") => http_response("200 OK", "text/css", WEB_CSS_TOKENS),
        ("GET", "/css/base.css") => http_response("200 OK", "text/css", WEB_CSS_BASE),
        ("GET", "/css/shell.css") => http_response("200 OK", "text/css", WEB_CSS_SHELL),
        ("GET", "/css/composer.css") => http_response("200 OK", "text/css", WEB_CSS_COMPOSER),
        ("GET", "/css/result.css") => http_response("200 OK", "text/css", WEB_CSS_RESULT),
        ("GET", "/css/responsive.css") => http_response("200 OK", "text/css", WEB_CSS_RESPONSIVE),
        ("GET", "/app.js") => http_response("200 OK", "application/javascript", WEB_APP),
        ("GET", "/api/health") => {
            let body = serde_json::json!({
                "ok": true,
                "model": config.model,
            });
            json_response("200 OK", &body.to_string())
        }
        ("OPTIONS", "/api/exam") => http_response("204 No Content", "text/plain", ""),
        ("POST", "/api/exam") => {
            handle_exam_request(&request.body, config, default_max_turns).await
        }
        _ => http_response("404 Not Found", "text/plain", "Not found"),
    }
}

async fn handle_exam_request(body: &str, config: Arc<Config>, default_max_turns: usize) -> String {
    let request = match serde_json::from_str::<ExamRequest>(body) {
        Ok(request) => request,
        Err(error) => return error_response("400 Bad Request", &format!("Invalid JSON: {error}")),
    };
    let mode = match Mode::parse(&request.mode) {
        Some(mode) => mode,
        None => {
            return error_response(
                "400 Bad Request",
                "Mode must be speaking, writing, or rubric",
            )
        }
    };
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return error_response("400 Bad Request", "Prompt cannot be empty");
    }

    let max_turns = request.max_turns.unwrap_or(default_max_turns).clamp(1, 8);
    let run_config = (*config).clone();
    let log_dir = run_config
        .session_log_dir
        .clone()
        .unwrap_or_else(|| run_config.workdir.join("logs").join("ielts_examinator"));
    let started = Instant::now();
    let output =
        match run_examiner_for_mode(run_config.clone(), mode, prompt, max_turns, &log_dir).await {
            Ok(output) => output,
            Err(error) => {
                return error_response(
                    "500 Internal Server Error",
                    &format!("Examiner failed: {error}"),
                )
            }
        };

    let response = ExamResponse {
        mode: mode.as_str().to_string(),
        model: run_config.model,
        text: output.text,
        session_log: output.session_log.map(|path| path.display().to_string()),
        duration_ms: started.elapsed().as_millis(),
    };

    match serde_json::to_string(&response) {
        Ok(body) => json_response("200 OK", &body),
        Err(error) => error_response(
            "500 Internal Server Error",
            &format!("Failed to encode response: {error}"),
        ),
    }
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn json_response(status: &str, body: &str) -> String {
    http_response(status, "application/json", body)
}

fn error_response(status: &str, message: &str) -> String {
    let body = serde_json::json!({ "error": message }).to_string();
    json_response(status, &body)
}

fn latest_session_log(log_dir: &Path) -> Result<Option<PathBuf>> {
    if !log_dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("session_") && name.ends_with(".json"))
            .unwrap_or(false)
        {
            continue;
        }

        let modified = entry.metadata()?.modified()?;
        if latest
            .as_ref()
            .map(|(current, _)| modified > *current)
            .unwrap_or(true)
        {
            latest = Some((modified, path));
        }
    }

    Ok(latest.map(|(_, path)| path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parse_cli_accepts_openai_compatible_endpoint() {
        let cli = parse_cli(&args(&[
            "ielts_examinator",
            "writing",
            "--provider",
            "vllm",
            "--base-url",
            "http://localhost:8000/v1",
            "--model",
            "local-model",
            "--api-key",
            "empty",
            "--max-turns",
            "2",
        ]))
        .expect("CLI should parse");

        assert_eq!(cli.mode, Mode::Writing);
        assert_eq!(cli.provider, Some(Provider::OpenAI));
        assert_eq!(cli.base_url.as_deref(), Some("http://localhost:8000/v1"));
        assert_eq!(cli.model.as_deref(), Some("local-model"));
        assert_eq!(cli.api_key.as_deref(), Some("empty"));
        assert_eq!(cli.max_turns, 2);
    }

    #[test]
    fn install_prompt_profile_replaces_default_prompt() {
        let mut config = Config::default();

        install_prompt_profile(&mut config, Mode::Speaking);
        let prompt = config.system_prompt(false);

        assert_eq!(config.prompts.active_profile, PROFILE_NAME);
        assert!(prompt.contains("IELTS Speaking practice"));
        assert!(prompt.contains("IELTS practice examiner"));
        assert!(!prompt.contains("CLI agent"));
    }

    #[test]
    fn latest_session_log_ignores_non_session_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let older = temp.path().join("session_20260101_000000.json");
        let newer = temp.path().join("session_20260101_000001.json");
        let ignored = temp.path().join("notes.json");

        std::fs::write(&older, "{}").expect("older log");
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(&newer, "{}").expect("newer log");
        std::fs::write(&ignored, "{}").expect("ignored file");

        let latest = latest_session_log(temp.path())
            .expect("latest log")
            .expect("session log path");

        assert_eq!(latest, newer);
    }
}
