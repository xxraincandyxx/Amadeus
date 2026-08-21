// @amadeus-header
// summary: Unit tests for TUI application sessions, dialogs, commands, events, rewind, and rendering behavior.
// layer: test
// status: test-only
// feature_flags:
// - tui
// provides:
// - module: crate::ui::app::tests
// uses:
// - type: crate::ui::app::App
// - runtime: tokio async runtime
// invariants:
// - Assertions preserve current session and terminal behavior.
// side_effects:
// - Reads and writes temporary filesystem state.
// tests:
// - cmd: cargo test -p tui --all-features
// @end-amadeus-header

use super::{App, MonitorStatus, Session, SlashDialogState, ToolMonitorState};
use crate::agent::messages::{ContentBlock, Message};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    backend::{CrosstermBackend, TestBackend},
    Terminal,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::benchmark::case::MockScript;
use crate::benchmark::mock::BenchmarkMockClient;

fn test_app() -> App<BenchmarkMockClient> {
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    App::new(agent, PathBuf::from("."), "test-model".to_string())
}

fn active_session_mut(app: &mut App<BenchmarkMockClient>) -> &mut Session<BenchmarkMockClient> {
    let idx = app.active_idx;
    &mut app.sessions[idx]
}

#[test]
fn tool_monitor_clears_when_all_tools_complete() {
    let mut monitor = ToolMonitorState::new(16);
    monitor.start_tool("root".to_string(), "bash".to_string(), None, None);
    monitor.complete("root", "done".to_string(), false);

    assert!(monitor.clear_if_idle());
    assert!(!monitor.has_content());
}

#[test]
fn tool_monitor_stays_visible_while_nested_tool_runs() {
    let mut monitor = ToolMonitorState::new(16);
    monitor.start_tool("root".to_string(), "sub_agent".to_string(), None, None);
    monitor.start_tool(
        "root::child".to_string(),
        "bash".to_string(),
        Some("root".to_string()),
        None,
    );
    monitor.complete("root", "waiting".to_string(), false);

    assert!(!monitor.clear_if_idle());
    assert!(monitor.has_content());
    assert_eq!(
        monitor.nodes.get("root::child").map(|node| node.status),
        Some(MonitorStatus::Pending)
    );
}

#[test]
fn active_snapshot_prefers_running_selection_then_running_descendant() {
    let mut monitor = ToolMonitorState::new(16);
    monitor.start_tool("root".to_string(), "sub_agent".to_string(), None, None);
    monitor.start_tool(
        "root::child".to_string(),
        "bash".to_string(),
        Some("root".to_string()),
        None,
    );
    monitor.complete("root", "delegating".to_string(), false);

    let snapshot = monitor.active_snapshot().expect("expected active snapshot");
    assert_eq!(snapshot.tool_name, "bash");
    assert_eq!(snapshot.running_count, 1);
}

#[test]
fn key_chord_label_formats_modifier_shortcuts() {
    assert_eq!(
        Session::<crate::client::openai::OpenAIClient>::key_chord_label(
            crossterm::event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL,)
        ),
        Some("ctrl+x".to_string())
    );
    assert_eq!(
        Session::<crate::client::openai::OpenAIClient>::key_chord_label(
            crossterm::event::KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE,)
        ),
        Some("i".to_string())
    );
}

#[test]
fn switch_session_advances_to_next() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);
    assert_eq!(app.active_idx, 0);

    app.switch_session(1);
    assert_eq!(app.active_idx, 1);
    assert!(app.pending_terminal_reset);
}

#[test]
fn switch_session_wraps_to_previous() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);
    app.active_idx = 1;

    app.switch_session(0);
    assert_eq!(app.active_idx, 0);
}

#[test]
fn switch_session_sets_pending_reset() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);
    assert!(!app.pending_terminal_reset);

    app.switch_session(1);
    assert!(app.pending_terminal_reset);
    assert_eq!(app.active_idx, 1);
}

#[test]
fn switch_to_next_session_wraps() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);
    app.active_idx = 1;

    assert!(app.switch_to_next_session());
    assert_eq!(app.active_idx, 0);
}

#[test]
fn switch_to_previous_session_wraps() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);

    assert!(app.switch_to_previous_session());
    assert_eq!(app.active_idx, 1);
}

#[test]
fn session_tabs_bracket_only_the_active_session() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session2.last_error = Some("boom".to_string());
    app.sessions.push(session2);

    assert_eq!(app.build_session_tabs(), "[root] session1!");
    app.active_idx = 1;
    assert_eq!(app.build_session_tabs(), "root [session1!]");
}

#[test]
fn empty_sessions_can_redraw_immediately_after_switch() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    app.sessions.push(session2);
    app.switch_session(1);

    assert!(app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn populated_sessions_require_full_switch_reset() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session2.messages.add_user("hello?".to_string());
    app.sessions.push(session2);
    app.switch_session(1);

    assert!(!app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn switching_away_from_populated_session_to_empty_allows_immediate_redraw() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session2.messages.add_user("hello?".to_string());
    app.sessions.push(session2);
    app.active_idx = 1;
    app.switch_session(0);

    assert!(!app.pending_reset_from_history);
    assert!(app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn spawn_new_session_creates_an_empty_active_session_ready_for_immediate_redraw() {
    let mut app = test_app();

    app.spawn_new_session()
        .expect("spawn new session should succeed");

    assert_eq!(app.active_idx, 1);
    assert_eq!(app.sessions[1].session_label, "session1");
    assert!(app.sessions[1].messages.is_empty());
    assert!(app.should_finish_session_switch_immediately());
}

#[test]
fn switching_from_populated_to_empty_session_allows_immediate_redraw() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session1 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session1.messages.add_user("hello?".to_string());
    app.sessions.push(session1);
    app.active_idx = 1;

    app.switch_session(0);

    assert!(!app.pending_reset_from_history);
    assert!(app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn switching_between_empty_sessions_allows_immediate_redraw() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    app.sessions.push(session2);

    app.switch_session(1);

    assert!(!app.pending_reset_from_history);
    assert!(app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn switching_to_populated_session_defers_redraw() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session2.messages.add_user("hello?".to_string());
    app.sessions.push(session2);

    app.switch_session(1);

    assert!(!app.pending_reset_from_history);
    assert!(!app.should_finish_session_switch_immediately());
    assert!(app.pending_terminal_reset);
}

#[test]
fn recycle_terminal_after_switch_replays_populated_session_history() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "session1".to_string(),
        1,
    );
    session2.messages.add_user("hello?".to_string());
    session2.messages.add_assistant("hi".to_string());
    let initial_lines = session2.messages.take_unrendered_lines(80);
    assert!(!initial_lines.is_empty());
    app.sessions.push(session2);

    app.switch_session(1);

    let _terminal = app
        .recycle_terminal_after_session_switch(terminal)
        .expect("session switch replay should keep the terminal alive");

    assert!(!app.pending_terminal_reset);
    let replayed = app.sessions[1].messages.take_unrendered_lines(80);
    let rendered = replayed
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Amadeus v0.1.0"));
    assert!(rendered.contains("hello?"));
    assert!(rendered.contains("hi"));
    assert!(rendered.contains("Premium CLI Coding Interface"));
}

#[test]
fn switch_to_parent_session_moves_to_parent() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        1,
    );
    session2.parent_session_id = Some(0);
    app.sessions.push(session2);
    app.active_idx = 1;

    assert!(app.switch_to_parent_session());
    assert_eq!(app.active_idx, 0);
}

#[test]
fn switch_to_parent_session_noops_for_root() {
    let mut app = test_app();

    assert!(!app.switch_to_parent_session());
    assert_eq!(app.active_idx, 0);
}

#[test]
fn switch_to_child_session_moves_to_first_direct_child() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let mut child = Session::new(
        agent.clone(),
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        1,
    );
    child.parent_session_id = Some(0);
    let mut grandchild = Session::new(
        agent.clone(),
        PathBuf::from("."),
        "test-model".to_string(),
        2,
        "grandchild".to_string(),
        2,
    );
    grandchild.parent_session_id = Some(1);
    let mut second_child = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        3,
        "child-2".to_string(),
        1,
    );
    second_child.parent_session_id = Some(0);
    app.sessions.push(child);
    app.sessions.push(grandchild);
    app.sessions.push(second_child);

    assert!(app.switch_to_child_session());
    assert_eq!(app.active_idx, 1);
}

#[test]
fn switch_to_child_session_noops_without_child() {
    let mut app = test_app();

    assert!(!app.switch_to_child_session());
    assert_eq!(app.active_idx, 0);
}

#[test]
fn resolved_done_text_uses_subagent_output_when_result_is_empty() {
    let mut last_subagent_output = Some("delegated answer".to_string());
    assert_eq!(
        Session::<crate::client::openai::OpenAIClient>::resolve_done_text(
            String::new(),
            &mut last_subagent_output,
        ),
        Some("delegated answer".to_string())
    );
}

#[tokio::test]
async fn typing_in_normal_mode_restores_input_focus() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.mode = super::AppMode::Normal;

    session
        .handle_normal_key(KeyEvent::from(KeyCode::Char('h')))
        .await
        .expect("normal key handling should succeed");

    assert_eq!(session.mode, super::AppMode::Input);
    assert_eq!(session.input.get_input(), "h");
}

#[tokio::test]
async fn quit_shortcut_still_works_in_normal_mode() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.mode = super::AppMode::Normal;

    session
        .handle_normal_key(KeyEvent::from(KeyCode::Char('q')))
        .await
        .expect("normal key handling should succeed");

    assert!(session.should_quit);
    assert!(session.input.get_input().is_empty());
}

#[tokio::test]
async fn question_mark_opens_shortcuts_overlay_when_input_is_empty() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);

    session
        .handle_input_key(KeyEvent::from(KeyCode::Char('?')), &mut terminal)
        .await
        .expect("question mark handling should succeed");

    assert!(session.input.is_shortcuts_visible());
    assert!(session.input.get_input().is_empty());
}

#[tokio::test]
async fn ctrl_b_and_ctrl_f_match_left_and_right_arrow_behavior() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);

    for ch in "abc".chars() {
        session.input.handle_char(ch);
    }

    session
        .handle_input_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .await
        .expect("ctrl+b should move left");
    session.input.handle_char('x');
    assert_eq!(session.input.get_input(), "abxc");

    session
        .handle_input_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .await
        .expect("ctrl+f should move right");
    session.input.handle_char('y');
    assert_eq!(session.input.get_input(), "abxcy");
}

#[tokio::test]
async fn ctrl_p_and_ctrl_n_move_cursor_up_and_down() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);

    for ch in "line1\nline2\nline3".chars() {
        session.input.handle_char(ch);
    }

    session
        .handle_input_key(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .await
        .expect("ctrl+p should move cursor up");

    session
        .handle_input_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .await
        .expect("ctrl+n should move cursor down");
}

#[tokio::test]
async fn slash_hooks_opens_dialog() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    for ch in "/hooks".chars() {
        session.input.handle_char(ch);
    }

    session.submit_input().await.expect("hooks command");

    assert!(matches!(session.mode, super::AppMode::SlashDialog));
    match session.slash_dialog.as_ref() {
        Some(super::SlashDialogState::Hooks(state)) => {
            assert_eq!(state.events.len(), 3);
            assert_eq!(state.events[0], crate::hooks::HookEvent::PreToolUse);
        }
        other => panic!("expected hooks dialog, got {other:?}"),
    }
}

#[tokio::test]
async fn slash_agents_switches_to_selected_existing_session() {
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    app.sessions.push(Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        1,
    ));
    app.active_idx = 1;

    for ch in "/agents".chars() {
        active_session_mut(&mut app).input.handle_char(ch);
    }
    active_session_mut(&mut app)
        .submit_input()
        .await
        .expect("agents command");

    assert_eq!(
        active_session_mut(&mut app).pending_agent_dialog,
        Some(super::AgentDialogAction::Open)
    );
    assert!(!app.process_pending_agent_dialog());
    match active_session_mut(&mut app).slash_dialog.as_ref() {
        Some(SlashDialogState::Agents(state)) => {
            assert_eq!(state.session_ids, vec![1, 0]);
            assert_eq!(state.dialog.selected(), Some(0));
        }
        other => panic!("expected agents dialog, got {other:?}"),
    }

    active_session_mut(&mut app)
        .handle_slash_dialog_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        .await
        .expect("select root agent");
    active_session_mut(&mut app)
        .handle_slash_dialog_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .expect("switch agent");

    assert!(app.process_pending_agent_dialog());
    assert_eq!(app.active_idx, 0);
}

#[tokio::test]
async fn slash_btw_uses_input_dropup_without_transcript_history() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    for ch in "/btw".chars() {
        session.input.handle_char(ch);
    }

    session.submit_input().await.expect("btw command");

    assert!(matches!(session.mode, super::AppMode::Input));
    assert!(session.stream_rx.is_none());
    assert!(session.input.btw_dropup_is_visible());
    assert_eq!(session.input.completion_height(), 2);
    let rendered = session
        .messages
        .take_unrendered_lines(80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!rendered.contains("Usage: /btw"));
}

#[tokio::test]
async fn submitting_prompt_clears_previous_btw_dropup() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.input.set_btw_dropup("/btw", "Usage: /btw", false);
    for ch in "hi".chars() {
        session.input.handle_char(ch);
    }

    session.submit_input().await.expect("prompt command");

    assert!(!session.input.btw_dropup_is_visible());
}

#[test]
fn render_shows_btw_dropup_inside_input_area() {
    let backend = TestBackend::new(90, 20);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.input.set_btw_dropup("/btw", "Usage: /btw", false);

    terminal
        .draw(|frame| session.render(frame))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let lines = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    assert!(lines.iter().any(|line| line.contains("/btw")));
    assert!(lines.iter().any(|line| line.contains("Usage:")));
    assert!(lines.iter().any(|line| line.contains("└")));
}

#[tokio::test]
async fn slash_rewind_restores_checkpoint() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session
        .capture_rewind_checkpoint("before".to_string())
        .await
        .expect("checkpoint");

    {
        let history = session.agent.history();
        let mut history = history.write().await;
        history.push(Message::user("hello"));
        history.push(Message::assistant(vec![ContentBlock::Text {
            text: "world".to_string(),
        }]));
    }
    session.messages.add_user("hello".to_string());
    session.messages.add_assistant("world".to_string());

    for ch in "/rewind 1".chars() {
        session.input.handle_char(ch);
    }

    session.submit_input().await.expect("rewind command");

    let history = session.agent.history();
    let history = history.read().await;
    assert!(history.is_empty());
    assert!(session.pending_transcript_reset);
    let rendered = session
        .messages
        .take_unrendered_lines(80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!rendered.contains("hello"));
    assert!(!rendered.contains("world"));
}

#[tokio::test]
async fn slash_export_writes_file_to_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("conversation.md");

    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.messages.add_user("hello there".to_string());
    session.messages.add_assistant("general kenobi".to_string());
    for ch in format!("/export {}", target.display()).chars() {
        session.input.handle_char(ch);
    }
    session.submit_input().await.expect("export command");

    assert!(target.exists());
    let body = fs::read_to_string(&target).expect("read export");
    assert!(body.contains("# Amadeus Conversation Export"));
    assert!(body.contains("hello there"));
    assert!(body.contains("general kenobi"));
}

#[tokio::test]
async fn slash_export_default_path_lands_under_amadeus_exports() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workdir = dir.path().to_path_buf();

    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.workdir = workdir.clone();
    session.messages.add_user("hi".to_string());
    for ch in "/export".chars() {
        session.input.handle_char(ch);
    }
    session.submit_input().await.expect("export command");

    let exports_dir = workdir.join(".amadeus").join("exports");
    assert!(
        exports_dir.exists(),
        "default export directory should exist"
    );
    let written: Vec<_> = fs::read_dir(&exports_dir)
        .expect("read exports")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(written.len(), 1, "exactly one markdown export should land");
}

#[tokio::test]
async fn slash_export_appends_timestamp_on_collision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("export.md");

    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.messages.add_user("first".to_string());
    for ch in format!("/export {}", target.display()).chars() {
        session.input.handle_char(ch);
    }
    session.submit_input().await.expect("first export");
    assert!(target.exists());

    let mut app2 = test_app();
    let session2 = active_session_mut(&mut app2);
    session2.messages.add_user("second".to_string());
    for ch in format!("/export {}", target.display()).chars() {
        session2.input.handle_char(ch);
    }
    session2.submit_input().await.expect("second export");

    let siblings: Vec<_> = fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    let with_prefix = siblings
        .iter()
        .filter(|name| name.starts_with("export-") && name.ends_with(".md"))
        .count();
    assert!(
        with_prefix >= 1,
        "collision should produce a timestamped sibling, got {siblings:?}"
    );
}

#[test]
fn app_exposes_export_on_exit_setter() {
    let mut app = test_app();
    assert!(app.take_pending_export().is_none());
    app.set_export_on_exit(Some(PathBuf::from("/tmp/out.md")));
    let taken = app.take_pending_export();
    assert_eq!(taken, Some(PathBuf::from("/tmp/out.md")));
    assert!(app.take_pending_export().is_none());
}

#[test]
fn code_snapshot_summary_counts_changed_lines_and_files() {
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
-    old();
+    new();
+    extra();
 }
";

    let summary = Session::<BenchmarkMockClient>::summarize_code_snapshot(diff);

    assert_eq!(summary.files, vec!["src/main.rs"]);
    assert_eq!(summary.additions, 2);
    assert_eq!(summary.deletions, 1);
}

#[tokio::test]
async fn rewind_dialog_confirm_step_is_shown_for_selected_checkpoint() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session
        .capture_rewind_checkpoint("before".to_string())
        .await
        .expect("checkpoint");

    session.show_rewind_dialog().await.expect("rewind dialog");
    session
        .submit_slash_dialog()
        .await
        .expect("select checkpoint");

    assert!(matches!(
        session.slash_dialog,
        Some(SlashDialogState::RewindConfirm(_))
    ));
}

#[tokio::test]
async fn restore_rewind_code_restores_tracked_git_diff() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "Test User"]);
    fs::write(root.join("tracked.txt"), "one\n").expect("write tracked");
    run_git(root, &["add", "tracked.txt"]);
    run_git(root, &["commit", "-m", "initial"]);

    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let mut config = Config::default();
    config.workdir = root.to_path_buf();
    let agent = Agent::builder(client, Arc::new(config)).build();
    let mut session = Session::new(
        agent,
        root.to_path_buf(),
        "test-model".to_string(),
        0,
        "root".to_string(),
        0,
    );

    session
        .capture_rewind_checkpoint("before edit".to_string())
        .await
        .expect("checkpoint");
    fs::write(root.join("tracked.txt"), "one\ntwo\n").expect("modify tracked");

    let entry = session.rewind_checkpoints[0].clone();
    session
        .restore_rewind_code(&entry)
        .expect("restore code snapshot");

    assert_eq!(
        fs::read_to_string(root.join("tracked.txt")).expect("read tracked"),
        "one\n"
    );
}

#[test]
fn rewind_transcript_reset_inserts_spacer_lines_and_clears_pending_flag() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.pending_transcript_reset = true;

    session
        .apply_pending_transcript_reset(&mut terminal)
        .expect("rewind reset should succeed");

    assert!(!session.pending_transcript_reset);
}

#[test]
fn cursor_position_timeout_is_recoverable_terminal_insert_error() {
    let error =
        std::io::Error::other("The cursor position could not be read within a normal duration");

    assert!(Session::<BenchmarkMockClient>::is_recoverable_terminal_insert_error(&error));
}

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn tab_global_switches_session_when_completion_is_not_active() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);

    let handled = app
        .handle_global_key(KeyEvent::from(KeyCode::Tab), &mut terminal)
        .expect("global tab should succeed");

    assert!(handled);
    assert_eq!(app.active_idx, 1);
    assert!(!app.pending_terminal_reset);
}

#[test]
fn tab_global_finishes_session_reset_immediately() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);

    app.handle_global_key(KeyEvent::from(KeyCode::Tab), &mut terminal)
        .expect("global tab should succeed");

    assert_eq!(app.active_idx, 1);
    assert!(!app.pending_terminal_reset);
}

#[test]
fn ctrl_i_global_switches_session_as_tab_alias() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);

    let handled = app
        .handle_global_key(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .expect("global ctrl+i should succeed");

    assert!(handled);
    assert_eq!(app.active_idx, 1);
    assert!(!app.pending_terminal_reset);
}

#[test]
fn tab_global_defers_to_completion_popup() {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
    let agent = Agent::builder(client, Arc::new(Config::default())).build();
    let session2 = Session::new(
        agent,
        PathBuf::from("."),
        "test-model".to_string(),
        1,
        "child".to_string(),
        0,
    );
    app.sessions.push(session2);
    let session = active_session_mut(&mut app);
    session.input.handle_char('/');
    session.input.force_show_completion();

    let handled = app
        .handle_global_key(KeyEvent::from(KeyCode::Tab), &mut terminal)
        .expect("global tab should succeed");

    assert!(!handled);
    assert_eq!(app.active_idx, 0);
}

#[test]
fn render_includes_footer_and_status_bar() {
    let backend = TestBackend::new(90, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.footer.set_status_message("footer ok");
    session.status_bar.start();
    session.status_bar.update_input_tokens(128);
    session.status_bar.update_text("streamed output");

    terminal
        .draw(|frame| session.render(frame))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
        .collect::<String>();

    assert!(rendered.contains("footer ok"));
    assert!(rendered.contains("thinking"));
}

#[test]
fn render_keeps_composer_visible_when_citation_completion_is_open() {
    let backend = TestBackend::new(90, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = test_app();
    let session = active_session_mut(&mut app);

    session.input.handle_char('@');
    session.input.handle_char('r');
    session.input.handle_char('e');
    session.input.handle_char('v');

    terminal
        .draw(|frame| session.render(frame))
        .expect("render should succeed");

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
        .collect::<String>();

    assert!(rendered.contains("❯"));
    assert!(rendered.contains("@rev"));
}

#[test]
fn monitor_navigation_uses_ctrl_x_prefix_with_plain_followup_keys() {
    fn apply_navigation_step(
        monitor: &mut ToolMonitorState,
        prefix: &mut bool,
        key: KeyEvent,
    ) -> bool {
        if !monitor.has_content() {
            *prefix = false;
            return false;
        }

        if *prefix {
            *prefix = false;
            return match key.code {
                KeyCode::Char('i' | 'I') => {
                    monitor.select_previous();
                    true
                }
                KeyCode::Char('k' | 'K') => {
                    monitor.select_next();
                    true
                }
                KeyCode::Char('l' | 'L') => monitor.enter_selected(),
                KeyCode::Char('j' | 'J') => monitor.exit_parent(),
                _ => false,
            };
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('x' | 'X')) => {
                *prefix = true;
                true
            }
            _ => false,
        }
    }

    let mut monitor = ToolMonitorState::new(16);
    let mut prefix = false;
    monitor.start_tool("root-1".to_string(), "bash".to_string(), None, None);
    monitor.start_tool("root-2".to_string(), "read".to_string(), None, None);

    assert!(apply_navigation_step(
        &mut monitor,
        &mut prefix,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    ));
    assert!(prefix);
    assert!(apply_navigation_step(
        &mut monitor,
        &mut prefix,
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
    ));
    assert_eq!(monitor.selected_id.as_deref(), Some("root-2"));
    assert!(!prefix);
}

fn render_session_to_string(
    session: &mut Session<BenchmarkMockClient>,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| session.render(frame))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
        .collect()
}

#[test]
fn clicking_thought_header_reveals_hidden_content() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    session.messages.add_user("Hello".to_string());
    session.messages.update_thinking("private reasoning");
    session.messages.finalize_thinking();

    let collapsed = render_session_to_string(session, 90, 16);
    assert!(collapsed.contains("Thought for 1 second"));
    assert!(!collapsed.contains("private reasoning"));

    session.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: session.thought_area.x.saturating_add(1),
        row: session.thought_area.y.saturating_add(1),
        modifiers: KeyModifiers::NONE,
    });

    let expanded = render_session_to_string(session, 90, 16);
    assert!(expanded.contains("private reasoning"));
}
