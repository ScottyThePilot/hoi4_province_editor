use once_cell::sync::Lazy;
use rusttype::{Font, Scale};

pub const FONT_SIZE: u32 = 11;
const FONT_SCALE: Scale = Scale { x: 15.0, y: 15.0 };

pub fn get_font() -> Font<'static> {
  get_font_ref().clone()
}

fn get_font_ref() -> &'static Font<'static> {
  const FONT_DATA: &[u8] = include_bytes!("../assets/Inconsolata-Regular.ttf");
  static FONT: Lazy<Font<'static>> = Lazy::new(|| {
    Font::try_from_bytes(FONT_DATA)
      .expect("unable to load font")
  });

  &*FONT
}

pub fn get_width_metric(ch: char) -> f64 {
  get_font_ref()
    .glyph(ch)
    .scaled(FONT_SCALE)
    .h_metrics()
    .advance_width
    as f64
}

pub fn get_width_metric_str(s: &str) -> f64 {
  get_font_ref()
    .glyphs_for(s.chars())
    .map(|glyph| {
      glyph
        .scaled(FONT_SCALE)
        .h_metrics()
        .advance_width
    })
    .sum::<f32>()
    as f64
}

pub fn get_height_metric() -> f64 {
  let v_metrics = get_font_ref().v_metrics(FONT_SCALE);
  (v_metrics.ascent - v_metrics.descent) as f64
}

pub fn get_v_metrics() -> VMetrics {
  let v_metrics = get_font_ref().v_metrics(FONT_SCALE);
  VMetrics {
    ascent: v_metrics.ascent as f64,
    descent: v_metrics.descent as f64
  }
}

#[derive(Debug, Clone, Copy)]
pub struct VMetrics {
  pub ascent: f64,
  pub descent: f64
}
