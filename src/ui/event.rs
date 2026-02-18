use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent};
use std::time::Duration;

pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
}

pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    pub fn next(&self) -> std::io::Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        Ok(AppEvent::Key(key))
                    } else {
                        self.next()
                    }
                }
                Event::Mouse(mouse) => Ok(AppEvent::Mouse(mouse)),
                Event::Resize(cols, rows) => Ok(AppEvent::Resize(cols, rows)),
                _ => self.next(),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
