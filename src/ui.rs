//! UI

use std::{fs, path::PathBuf, str::FromStr};

use crossterm::event::KeyCode;
use image::{DynamicImage, GenericImageView, RgbImage};
use log::{debug, error};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect, Size},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::{CACHE_BG_DIR, PREVIOUS_USERNAME_FILE, Session, core::authenticate};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Session,
    Username,
    Password,
}

pub(crate) struct UI<'a> {
    render_img: Option<image::DynamicImage>,

    focus: Focus,
    username: String,
    password: String,
    cursor_pos: usize,
    message: String,

    sessions: &'a [Session],
    session_idx: usize,
    handling: bool,
}

impl<'a> UI<'a> {
    pub(crate) fn new(
        background: &str,
        sessions: &'a [Session],
        prev_username: &str,
        size: Size,
    ) -> Self {
        let focus = if prev_username.is_empty() {
            Focus::Username
        } else {
            Focus::Password
        };
        let cursor_pos = if prev_username.is_empty() {
            0
        } else {
            prev_username.chars().count()
        };

        let mut render_img: Option<image::DynamicImage> = None;

        match PathBuf::from_str(background) {
            Ok(path) => {
                if path.exists() && path.is_file() {
                    let file_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    if render_img.is_none() {
                        let cache_file = format!(
                            "{}/{}_{}x{}.bin",
                            CACHE_BG_DIR, file_name, size.width, size.height
                        );
                        debug!("check cache file: {cache_file}");

                        let cache_path = PathBuf::from_str(&cache_file).unwrap();
                        if cache_path.exists() {
                            if let Ok(raw_bytes) = fs::read(&cache_path) {
                                if let Some(rgb_buf) = RgbImage::from_raw(
                                    size.width as u32,
                                    size.height as u32,
                                    raw_bytes,
                                ) {
                                    debug!("cache file exists");
                                    render_img = Some(DynamicImage::ImageRgb8(rgb_buf));
                                }
                            }
                        }

                        if render_img.is_none() {
                            debug!("has no cache");

                            let base_img = image::ImageReader::open(path)
                                .unwrap()
                                .with_guessed_format()
                                .unwrap()
                                .decode()
                                .unwrap();
                            let resized_img = base_img.resize_exact(
                                size.width as u32,
                                size.height as u32,
                                image::imageops::FilterType::Nearest,
                            );
                            let rgb8 = resized_img.to_rgb8();
                            _ = fs::write(&cache_file, rgb8.as_raw());
                            debug!("cache new file");
                            render_img = Some(resized_img);
                        }
                    }
                }
            }
            Err(_) => {
                debug!("base_img is none, not draw background");
            }
        }

        Self {
            render_img,

            focus,
            username: prev_username.to_string(),
            password: String::new(),
            cursor_pos,
            message: String::new(),

            sessions,
            session_idx: 0,
            handling: false,
        }
    }

    fn current_session(&mut self) -> Option<&Session> {
        let mut sessions = self.sessions.iter();
        if let Some(session) = sessions.nth(self.session_idx) {
            Some(session)
        } else {
            self.session_idx = 0;
            sessions.nth(0)
        }
    }

    fn go_next_session(&mut self) -> Option<&Session> {
        let sessions = self.sessions.iter();
        let count = sessions.count();
        self.session_idx = (self.session_idx + 1) % count;

        self.current_session()
    }
    fn go_prev_session(&mut self) -> Option<&Session> {
        if self.session_idx > 0 {
            self.session_idx -= 1;
        } else {
            self.session_idx = self.sessions.len() - 1;
        }

        self.current_session()
    }

    pub(crate) fn draw(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();

        self.draw_background(area, f.buffer_mut());

        let area = centered_rect(40, 10, area);
        self.draw_login_box(area, f);
    }

    fn draw_login_box(&mut self, area: Rect, f: &mut ratatui::Frame) {
        let message = if self.handling {
            "auth..."
        } else {
            &self.message.clone()
        };

        let block = Block::default()
            .title(message)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .style(Style::default().bg(Color::Black))
            .padding(Padding::new(2, 2, 1, 0));

        // 用布局切出内部区域
        let inner = block.inner(area);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // 标题
                Constraint::Length(1), // 空行
                Constraint::Length(1), // username
                Constraint::Length(1),
                Constraint::Length(1), // password
            ])
            .split(inner);

        let session_name = self
            .current_session()
            .map_or("".to_string(), |s| s.name.clone());

        // 标题
        let title_span = Span::styled(
            format!("< {session_name} >"),
            Style::default()
                .fg(Color::White)
                .add_modifier(if self.focus == Focus::Session {
                    Modifier::UNDERLINED
                } else {
                    Modifier::empty()
                }),
        );
        let title = Paragraph::new(title_span).alignment(Alignment::Center);
        f.render_widget(title, rows[0]);

        // Username 行
        let user_style = if self.focus == Focus::Username {
            Style::default().fg(Color::Cyan) // 获焦时高亮
        } else {
            Style::default().fg(Color::Gray)
        };
        let user_text = format!("{}: {}", "Username", self.username);
        f.render_widget(Paragraph::new(user_text.clone()).style(user_style), rows[2]);
        // cursor posinion
        if self.focus == Focus::Username {
            let x = rows[2].x + user_text.len() as u16;
            let y = rows[2].y;
            f.set_cursor_position(Position::new(x, y));
        }

        // Password 行（类似，密码用 * 显示）
        let pass_style = if self.focus == Focus::Password {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };
        let pass_text = format!("{}: ", "Password",);
        if self.focus == Focus::Password {
            let x = rows[4].x + pass_text.len() as u16;
            let y = rows[4].y;
            f.set_cursor_position(Position::new(x, y));
        }
        f.render_widget(Paragraph::new(pass_text).style(pass_style), rows[4]);

        // 最后画边框
        f.render_widget(block, area);
    }

    fn draw_background(&mut self, area: Rect, buf: &mut Buffer) {
        if self.render_img.is_none() {
            return;
        }

        let render_img = self.render_img.as_ref().unwrap();

        debug!("draw background start");
        debug!("area height: {}", area.height);
        debug!("area width: {}", area.width);
        for y in 0..area.height {
            for x in 0..area.width {
                let pixel = render_img.get_pixel(x as u32, y as u32);

                let mut r = pixel[0];
                let mut g = pixel[1];
                let mut b = pixel[2];

                if r > 230 && g > 230 && b > 230 {
                    r = 255;
                    g = 255;
                    b = 255;
                }
                let bg_color = if r == 255 && g == 255 && b == 255 {
                    Color::Indexed(15)
                } else {
                    Color::Rgb(r, g, b)
                };

                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char(' ').set_bg(bg_color);
                }
            }
        }
        debug!("draw background end");
    }
}

impl UI<'_> {
    pub(crate) fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Session => Focus::Username,
            Focus::Username => Focus::Password,
            Focus::Password => Focus::Session,
        };

        self.update_cursor_pos();
    }

    pub(crate) fn update_cursor_pos(&mut self) {
        self.cursor_pos = match self.focus {
            Focus::Session => 0,
            Focus::Username => self.username.chars().count(),
            Focus::Password => self.password.len(),
        };
    }

    pub(crate) fn handle_input(&mut self, key: KeyCode) {
        if self.handling {
            return;
        }
        match self.focus {
            Focus::Session => match key {
                KeyCode::Left => {
                    self.go_prev_session();
                }
                KeyCode::Right => {
                    self.go_next_session();
                }
                _ => {}
            },
            Focus::Username => match key {
                KeyCode::Backspace => _ = self.username.pop(),
                KeyCode::Char(c) => self.username.push(c),
                _ => {}
            },
            Focus::Password => match key {
                KeyCode::Backspace => _ = self.password.pop(),
                KeyCode::Char(c) => self.password.push(c),
                _ => {}
            },
        }
    }

    /// Returns `true` if authentication succeeded and the session should be launched.
    pub(crate) fn handle_enter(&mut self) -> bool {
        if self.handling {
            return false;
        }

        self.handling = true;
        let username: &str = &(self.username.clone());
        let password: &str = &(self.password.clone());
        let session = self.current_session().unwrap();
        let res = match authenticate(username, password, session) {
            Ok(()) => {
                debug!("auth success, exiting event loop to launch session");
                _ = fs::write(PREVIOUS_USERNAME_FILE, username);
                true
            }
            Err(e) => {
                self.message = e.to_string();
                error!("authenticate failed: {:?}", e);
                false
            }
        };

        self.handling = false;
        res
    }
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    // 处理屏幕比组件还小的情况
    if r.width < width || r.height < height {
        return r;
    }

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height - height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width - width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}
