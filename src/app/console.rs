use graphics::Transformed;
use graphics::types::Color;
use opengl_graphics::GlGraphics;
use vecmath::Matrix2x3;

use super::FontGlyphCache;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

use std::collections::VecDeque;
use std::time::{Instant, Duration};

const CURSOR: char = '\u{2588}';
const MARGIN_SIZE: u32 = 8;
const LINE_SPACING: f64 = 1.10;
pub const FONT_SIZE: u32 = 10;
pub const TEXT_USER: Color = [0.8, 0.8, 0.8, 1.0];
pub const TEXT_SYSTEM: Color = [1.0, 1.0, 1.0, 1.0];
pub const TEXT_SYSTEM_ERROR: Color = [1.0, 0.2, 0.2, 1.0];
pub const NEUTRAL: Color = [0.0, 0.0, 0.0, 0.5];

#[derive(Debug)]
pub struct Console {
  epoch: Instant,
  max_lifetime: Duration,
  mode: Option<ActiveConsole>,
  messages: VecDeque<ConsoleMessage>
}

impl Console {
  pub fn new(max_lifetime: Duration) -> Self {
    Console {
      epoch: Instant::now(),
      max_lifetime,
      mode: None,
      messages: VecDeque::new()
    }
  }

  pub(super) fn enter_command(&mut self) -> Option<String> {
    if let Some(active_console) = &mut self.mode {
      let command = active_console.take();
      let entry = format!("> {}", command);
      self.push(entry);
      Some(command)
    } else {
      None
    }
  }

  pub(super) fn activate(&mut self) {
    self.mode = Some(ActiveConsole::default());
  }

  pub(super) fn deactivate(&mut self) {
    self.mode = None;
  }

  pub fn is_active(&self) -> bool {
    self.mode.is_some()
  }

  pub(super) fn handle(&mut self) -> ConsoleHandle<'_> {
    ConsoleHandle { inner: self }
  }

  pub fn action(&mut self, action: ConsoleAction) {
    if let Some(active_console) = &mut self.mode {
      match action {
        ConsoleAction::Insert(data) => active_console.insert(data),
        ConsoleAction::Left => active_console.left(),
        ConsoleAction::Right => active_console.right(),
        ConsoleAction::Backspace => active_console.backspace(),
        ConsoleAction::Delete => active_console.delete()
      };
    };
  }

  fn iter_all(&self) -> impl Iterator<Item = (&str, Color)> {
    self.messages.iter()
      .rev()
      .map(|m| (m.text.as_str(), m.color.get()))
  }

  fn iter(&self, now: Instant) -> impl Iterator<Item = (&str, Color)> {
    self.messages.iter()
      .rev()
      .filter(move |m| !m.dead(now))
      .map(move |m| (m.text.as_str(), m.color(now)))
  }

  fn len(&self, now: Instant) -> usize {
    self.messages.iter()
      .filter(|m| !m.dead(now))
      .count()
  }

  pub fn tick(&mut self) {
    while self.messages.len() >= 48 {
      self.messages.pop_front();
    };
  }

  pub fn push<S: Into<String>>(&mut self, text: S) {
    self.messages.push_back(ConsoleMessage::new(text.into(), ConsoleColor::User, self.max_lifetime));
  }

  pub fn push_system<S: Into<String>>(&mut self, text: Result<S, S>) {
    let (text, color) = match text {
      Ok(t) => (t.into(), ConsoleColor::System),
      Err(t) => (t.into(), ConsoleColor::SystemError)
    };
    
    self.messages.push_back(ConsoleMessage::new(text, color, self.max_lifetime));
  }

  pub fn draw(&self, transform: Matrix2x3<f64>, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    draw_console(self, Instant::now(), transform, glyph_cache, gl)
  }
}

#[repr(transparent)]
pub struct ConsoleHandle<'c> {
  inner: &'c mut Console
}

impl<'c> ConsoleHandle<'c> {
  pub fn push<S: Into<String>>(&mut self, text: S) {
    self.inner.push(text)
  }

  pub fn push_system<S: Into<String>>(&mut self, text: Result<S, S>) {
    self.inner.push_system(text)
  }
}

#[derive(Debug)]
struct ConsoleMessage {
  text: String,
  color: ConsoleColor,
  epoch: Instant,
  max_lifetime: Duration
}

impl ConsoleMessage {
  fn new(text: String, color: ConsoleColor, max_lifetime: Duration) -> Self {
    ConsoleMessage { text, color, epoch: Instant::now(), max_lifetime }
  }

  fn dead(&self, now: Instant) -> bool {
    now.duration_since(self.epoch) > self.max_lifetime
  }
  
  fn color(&self, now: Instant) -> Color {
    let mut color = self.color.get();
    color[3] = self.alpha_lifetime(now);
    color
  }

  fn alpha_lifetime(&self, now: Instant) -> f32 {
    let dt = now.duration_since(self.epoch).as_secs_f32();
    let dt = self.max_lifetime.as_secs_f32() - dt;
    dt.min(1.0).max(0.0)
  }
}

#[derive(Debug)]
enum ConsoleColor {
  User, System, SystemError
}

impl ConsoleColor {
  fn get(&self) -> Color {
    match self {
      ConsoleColor::User => TEXT_USER,
      ConsoleColor::System => TEXT_SYSTEM,
      ConsoleColor::SystemError => TEXT_SYSTEM_ERROR
    }
  }
}

#[derive(Debug)]
pub enum ConsoleAction {
  Insert(String),
  Left,
  Right,
  Backspace,
  Delete
}

#[derive(Debug)]
struct ActiveConsole {
  input: Vec<char>,
  cursor: usize
}

impl ActiveConsole {
  fn text(&self, blink: bool) -> String {
    if blink {
      self.input.iter().collect()
    } else if self.cursor >= self.input.len() {
      let mut input: String = self.input.iter().collect();
      input.push(CURSOR);
      input
    } else {
      self.input.iter()
        .enumerate()
        .map(|(i, &ch)| match i == self.cursor {
          true => CURSOR,
          false => ch
        })
        .collect()
    }
  }

  fn take(&mut self) -> String {
    self.cursor = 0;
    self.input.drain(..).collect()
  }

  fn left(&mut self) {
    self.cursor = self.cursor.saturating_sub(1);
  }

  fn right(&mut self) {
    self.cursor = (self.cursor + 1)
      .min(self.input.len());
  }

  fn backspace(&mut self) {
    if self.cursor > 0 {
      self.cursor -= 1;
      self.input.remove(self.cursor);
    };
  }

  fn delete(&mut self) {
    if self.input.len() > self.cursor {
      self.input.remove(self.cursor);
    };
  }

  fn insert(&mut self, data: String) {
    for ch in data.chars() {
      if ch != '\n' {
        self.input.insert(self.cursor, ch);
        self.cursor += 1;
      };
    };
  }
}

impl Default for ActiveConsole {
  fn default() -> Self {
    ActiveConsole {
      input: Vec::new(),
      cursor: 0
    }
  }
}

fn draw_console(
  console: &Console,
  now: Instant,
  transform: Matrix2x3<f64>,
  glyph_cache: &mut FontGlyphCache,
  gl: &mut GlGraphics
) {
  const PADDING: f64 = MARGIN_SIZE as f64;
  const WINDOW_HEIGHT_F: f64 = WINDOW_HEIGHT as f64;
  const WINDOW_WIDTH_F: f64 = WINDOW_WIDTH as f64;
  const FONT_HEIGHT: f64 = (FONT_SIZE as f64 * LINE_SPACING * 1.5) as u32 as f64;

  let blink = now.duration_since(console.epoch).as_secs_f32();
  let blink = (blink * 2.0) as u32 % 2 == 0;

  if let Some(active_console) = &console.mode {
    let height = WINDOW_HEIGHT_F - PADDING - FONT_HEIGHT;

    let pos = [WINDOW_WIDTH_F, WINDOW_HEIGHT_F];
    graphics::rectangle_from_to(NEUTRAL, [0.0, 0.0], pos, transform, gl);

    let t = transform.trans(PADDING, WINDOW_HEIGHT_F - PADDING);
    let text = active_console.text(blink);
    graphics::text(TEXT_USER, FONT_SIZE, &text, glyph_cache, t, gl)
      .expect("unable to draw text");

    for (i, (text, color)) in console.iter_all().enumerate() {
      let y = height - i as f64 * FONT_HEIGHT;
      let t = transform.trans(PADDING, y);
      graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
        .expect("unable to draw text");
    };
  } else {
    let height = console.len(now) as f64 * FONT_HEIGHT + PADDING;
    let height = height.min(WINDOW_HEIGHT_F - PADDING);

    for (i, (text, color)) in console.iter(now).enumerate() {
      let y = height - i as f64 * FONT_HEIGHT;
      let t = transform.trans(PADDING, y);
      graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
        .expect("unable to draw text");
    };
  };
}
