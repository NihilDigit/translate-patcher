use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use translate_patcher_core::{APP_DESCRIPTION, APP_NAME};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);

            let title = Paragraph::new(Line::from(vec![
                Span::styled(APP_NAME, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::raw(APP_DESCRIPTION),
            ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM));
            frame.render_widget(title, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from("MVP skeleton is ready."),
                Line::from(""),
                Line::from(
                    "Next steps: scan current folder, choose ASAR/JSON, preview, patch, restore.",
                ),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("translate-patcher"),
            );
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new("Press q or Esc to quit")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::TOP));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()? {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                break;
            }
        }
    }

    Ok(())
}
