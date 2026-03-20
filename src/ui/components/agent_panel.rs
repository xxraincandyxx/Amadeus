//! # Agent Panel Component
//!
//! UI component to display agent list and manage agents.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::ui::api_client::AgentInfo;

/// Render the agent panel in the sidebar.
pub fn render_agent_panel<B: ratatui::backend::Backend>(
    frame: &mut Frame<B>,
    area: Rect,
    agents: &[AgentInfo],
    active_index: usize,
) {
    // Build list items
    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, agent)| {
            let prefix = if i == active_index { "●" } else { "○" };
            let status = if agent.status == "running" {
                "[█]"
            } else {
                "[░]"
            };

            let content = format!("{} {} {} {}", prefix, agent.name, status, agent.profile);

            ListItem::new(content).style(if i == active_index {
                Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Agents").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

/// Render the agent creation dialog.
pub fn render_agent_dialog<B: ratatui::backend::Backend>(
    frame: &mut Frame<B>,
    area: Rect,
    selected_profile: usize,
) {
    let profiles = ["Default", "Debug", "Docs", "Code Review"];

    let mut content: Vec<Line> = vec![
        Line::from("Create New Agent"),
        Line::from(""),
        Line::from("Profile:"),
    ];

    // Render profile options
    for (i, profile) in profiles.iter().enumerate() {
        let prefix = if i == selected_profile { "▶" } else { " " };
        content.push(Line::from(format!("  {} {}", prefix, profile)));
    }

    content.push(Line::from(""));
    content.push(Line::from("[Enter] Create  [Esc] Cancel"));

    let paragraph = Paragraph::new(content)
        .block(Block::default().title("New Agent").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    // Center the dialog
    let dialog_width = 40.min(area.width);
    let dialog_height = 10.min(area.height);
    let x = (area.width - dialog_width) / 2;
    let y = (area.height - dialog_height) / 2;

    let dialog_area = Rect::new(area.x + x, area.y + y, dialog_width, dialog_height);
    frame.render_widget(paragraph, dialog_area);
}
