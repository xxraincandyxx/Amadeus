// @amadeus-header
// summary: Starts the native Tauri shell and its bundled local Amadeus API server.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: run
// - runtime: Tauri desktop application
// uses:
// - fn: tauri::App::run
// - process: bundled amadeus-server sidecar
// - artifact: tauri.conf.json
// invariants:
// - An existing server on the desktop API port is reused instead of replaced.
// - The bundled server is terminated when the desktop window closes.
// side_effects:
// - Creates and runs a native application window.
// - Starts a local Amadeus HTTP API process when the configured port is free.
// tests:
// - cmd: npm run desktop:build
// @end-amadeus-header

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::Manager;

const DESKTOP_API_PORT: u16 = 3000;

struct ServerProcess(Mutex<Option<Child>>);

fn server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DESKTOP_API_PORT)
}

fn server_is_online() -> bool {
    TcpStream::connect_timeout(&server_address(), Duration::from_millis(150)).is_ok()
}

fn development_server_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/amadeus")
}

fn bundled_server_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable
        .parent()
        .ok_or_else(|| "desktop executable has no parent directory".to_string())?;
    Ok(directory.join("amadeus-server"))
}

fn server_path() -> Result<PathBuf, String> {
    let bundled = bundled_server_path()?;
    if bundled.exists() {
        return Ok(bundled);
    }
    let development = development_server_path();
    if development.exists() {
        return Ok(development);
    }
    Err("bundled Amadeus server executable is missing".to_string())
}

fn server_workdir() -> Option<PathBuf> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    workspace
        .join(".amadeus/settings.json")
        .exists()
        .then_some(workspace)
}

fn start_server() -> Result<Option<Child>, String> {
    if server_is_online() {
        return Ok(None);
    }

    let mut command = Command::new(server_path()?);
    command
        .args(["--server", &DESKTOP_API_PORT.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(workdir) = server_workdir() {
        command.current_dir(workdir);
    }
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if server_is_online() {
            return Ok(Some(child));
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("the bundled Amadeus server did not become ready".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let child = start_server().map_err(std::io::Error::other)?;
            app.manage(ServerProcess(Mutex::new(child)));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Amadeus desktop application");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Ok(mut process) = app.state::<ServerProcess>().0.lock() {
                if let Some(mut child) = process.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    });
}
