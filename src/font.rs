use chrono::Local;
use defy::Contextualize;
use fs_err as fs;
use once_cell::sync::Lazy;
use rusttype::{Font, Scale};

use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::Error;
use crate::i18n;

pub const FONT_SIZE: u32 = 11;
const POINTS_TO_PIXELS: f32 = 4.0 / 3.0;
const FONT_SCALE: Scale = Scale {
    x: FONT_SIZE as f32 * POINTS_TO_PIXELS,
    y: FONT_SIZE as f32 * POINTS_TO_PIXELS,
};
static DPI_SCALE_BITS: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

#[inline]
pub fn dpi_scale() -> f32 {
    f32::from_bits(DPI_SCALE_BITS.load(Ordering::Relaxed))
}

pub fn set_dpi_scale(scale: f64) {
    DPI_SCALE_BITS.store((scale as f32).to_bits(), Ordering::Relaxed);
}

pub fn render_font_size() -> u32 {
    ((FONT_SIZE as f32) * dpi_scale()).round().max(1.0) as u32
}

pub fn get_font() -> Font<'static> {
    get_font_ref().clone()
}

fn get_font_ref() -> &'static Font<'static> {
    static FONT: Lazy<Font<'static>> = Lazy::new(load_font);

    &*FONT
}

fn load_font() -> Font<'static> {
    try_load_system_font().unwrap_or_else(|| {
        const FONT_DATA: &[u8] = include_bytes!("../assets/Inconsolata-Regular.ttf");
        Font::try_from_bytes(FONT_DATA).expect("unable to load font")
    })
}

fn try_load_system_font() -> Option<Font<'static>> {
    #[cfg(target_os = "windows")]
    {
        if i18n::lang() != i18n::Lang::ZhCn {
            return None;
        }

        for path in [
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\msyh.ttf",
            r"C:\Windows\Fonts\simkai.ttf",
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\simsun.ttc",
        ] {
            if let Ok(data) = fs::read(path) {
                if let Some(font) = Font::try_from_vec(data) {
                    return Some(font);
                }
            }
        }
    }

    None
}

pub fn get_width_metric(ch: char) -> f64 {
    get_font_ref()
        .glyph(ch)
        .scaled(FONT_SCALE)
        .h_metrics()
        .advance_width as f64
}

pub fn get_width_metric_str(s: &str) -> f64 {
    get_font_ref()
        .glyphs_for(s.chars())
        .map(|glyph| glyph.scaled(FONT_SCALE).h_metrics().advance_width)
        .sum::<f32>() as f64
}

pub fn get_height_metric() -> f64 {
    let v_metrics = get_font_ref().v_metrics(FONT_SCALE);
    (v_metrics.ascent - v_metrics.descent) as f64
}

pub fn get_v_metrics() -> VMetrics {
    let v_metrics = get_font_ref().v_metrics(FONT_SCALE);
    VMetrics {
        ascent: v_metrics.ascent as f64,
        descent: v_metrics.descent as f64,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VMetrics {
    pub ascent: f64,
    pub descent: f64,
}

pub fn view_font_license() -> Result<(), Error> {
    const LICENSE_CONTENTS: &[u8] = include_bytes!("../assets/Inconsolata-OFL.txt");

    let now = Local::now().format("%Y%m%d-%H%M%S");
    let path = env::temp_dir().join(format!("Inconsolata-OFL-{}.txt", now));

    fs::write(&path, LICENSE_CONTENTS).context("Failed to write font license to disk")?;

    if cfg!(target_os = "windows") {
        Command::new("notepad")
            .arg(path)
            .spawn()
            .context("Failed to open license")?;
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(path)
            .spawn()
            .context("Failed to open license")?;
    } else {
        unimplemented!()
    };

    Ok(())
}
