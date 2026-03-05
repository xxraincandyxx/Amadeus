use anyhow::Result;
use crossterm::{
    cursor,
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};

fn main() -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(Clear(ClearType::All))?;
    let (_, rows) = terminal::size()?;
    stdout.execute(cursor::MoveTo(0, rows.saturating_sub(1)))?;

    let backend = CrosstermBackend::new(stdout);

    // Start with height 2
    let mut term = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(2),
        },
    )?;

    term.draw(|f| {
        let size = f.area();
        f.render_widget(
            ratatui::widgets::Block::default()
                .title("Height 2")
                .borders(ratatui::widgets::Borders::ALL),
            size,
        );
    })?;

    std::thread::sleep(std::time::Duration::from_secs(1));

    // To resize, we drop the old term and move cursor UP by the old height
    drop(term);
    let mut stdout = std::io::stdout();
    stdout.execute(cursor::MoveUp(2))?;
    stdout.execute(Clear(ClearType::FromCursorDown))?;

    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(10),
        },
    )?;

    term.draw(|f| {
        let size = f.area();
        f.render_widget(
            ratatui::widgets::Block::default()
                .title("Height 10")
                .borders(ratatui::widgets::Borders::ALL),
            size,
        );
    })?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    terminal::disable_raw_mode()?;
    Ok(())
}
