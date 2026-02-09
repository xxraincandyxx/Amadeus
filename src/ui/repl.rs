use std::sync::Arc;
use std::io::{self, Write};
use tokio::sync::RwLock;
use crate::agent::loop_agent::Agent;
use crate::ui::colors::Palette;

pub struct Repl {
    agent: Agent,
}

impl Repl {
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let history = Arc::new(RwLock::new(Vec::new()));

        println!("{}", Palette::header());
        println!("Type 'q', 'exit', or press Ctrl+D to quit.\n");

        loop {
            print!("{}", Palette::prompt());
            io::stdout().flush()?;

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", Palette::error(&format!("Input error: {}", e)));
                    continue;
                }
            }

            let input = input.trim();

            if input.is_empty() || input == "q" || input == "exit" {
                break;
            }

            if let Err(e) = self.agent.run(input, Arc::clone(&history)).await {
                eprintln!("{}", Palette::error(&format!("Agent error: {}", e)));
            }

            println!();
        }

        println!("Goodbye!");
        Ok(())
    }
}
