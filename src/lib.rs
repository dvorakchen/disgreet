//! disgreet lib

use std::{
    fs::{self, OpenOptions},
    io,
    path::Path,
};

use crate::ui::UI;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ini::Ini;
use log::{debug, warn};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use simplelog::{CombinedLogger, WriteLogger};
use thiserror::Error;
mod core;
mod ui;

// i18n

pub(crate) const CACHE_BG_DIR: &str = "/var/cache/disgreet/";
pub(crate) const DEFAULT_LOG_PATH: &str = "/var/log/disgreet/disgreet.log";
const DESKTOP_FILE_PATH: &str = "/usr/share/wayland-sessions/";
pub(crate) const PREVIOUS_USERNAME_FILE: &str = "/var/cache/disgreet/previous_username";

pub struct Disgreet {
    background: String,
    sessions: Vec<Session>,
    prev_username: String,
}
impl Disgreet {
    pub fn new(background: &str) -> Self {
        Self {
            background: background.into(),
            sessions: vec![],
            prev_username: String::new(),
        }
    }

    fn setup(&mut self) {
        if let Ok(log_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(DEFAULT_LOG_PATH)
        {
            let level = if cfg!(debug_assertions) {
                simplelog::LevelFilter::Debug
            } else {
                simplelog::LevelFilter::Info
            };

            _ = CombinedLogger::init(vec![WriteLogger::new(
                level,
                simplelog::Config::default(),
                log_file,
            )]);
        }

        debug!("new disgreet: \n{{ background: {} }}", self.background,);
        debug!("setup disgreet");

        if !Path::new(DESKTOP_FILE_PATH).exists() {
            warn!("path {} not exists", DESKTOP_FILE_PATH);
        }
        debug!("path {} exists", DESKTOP_FILE_PATH);

        self.prev_username = fs::read_to_string(PREVIOUS_USERNAME_FILE).unwrap_or_default();

        self.sessions = find_desktop_files(Path::new(DESKTOP_FILE_PATH));
        append_bash_session(&mut self.sessions);
    }
    pub fn run(mut self) -> anyhow::Result<()> {
        self.setup();

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = run_app(
            &mut terminal,
            &self.background,
            &self.sessions,
            &self.prev_username,
        );

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{:?}", err);
        }

        Ok(())
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    background: &str,
    sessions: &[Session],
    username: &str,
) -> anyhow::Result<()> {
    let size = terminal.size().unwrap_or_default();
    let mut ui = UI::new(background, sessions, username, size);
    loop {
        terminal.draw(|f| ui.draw(f)).expect("UI draw failed");

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    if cfg!(debug_assertions) {
                        return Ok(());
                    }
                }
                KeyCode::Tab => ui.next_focus(),
                KeyCode::Enter => {
                    if ui.handle_enter() {
                        return Ok(());
                    }
                }
                _ => ui.handle_input(key.code),
            }
        }
    }
}

fn append_bash_session(sessions: &mut Vec<Session>) {
    if !sessions.iter().any(|s| s.name.to_lowercase() == "bash") {
        sessions.push(Session {
            name: "bash".to_string(),
            exec: "/usr/bin/bash".to_string(),
        });
    }
}

fn find_desktop_files(path: &Path) -> Vec<Session> {
    let mut sessions = vec![];
    if let (true, Ok(entries)) = (path.is_dir(), fs::read_dir(path)) {
        for entry in entries.flatten() {
            let file_path = entry.path();

            if file_path.is_file() {
                if let Some(ext) = file_path.extension() {
                    if ext == "desktop" {
                        if let Ok(session) = parser_desktop_file(&file_path) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
    }

    sessions
}

fn parser_desktop_file(path: &Path) -> anyhow::Result<Session> {
    let conf = Ini::load_from_file(path)?;
    let section = conf
        .section(Some("Desktop Entry".to_string()))
        .ok_or(DesktopFile::NotExist)?;

    Ok(Session {
        name: section.get("Name").unwrap_or_default().to_string(),
        // comment: section.get("Comment").unwrap_or_default().to_string(),
        exec: section.get("Exec").unwrap_or_default().to_string(),
        // session_type: section.get("Type").unwrap_or_default().to_string(),
        // desktop_names: section.get("DesktopNames").unwrap_or_default().to_string(),
    })
}

#[derive(Debug, Clone)]
struct Session {
    name: String,
    // comment: String,
    exec: String,
    // session_type: String,
    // desktop_names: String,
}

#[derive(Debug, Error)]
enum DesktopFile {
    #[error("Desktop Entry section not exists")]
    NotExist,
}
