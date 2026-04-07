// @amadeus-header
// summary: TUI module code for event.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::event
// - type: crate::ui::event::AppEvent
// - type: crate::ui::event::EventHandler
// uses:
// - runtime: tokio async runtime
// - runtime: crossterm terminal events
// - runtime: tokio task scheduling
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Spawns asynchronous tasks.
// - Sends or receives messages across async channels.
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
}

pub struct EventHandler {
    receiver: mpsc::Receiver<AppEvent>,
    task: tokio::task::JoinHandle<()>,
    tick_rate_tx: mpsc::Sender<Duration>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(16);
        let (tick_rate_tx, mut tick_rate_rx) = mpsc::channel(1);

        let task = tokio::spawn(async move {
            let mut current_tick_rate = tick_rate;
            loop {
                if tx.is_closed() {
                    break;
                }

                // Check for tick rate updates
                match tick_rate_rx.try_recv() {
                    Ok(new_rate) => current_tick_rate = new_rate,
                    Err(mpsc::error::TryRecvError::Empty) => {}
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }

                if event::poll(current_tick_rate).unwrap_or(false) {
                    if let Some(app_event) = Self::read_event() {
                        if tx.send(app_event).await.is_err() {
                            break;
                        }
                    }
                } else if tx.send(AppEvent::Tick).await.is_err() {
                    break;
                }
            }
        });

        Self {
            receiver: rx,
            task,
            tick_rate_tx,
        }
    }

    pub fn set_tick_rate(&self, tick_rate: Duration) {
        let _ = self.tick_rate_tx.try_send(tick_rate);
    }

    fn read_event() -> Option<AppEvent> {
        match event::read().ok()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    Some(AppEvent::Key(key))
                } else {
                    None
                }
            }
            Event::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
            Event::Resize(cols, rows) => Some(AppEvent::Resize(cols, rows)),
            _ => None,
        }
    }

    pub async fn next(&mut self) -> std::io::Result<AppEvent> {
        self.receiver.recv().await.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Event channel closed")
        })
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        self.task.abort();
    }
}
