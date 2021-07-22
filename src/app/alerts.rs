use graphics::Transformed;
use graphics::context::Context;
use graphics::types::Color;
use opengl_graphics::GlGraphics;

use super::FontGlyphCache;
use super::colors;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

use std::collections::VecDeque;

pub const FONT_SIZE: u32 = 10;

const TOOLBAR_PADDING: f64 = 0.0; // 19.0;
const MARGIN_SIZE: u32 = 8;
const LINE_SPACING: f64 = 1.10;

#[derive(Debug)]
pub struct Alerts {
  now: f32,
  messages: VecDeque<AlertMessage>,
  max_lifetime: f32,
  active: bool
}

impl Alerts {
  pub fn new(max_lifetime: f32) -> Self {
    Alerts {
      now: 0.0,
      messages: VecDeque::new(),
      max_lifetime,
      active: false
    }
  }

  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn set_state(&mut self, active: bool) {
    self.active = active;
  }

  fn len(&self) -> usize {
    self.messages.iter()
      .filter(|m| !m.is_dead(self.now))
      .count()
  }

  pub fn tick(&mut self, dt: f32) {
    self.now += dt;
    while self.messages.len() >= 48 {
      self.messages.pop_front();
    };
  }

  pub fn push<S: Into<String>>(&mut self, text: S) {
    self.push_message(text.into(), MessageKind::User);
  }

  pub fn push_system<S: Into<String>>(&mut self, text: Result<S, S>) {
    match text {
      Ok(t) => self.push_message(t.into(), MessageKind::System),
      Err(t) => self.push_message(t.into(), MessageKind::SystemError)
    };
  }

  pub fn draw(&self, ctx: Context, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    const PADDING: f64 = MARGIN_SIZE as f64;
    const WINDOW_HEIGHT_F: f64 = WINDOW_HEIGHT as f64;
    const WINDOW_WIDTH_F: f64 = WINDOW_WIDTH as f64;
    const FONT_HEIGHT: f64 = (FONT_SIZE as f64 * LINE_SPACING * 1.5) as u32 as f64;

    if self.active {
      let height = WINDOW_HEIGHT_F - PADDING;

      let pos = [WINDOW_WIDTH_F, WINDOW_HEIGHT_F];
      graphics::rectangle_from_to(colors::OVERLAY_T, [0.0, 0.0], pos, ctx.transform, gl);

      for (i, (text, color)) in self.iter_all().enumerate() {
        let y = height - i as f64 * FONT_HEIGHT;
        let t = ctx.transform.trans(8.0, y);
        graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
          .expect("unable to draw text");
      };
    } else {
      let height = self.len() as f64 * FONT_HEIGHT + 8.0 + TOOLBAR_PADDING;
      let height = height.min(WINDOW_HEIGHT_F - 8.0);

      for (i, (text, color)) in self.iter().enumerate() {
        let y = height - i as f64 * FONT_HEIGHT;
        let t = ctx.transform.trans(8.0, y);
        graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
          .expect("unable to draw text");
      };
    };
  }

  fn push_message(&mut self, text: String, color: MessageKind) {
    self.messages.push_back(AlertMessage::new(text, color, self.now + self.max_lifetime));
  }

  fn iter_all(&self) -> impl Iterator<Item = (&str, Color)> {
    self.messages.iter()
      .rev()
      .map(|m| (m.text.as_str(), m.get_color()))
  }

  fn iter(&self) -> impl Iterator<Item = (&str, Color)> {
    self.messages.iter()
      .rev()
      .filter(move |m| !m.is_dead(self.now))
      .map(move |m| (m.text.as_str(), m.get_color_alpha(self.now)))
  }
}

#[derive(Debug)]
struct AlertMessage {
  text: String,
  kind: MessageKind,
  expiry: f32
}

impl AlertMessage {
  fn new(text: String, kind: MessageKind, expiry: f32) -> Self {
    AlertMessage { text, kind, expiry }
  }

  fn is_dead(&self, now: f32) -> bool {
    now >= self.expiry
  }

  fn get_color(&self) -> Color {
    self.kind.get_color()
  }

  fn get_color_alpha(&self, now: f32) -> Color {
    let mut color = self.kind.get_color();
    let alpha = (self.expiry - now).min(1.0).max(0.0);
    color[3] = alpha as f32;
    color
  }
}

const TEXT_USER: Color = [0.8, 0.8, 0.8, 1.0];
const TEXT_SYSTEM: Color = [1.0, 1.0, 1.0, 1.0];
const TEXT_SYSTEM_ERROR: Color = [1.0, 0.2, 0.2, 1.0];

#[derive(Debug)]
enum MessageKind {
  User, System, SystemError
}

impl MessageKind {
  fn get_color(&self) -> Color {
    match self {
      MessageKind::User => TEXT_USER,
      MessageKind::System => TEXT_SYSTEM,
      MessageKind::SystemError => TEXT_SYSTEM_ERROR
    }
  }
}
