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
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(16);

        tokio::spawn(async move {
            loop {
                if tx.is_closed() {
                    break;
                }

                if event::poll(tick_rate).unwrap_or(false) {
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

        Self { receiver: rx }
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
