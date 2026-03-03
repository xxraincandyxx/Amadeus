use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub purple: Color,
    pub cyan: Color,
    pub green: Color,
    pub pink: Color,
    pub orange: Color,
    pub red: Color,
    pub comment: Color,
    pub current_line: Color,
    pub selection: Color,
    pub user_msg: Color,
    pub assistant_msg: Color,
    pub tool_bg: Color,
    pub border: Color,
}

impl Theme {
    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            purple: Color::Rgb(189, 147, 249),
            cyan: Color::Rgb(139, 233, 253),
            green: Color::Rgb(80, 250, 123),
            pink: Color::Rgb(255, 121, 198),
            orange: Color::Rgb(255, 184, 108),
            red: Color::Rgb(255, 85, 85),
            comment: Color::Rgb(98, 114, 164),
            current_line: Color::Rgb(68, 71, 90),
            selection: Color::Rgb(68, 71, 90),
            user_msg: Color::Rgb(139, 233, 253),
            assistant_msg: Color::Rgb(189, 147, 249),
            tool_bg: Color::Rgb(33, 35, 44),
            border: Color::Rgb(68, 71, 90),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dracula()
    }
}

pub static THEME: Theme = Theme {
    bg: Color::Rgb(40, 42, 54),
    fg: Color::Rgb(248, 248, 242),
    purple: Color::Rgb(189, 147, 249),
    cyan: Color::Rgb(139, 233, 253),
    green: Color::Rgb(80, 250, 123),
    pink: Color::Rgb(255, 121, 198),
    orange: Color::Rgb(255, 184, 108),
    red: Color::Rgb(255, 85, 85),
    comment: Color::Rgb(98, 114, 164),
    current_line: Color::Rgb(68, 71, 90),
    selection: Color::Rgb(68, 71, 90),
    user_msg: Color::Rgb(139, 233, 253),
    assistant_msg: Color::Rgb(189, 147, 249),
    tool_bg: Color::Rgb(33, 35, 44),
    border: Color::Rgb(68, 71, 90),
};

pub struct Palette;

impl Palette {
    pub fn header() -> String {
        "🎣".to_string()
    }

    pub fn prompt() -> String {
        ">> ".to_string()
    }

    pub fn command(cmd: &str) -> String {
        format!("$ {}", cmd)
    }

    pub fn tool_result() -> String {
        "✓".to_string()
    }

    pub fn error(msg: &str) -> String {
        format!("✗ {}", msg)
    }

    pub fn info(msg: &str) -> String {
        format!("ℹ {}", msg)
    }
}

pub fn print_command(cmd: &str) {
    println!("{}", Palette::command(cmd));
}

pub fn print_tool_result(output: &str) {
    if output.is_empty() {
        println!("(empty)");
        return;
    }

    let truncated = if output.len() > 50000 {
        &output[..50000]
    } else {
        output
    };

    println!("{}", truncated);
}
