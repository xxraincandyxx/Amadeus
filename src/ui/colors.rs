use colored::*;

pub struct Palette;

impl Palette {
    pub fn header() -> String {
        "🎣".purple().bold().to_string()
    }

    pub fn prompt() -> String {
        ">> ".purple().bold().to_string()
    }

    pub fn command(cmd: &str) -> String {
        format!("$ {}", cmd).purple().to_string()
    }

    pub fn tool_result() -> String {
        "✓".truecolor(255, 0, 255).to_string()
    }

    pub fn error(msg: &str) -> String {
        format!("✗ {}", msg).red().bold().to_string()
    }

    pub fn info(msg: &str) -> String {
        format!("ℹ {}", msg).cyan().to_string()
    }
}

pub fn print_command(cmd: &str) {
    println!("{}", Palette::command(cmd));
}

pub fn print_tool_result(output: &str) {
    if output.is_empty() {
        println!("(empty)");
    } else {
        let truncated = if output.len() > 50000 {
            &output[..50000]
        } else {
            output
        };
        println!("{}", truncated);
    }
}
