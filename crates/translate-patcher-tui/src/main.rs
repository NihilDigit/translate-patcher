use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use translate_patcher_core::{
    patch::{apply_patch, preview_patch, PatchPreview, PatchReport},
    scan::{scan_from, ScanResult},
    APP_DESCRIPTION, APP_NAME,
};

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
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let mut app = App::new(cwd);

    loop {
        terminal.draw(|frame| app.render(frame))?;

        if let Event::Key(key) = event::read()? {
            if app.handle_key(key.code) {
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct App {
    cwd: PathBuf,
    scan: ScanResult,
    preview: Option<PatchPreview>,
    report: Option<PatchReport>,
    screen: Screen,
    selected_action: usize,
    picker: Option<FilePicker>,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Select,
    Confirm,
    Picker,
    Done,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickMode {
    Asar,
    Json,
}

impl PickMode {
    fn title(self) -> &'static str {
        match self {
            Self::Asar => "Choose ASAR",
            Self::Json => "Choose JSON",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Asar => "asar",
            Self::Json => "json",
        }
    }
}

impl App {
    fn new(cwd: PathBuf) -> Self {
        let scan = scan_from(&cwd);
        let mut app = Self {
            cwd,
            scan,
            preview: None,
            report: None,
            screen: Screen::Select,
            selected_action: 0,
            picker: None,
            error: None,
        };
        app.refresh_preview();
        app
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match self.screen {
            Screen::Select => self.handle_select_key(code),
            Screen::Confirm => self.handle_confirm_key(code),
            Screen::Picker => self.handle_picker_key(code),
            Screen::Done => self.handle_done_key(code),
            Screen::Error => self.handle_error_key(code),
        }
    }

    fn handle_select_key(&mut self, code: KeyCode) -> bool {
        let action_count = if self.preview.is_some() { 4 } else { 3 };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_action = self.selected_action.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.selected_action = (self.selected_action + 1).min(action_count - 1);
            }
            KeyCode::Enter => match self.selected_action {
                0 if self.preview.is_some() => self.screen = Screen::Confirm,
                0 => self.open_picker(PickMode::Asar),
                1 if self.preview.is_some() => self.open_picker(PickMode::Asar),
                1 => self.open_picker(PickMode::Json),
                2 if self.preview.is_some() => self.open_picker(PickMode::Json),
                2 => return true,
                _ => return true,
            },
            _ => {}
        }
        false
    }

    fn handle_confirm_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Esc | KeyCode::Char('b') => self.screen = Screen::Select,
            KeyCode::Enter => {
                if let Some(preview) = &self.preview {
                    match apply_patch(preview) {
                        Ok(report) => {
                            self.report = Some(report);
                            self.screen = Screen::Done;
                        }
                        Err(err) => {
                            self.error = Some(format!("{err:#}"));
                            self.screen = Screen::Error;
                        }
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn handle_picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = &mut self.picker else {
            self.screen = Screen::Select;
            return false;
        };

        match code {
            KeyCode::Esc => {
                self.picker = None;
                self.screen = Screen::Select;
            }
            KeyCode::Up | KeyCode::Char('k') => picker.move_up(),
            KeyCode::Down | KeyCode::Char('j') => picker.move_down(),
            KeyCode::Backspace => picker.go_up(),
            KeyCode::Enter => {
                if let Some(selected) = picker.selected_path() {
                    if selected.is_dir() {
                        picker.enter_dir(selected);
                    } else {
                        match picker.mode {
                            PickMode::Asar => self.scan.selected_asar = Some(selected),
                            PickMode::Json => self.scan.selected_json = Some(selected),
                        }
                        self.picker = None;
                        self.screen = Screen::Select;
                        self.selected_action = 0;
                        self.refresh_preview();
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn handle_done_key(&mut self, code: KeyCode) -> bool {
        matches!(code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter)
    }

    fn handle_error_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') => true,
            KeyCode::Esc | KeyCode::Enter => {
                self.screen = Screen::Select;
                false
            }
            _ => false,
        }
    }

    fn open_picker(&mut self, mode: PickMode) {
        let current = match mode {
            PickMode::Asar => self
                .scan
                .selected_asar
                .as_ref()
                .and_then(|path| path.parent())
                .unwrap_or(&self.cwd),
            PickMode::Json => self
                .scan
                .selected_json
                .as_ref()
                .and_then(|path| path.parent())
                .unwrap_or(&self.cwd),
        };
        self.picker = Some(FilePicker::new(
            self.scan.scan_root.clone(),
            current.to_path_buf(),
            mode,
        ));
        self.screen = Screen::Picker;
    }

    fn refresh_preview(&mut self) {
        self.preview = match (&self.scan.selected_asar, &self.scan.selected_json) {
            (Some(asar), Some(json)) => preview_patch(asar, json).ok(),
            _ => None,
        };
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        render_header(frame, layout[0]);
        match self.screen {
            Screen::Select => self.render_select(frame, layout[1]),
            Screen::Confirm => self.render_confirm(frame, layout[1]),
            Screen::Picker => self.render_picker(frame, layout[1]),
            Screen::Done => self.render_done(frame, layout[1]),
            Screen::Error => self.render_error(frame, layout[1]),
        }
        render_footer(frame, layout[2], self.screen);
    }

    fn render_select(&self, frame: &mut Frame, area: Rect) {
        let asar = self
            .scan
            .selected_asar
            .as_deref()
            .map(display_path)
            .unwrap_or_else(|| "not selected".to_string());
        let json = self
            .scan
            .selected_json
            .as_deref()
            .map(display_path)
            .unwrap_or_else(|| "not selected".to_string());

        let mut lines = vec![
            Line::from("Found game resources"),
            Line::from(""),
            Line::from(vec![
                Span::raw("Game folder        "),
                Span::raw(display_path(&self.cwd)),
            ]),
            Line::from(vec![Span::raw("Resource pack      "), Span::raw(asar)]),
            Line::from(vec![Span::raw("Translation file   "), Span::raw(json)]),
            Line::from(""),
        ];

        if let Some(preview) = &self.preview {
            lines.extend([
                Line::from(vec![
                    Span::raw("Engine             "),
                    Span::raw(preview.backend.label()),
                ]),
                Line::from(vec![
                    Span::raw("Translation entries  "),
                    Span::raw(preview.translation_entries.to_string()),
                ]),
                Line::from(vec![
                    Span::raw("Scenario files       "),
                    Span::raw(preview.scenario_files.to_string()),
                ]),
                Line::from(vec![
                    Span::raw("Estimated matches    "),
                    Span::raw(preview.estimated_matches.to_string()),
                ]),
                Line::from(""),
            ]);
            lines.push(action_line(
                &["Patch game", "Change ASAR", "Change JSON", "Quit"],
                self.selected_action,
            ));
        } else {
            lines.extend([
                Line::from("Select an ASAR and a JSON translation file to continue."),
                Line::from(""),
                action_line(
                    &["Change ASAR", "Change JSON", "Quit"],
                    self.selected_action,
                ),
            ]);
        }

        frame.render_widget(panel(lines, "Select files"), area);
    }

    fn render_confirm(&self, frame: &mut Frame, area: Rect) {
        let Some(preview) = &self.preview else {
            frame.render_widget(
                panel(vec![Line::from("No valid patch preview.")], "Confirm patch"),
                area,
            );
            return;
        };

        let lines = vec![
            Line::from("translate-patcher will modify:"),
            Line::from(""),
            Line::from(format!("  {}", preview.asar_path.display())),
            Line::from(""),
            Line::from("A backup will be created first:"),
            Line::from(""),
            Line::from(format!("  {}", preview.backup_path.display())),
            Line::from(""),
            Line::from("Mode"),
            Line::from("  Conservative: Tyrano scenario text and character names only."),
            Line::from(""),
            action_line(&["Start patch", "Back", "Quit"], 0),
        ];
        frame.render_widget(panel(lines, "Confirm patch"), area);
    }

    fn render_picker(&self, frame: &mut Frame, area: Rect) {
        let Some(picker) = &self.picker else {
            return;
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(5)])
            .split(area);

        let summary = vec![
            Line::from(format!("Root  {}", picker.root.display())),
            Line::from(format!("Path  {}", picker.current.display())),
            Line::from(format!(
                "Showing directories and *.{}",
                picker.mode.extension()
            )),
        ];
        frame.render_widget(panel(summary, picker.mode.title()), chunks[0]);

        let items: Vec<ListItem> = picker
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let marker = if index == picker.selected { "> " } else { "  " };
                let suffix = if entry.is_dir { "/" } else { "" };
                ListItem::new(format!("{marker}{}{suffix}", entry.name))
            })
            .collect();
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL)),
            chunks[1],
        );
    }

    fn render_done(&self, frame: &mut Frame, area: Rect) {
        let Some(report) = &self.report else {
            frame.render_widget(
                panel(vec![Line::from("No report available.")], "Patch complete"),
                area,
            );
            return;
        };
        let lines = vec![
            Line::from("Game patched successfully."),
            Line::from(""),
            Line::from(format!("Modified files       {}", report.modified_files)),
            Line::from(format!("Applied entries      {}", report.applied_entries)),
            Line::from(format!("Unused entries       {}", report.unused_entries)),
            Line::from(""),
            Line::from("Backup saved at:"),
            Line::from(format!("  {}", report.backup_path.display())),
            Line::from(""),
            Line::from("Report saved at:"),
            Line::from(format!("  {}", report.report_path.display())),
        ];
        frame.render_widget(panel(lines, "Patch complete"), area);
    }

    fn render_error(&self, frame: &mut Frame, area: Rect) {
        let error = self.error.as_deref().unwrap_or("unknown error");
        frame.render_widget(
            panel(
                vec![
                    Line::from("Patch failed."),
                    Line::from(""),
                    Line::from(error.to_string()),
                    Line::from(""),
                    Line::from("Press Enter or Esc to go back."),
                ],
                "Error",
            ),
            area,
        );
    }
}

#[derive(Debug)]
struct FilePicker {
    root: PathBuf,
    current: PathBuf,
    mode: PickMode,
    entries: Vec<FileEntry>,
    selected: usize,
}

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

impl FilePicker {
    fn new(root: PathBuf, current: PathBuf, mode: PickMode) -> Self {
        let current = if current.starts_with(&root) {
            current
        } else {
            root.clone()
        };
        let mut picker = Self {
            root,
            current,
            mode,
            entries: Vec::new(),
            selected: 0,
        };
        picker.refresh();
        picker
    }

    fn refresh(&mut self) {
        let mut entries = Vec::new();
        if self.current != self.root {
            entries.push(FileEntry {
                name: "..".to_string(),
                path: self.current.parent().unwrap_or(&self.root).to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(read_dir) = fs::read_dir(&self.current) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let is_dir = file_type.is_dir();
                let is_match =
                    path.extension().and_then(|ext| ext.to_str()) == Some(self.mode.extension());
                if is_dir || is_match {
                    entries.push(FileEntry {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path,
                        is_dir,
                    });
                }
            }
        }

        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.entries.len().saturating_sub(1));
    }

    fn go_up(&mut self) {
        if self.current != self.root {
            self.current = self.current.parent().unwrap_or(&self.root).to_path_buf();
            self.selected = 0;
            self.refresh();
        }
    }

    fn enter_dir(&mut self, path: PathBuf) {
        if path.starts_with(&self.root) {
            self.current = path;
            self.selected = 0;
            self.refresh();
        }
    }

    fn selected_path(&self) -> Option<PathBuf> {
        self.entries
            .get(self.selected)
            .map(|entry| entry.path.clone())
    }
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(APP_NAME, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::raw(APP_DESCRIPTION),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn render_footer(frame: &mut Frame, area: Rect, screen: Screen) {
    let text = match screen {
        Screen::Select => "Left/Right select    Enter choose    q quit",
        Screen::Confirm => "Enter start patch    Esc back    q quit",
        Screen::Picker => "Up/Down move    Enter open/select    Backspace up    Esc cancel",
        Screen::Done => "Enter quit",
        Screen::Error => "Enter back    q quit",
    };
    let footer = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, area);
}

fn panel<'a>(lines: Vec<Line<'a>>, title: &'a str) -> Paragraph<'a> {
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .alignment(Alignment::Left)
}

fn action_line<'a>(actions: &[&'a str], selected: usize) -> Line<'a> {
    let mut spans = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("     "));
        }
        let style = if index == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(format!(" {action} "), style));
    }
    Line::from(spans)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
