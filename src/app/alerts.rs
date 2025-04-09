use graphics::Transformed;
use graphics::context::Context;
use graphics::types::Color as DrawColor;
use opengl_graphics::GlGraphics;

use super::{colors, FontGlyphCache};
use super::interface::Interface;
use crate::font::{self, FONT_SIZE};

use std::collections::VecDeque;

pub const PADDING: [f64; 2] = [
  super::interface::PADDING[0] * 2.0,
  super::interface::PADDING[1] * 2.0
];

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

  pub fn push<S: Into<String>>(&mut self, text: Result<S, S>) {
    match text {
      Ok(t) => self.push_message(t.into(), TEXT_SYSTEM),
      Err(t) => self.push_message(t.into(), TEXT_SYSTEM_ERROR)
    };
  }

  pub fn draw(&self, ctx: Context, interface: &Interface, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    const LINE_SPACING: f64 = 1.10;

    let [window_width, window_height] = interface.get_window_size();

    let v_metrics = font::get_v_metrics();
    let font_height = ((v_metrics.ascent - v_metrics.descent) * LINE_SPACING).round();

    if self.active {
      let height = window_height - font_height - PADDING[1] * 1.25;

      let pos = [window_width, window_height];
      graphics::rectangle_from_to(colors::OVERLAY_T, [0.0, 0.0], pos, ctx.transform, gl);

      for (i, (text, color)) in self.iter_all().enumerate() {
        let x = PADDING[0] + interface.get_sidebar_width() as f64;
        let y = height - i as f64 * font_height;
        let t = ctx.transform.trans(x, y);
        graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
          .expect("unable to draw text");
      };
    } else {
      let height = self.len() as f64 * font_height + PADDING[1] + interface.get_toolbar_height() as f64;
      let height = height.min(window_height - PADDING[1] * 1.25);

      for (i, (text, color)) in self.iter().enumerate() {
        let x = PADDING[0] + interface.get_sidebar_width() as f64;
        let y = height - i as f64 * font_height;
        let t = ctx.transform.trans(x, y);
        graphics::text(color, FONT_SIZE, text, glyph_cache, t, gl)
          .expect("unable to draw text");
      };
    };
  }

  fn push_message(&mut self, text: String, color: DrawColor) {
    self.messages.push_back(AlertMessage::new(text, color, self.now + self.max_lifetime));
  }

  fn iter_all(&self) -> impl Iterator<Item = (&str, DrawColor)> {
    self.messages.iter()
      .rev()
      .map(|m| (m.text.as_str(), m.get_color()))
  }

  fn iter(&self) -> impl Iterator<Item = (&str, DrawColor)> {
    self.messages.iter()
      .rev()
      .filter(move |m| !m.is_dead(self.now))
      .map(move |m| (m.text.as_str(), m.get_color_alpha(self.now)))
  }
}

#[derive(Debug)]
struct AlertMessage {
  text: String,
  color: DrawColor,
  expiry: f32
}

impl AlertMessage {
  fn new(text: String, color: DrawColor, expiry: f32) -> Self {
    AlertMessage { text, color, expiry }
  }

  fn is_dead(&self, now: f32) -> bool {
    now >= self.expiry
  }

  fn get_color(&self) -> DrawColor {
    self.color
  }

  fn get_color_alpha(&self, now: f32) -> DrawColor {
    let mut color = self.color;
    color[3] = (self.expiry - now).min(1.0).max(0.0);
    color
  }
}

const TEXT_SYSTEM: DrawColor = [1.0, 1.0, 1.0, 1.0];
const TEXT_SYSTEM_ERROR: DrawColor = [1.0, 0.2, 0.2, 1.0];
