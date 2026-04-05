// @amadeus-header
// summary: Runnable example for test inline usage.
// layer: example
// status: experimental
// feature_flags:
// - full
// provides:
// - module: example::test_inline
// uses:
// - runtime: ratatui terminal rendering
// - runtime: crossterm terminal events
// - runtime: anyhow error handling
// invariants:
// - Example code remains runnable against the current public API.
// side_effects: none
// tests:
// - cmd: cargo run --example test_inline --features full
// @end-amadeus-header

use anyhow::Result;
use crossterm::{
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal, TerminalOptions, Viewport};

fn main() -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(Clear(ClearType::All))?;
    let (cols, rows) = terminal::size()?;

    let backend = CrosstermBackend::new(stdout);

    // We compute the fixed rect at the bottom
    let height: u16 = 10;
    let mut term = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, rows.saturating_sub(height), cols, height)),
        },
    )?;

    term.draw(|f: &mut ratatui::Frame| {
        let size = f.area();
        f.render_widget(
            ratatui::widgets::Block::default()
                .title("Test Fixed Viewport")
                .borders(ratatui::widgets::Borders::ALL),
            size,
        );
    })?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    terminal::disable_raw_mode()?;
    Ok(())
}
