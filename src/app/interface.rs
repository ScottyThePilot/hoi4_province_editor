use graphics::Transformed;
use graphics::context::Context;
use graphics::types::Color;
use opengl_graphics::GlGraphics;
use rusttype::{Font, Scale};
use vecmath::Vector2;

use super::{colors, FontGlyphCache, FONT_SIZE};

const PADDING: Vector2<f64> = [6.0, 4.0];

const PALETTE_BUTTON: Palette = Palette {
  foreground: colors::WHITE,
  background: colors::BUTTON,
  background_hover: colors::BUTTON_HOVER
};

const PALETTE_BUTTON_TOOLBAR: Palette = Palette {
  foreground: colors::WHITE,
  background: colors::BUTTON_TOOLBAR,
  background_hover: colors::BUTTON_TOOLBAR_HOVER
};

#[derive(Debug, Clone)]
pub struct Interface {
  buttons: Vec<ButtonElement>,
  toolbar_buttons: Vec<ToolbarButtonElement>,
  toolbar_plate: PlateComponent
}

impl Interface {
  pub fn on_mouse_click(&mut self, pos: Vector2<f64>) -> Result<ButtonId, bool> {
    for button in &self.buttons {
      if button.text_box.test(pos) {
        return Ok(button.id);
      };
    };

    for toolbar_button in &mut self.toolbar_buttons {
      if toolbar_button.text_box.test(pos) {
        toolbar_button.enabled = !toolbar_button.enabled;
        return Err(false);
      };

      if toolbar_button.enabled {
        for button in &toolbar_button.buttons {
          if button.text_box.test(pos) {
            toolbar_button.enabled = false;
            return Ok(button.id);
          };
        };
      };
    };

    Err(true)
  }

  pub fn on_mouse_position(&mut self, pos: Vector2<f64>) {
    for toolbar_button in &mut self.toolbar_buttons {

      if toolbar_button.enabled && !toolbar_button.test(pos) {
        toolbar_button.enabled = false;

        for toolbar_button in &mut self.toolbar_buttons {
          if toolbar_button.text_box.test(pos) {
            toolbar_button.enabled = true;
          };
        };

        break;
      };
    };
  }

  pub fn draw(&self, ctx: Context, pos: Option<Vector2<f64>>, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    for button in &self.buttons {
      button.draw(ctx, pos, glyph_cache, gl);
    };

    let toolbar_colors = self.toolbar_buttons[0].text_box.colors();
    self.toolbar_plate.draw(ctx, false, toolbar_colors, gl);

    for toolbar_button in &self.toolbar_buttons {
      if toolbar_button.enabled {
        toolbar_button.text_box.draw(ctx, true, glyph_cache, gl);

        for button in &toolbar_button.buttons {
          button.draw(ctx, pos, glyph_cache, gl);
        };
      } else {
        let hover = toolbar_button.text_box.test_maybe(pos);
        toolbar_button.text_box.draw(ctx, hover, glyph_cache, gl);
      };
    };
  }
}

#[derive(Debug, Clone)]
struct ButtonElement {
  text_box: TextBox,
  id: ButtonId
}

impl ButtonElement {
  fn test(&self, pos: Vector2<f64>) -> bool {
    self.text_box.test(pos)
  }

  fn draw(&self, ctx: Context, pos: Option<Vector2<f64>>, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    self.text_box.draw(ctx, self.text_box.test_maybe(pos), glyph_cache, gl);
  }
}

#[derive(Debug, Clone)]
struct ToolbarButtonElement {
  text_box: TextBox,
  buttons: Vec<ButtonElement>,
  enabled: bool
}

impl ToolbarButtonElement {
  fn test(&self, pos: Vector2<f64>) -> bool {
    self.text_box.test(pos) || self.buttons.iter().any(|button| button.test(pos))
  }
}

#[derive(Debug, Clone)]
struct Metrics {
  width: f64,
  ascent: f64,
  descent: f64
}

impl Metrics {
  pub fn from_font(font: &Font<'static>) -> Metrics {
    let scale = (FONT_SIZE as f32 * 1.333).round();
    let scale = Scale { x: scale, y: scale };
    let h_metrics = font.glyph('_').scaled(scale).h_metrics();
    let v_metrics = font.v_metrics(scale);
    Metrics {
      width: h_metrics.advance_width as f64,
      ascent: v_metrics.ascent as f64,
      descent: v_metrics.descent as f64
    }
  }
}

#[derive(Debug, Clone)]
enum TextBox {
  BoxFitWidth {
    text: TextComponent,
    plate: PlateComponent,
    colors: &'static Palette
  },
  BoxDoubleText {
    text_left: TextComponent,
    text_right: TextComponent,
    plate: PlateComponent,
    colors: &'static Palette
  }
}

impl TextBox {
  fn new_fit_width(metrics: &Metrics, text: &'static str, pos: Vector2<u32>, colors: &'static Palette) -> Self {
    let text_pos = [pos[0] as f64 + PADDING[0], pos[1] as f64 + PADDING[1] + metrics.ascent];
    let plate_pos = [pos[0] as f64, pos[1] as f64];
    let plate_width = (metrics.width * text.len() as f64 + PADDING[0] * 2.0).round();
    let plate_height = (metrics.ascent - metrics.descent + PADDING[1] * 2.0).round();
    TextBox::BoxFitWidth {
      text: TextComponent { pos: text_pos, text },
      plate: PlateComponent { pos: plate_pos, size: [plate_width, plate_height] },
      colors
    }
  }

  fn new_double_text(metrics: &Metrics, text: [&'static str; 2], pos: Vector2<u32>, width: u32, colors: &'static Palette) -> Self {
    let text_y = pos[1] as f64 + PADDING[1] + metrics.ascent;
    let text_pos_left = [pos[0] as f64 + PADDING[0], text_y];
    let text_width_right = metrics.width * text[1].len() as f64;
    let text_pos_right = [pos[0] as f64 + width as f64 - text_width_right - PADDING[0], text_y];
    let plate_pos = [pos[0] as f64, pos[1] as f64];
    let plate_height = (metrics.ascent - metrics.descent + PADDING[1] * 2.0).round();
    TextBox::BoxDoubleText {
      text_left: TextComponent { pos: text_pos_left, text: text[0] },
      text_right: TextComponent { pos: text_pos_right, text: text[1] },
      plate: PlateComponent { pos: plate_pos, size: [width as f64, plate_height] },
      colors
    }
  }

  fn width(&self) -> u32 {
    self.plate().size[0] as u32
  }

  fn height(&self) -> u32 {
    self.plate().size[1] as u32
  }

  fn test_maybe(&self, pos: Option<Vector2<f64>>) -> bool {
    if let Some(pos) = pos {
      self.test(pos)
    } else {
      false
    }
  }

  fn test(&self, pos: Vector2<f64>) -> bool {
    self.plate().test(pos)
  }

  fn draw(&self, ctx: Context, hover: bool, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    match self {
      TextBox::BoxFitWidth { text, plate, colors } => {
        plate.draw(ctx, hover, colors, gl);
        text.draw(ctx, colors, glyph_cache, gl);
      },
      TextBox::BoxDoubleText { text_left, text_right, plate, colors } => {
        plate.draw(ctx, hover, colors, gl);
        text_left.draw(ctx, colors, glyph_cache, gl);
        text_right.draw(ctx, colors, glyph_cache, gl);
      }
    }
  }

  fn plate(&self) -> &PlateComponent {
    match self {
      TextBox::BoxFitWidth { plate, .. } => plate,
      TextBox::BoxDoubleText { plate, .. } => plate
    }
  }

  fn colors(&self) -> &'static Palette {
    match self {
      TextBox::BoxFitWidth { colors, .. } => colors,
      TextBox::BoxDoubleText { colors, .. } => colors
    }
  }
}

#[derive(Debug, Clone, Copy)]
struct TextComponent {
  pos: Vector2<f64>,
  text: &'static str
}

impl TextComponent {
  fn draw(&self, ctx: Context, colors: &Palette, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    let transform = ctx.transform.trans_pos(self.pos);
    graphics::text(colors.foreground, FONT_SIZE, self.text, glyph_cache, transform, gl)
      .expect("unable to draw text");
  }
}

#[derive(Debug, Clone, Copy)]
struct PlateComponent {
  pos: Vector2<f64>,
  size: Vector2<f64>
}

impl PlateComponent {
  fn draw(&self, ctx: Context, hover: bool, colors: &Palette, gl: &mut GlGraphics) {
    let color = if hover { colors.background_hover } else { colors.background };
    graphics::rectangle(color, [self.pos[0], self.pos[1], self.size[0], self.size[1]], ctx.transform, gl);
  }

  fn test(&self, pos: Vector2<f64>) -> bool {
    let upper = vecmath::vec2_add(self.pos, self.size);
    pos[0] >= self.pos[0] && pos[1] >= self.pos[1] &&
    pos[0] < upper[0] && pos[1] < upper[1]
  }
}

#[derive(Debug, Clone)]
struct Palette {
  foreground: Color,
  background: Color,
  background_hover: Color
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ButtonId {
  ToolbarFileOpenFileArchive,
  ToolbarFileOpenFolder,
  ToolbarFileSave,
  ToolbarFileSaveAsArchive,
  ToolbarFileSaveAsFolder,
  ToolbarFileReveal,
  ToolbarEditUndo,
  ToolbarEditRedo,
  ToolbarEditCoastal,
  ToolbarEditRecolor,
  ToolbarEditProblems,
  ToolbarViewMode1,
  ToolbarViewMode2,
  ToolbarViewMode3,
  ToolbarViewMode4,
  ToolbarViewMode5,
  ToolbarViewResetZoom
}

type ToolbarButtonPrimitive<'a> = (&'a str, &'a [(&'a str, &'a str, ButtonId)]);
type ToolbarPrimitive<'a> = &'a [ToolbarButtonPrimitive<'a>];

const TOOLBAR_WIDTH: u32 = 320;
const TOOLBAR_PRIMITIVE: ToolbarPrimitive<'static> = &[
  ("File", &[
    ("Open File or Archive...", "Ctrl+Alt+O", ButtonId::ToolbarFileOpenFileArchive),
    ("Open Folder...", "Ctrl+O", ButtonId::ToolbarFileOpenFolder),
    ("Save", "Ctrl+S", ButtonId::ToolbarFileSave),
    ("Save As Archive...", "Ctrl+Shift+Alt+S", ButtonId::ToolbarFileSaveAsArchive),
    ("Save As...", "Ctrl+Shift+S", ButtonId::ToolbarFileSaveAsFolder),
    ("Reveal in File Browser", "Ctrl+Alt+R", ButtonId::ToolbarFileReveal),
  ]),
  ("Edit", &[
    ("Undo", "Ctrl+Z", ButtonId::ToolbarEditUndo),
    ("Redo", "Ctrl+Y", ButtonId::ToolbarEditRedo),
    ("Re-calculate Coastal Provinces", "Shift+C", ButtonId::ToolbarEditCoastal),
    ("Re-color Provinces", "Shift+R", ButtonId::ToolbarEditRecolor),
    ("Calculate Map Errors/Warnings", "Shift+P", ButtonId::ToolbarEditProblems),
  ]),
  ("View", &[
    ("Color/Province Map View Mode", "1", ButtonId::ToolbarViewMode1),
    ("Terrain/Biome Map View Mode", "2", ButtonId::ToolbarViewMode2),
    ("Land Type Map View Mode", "3", ButtonId::ToolbarViewMode3),
    ("Continents Map View Mode", "4", ButtonId::ToolbarViewMode4),
    ("Coastal Provinces Map View Mode", "5", ButtonId::ToolbarViewMode5),
    ("Reset Zoom", "H", ButtonId::ToolbarViewResetZoom),
  ])
];

pub fn construct_interface(font: &Font<'static>) -> Interface {
  let metrics = Metrics::from_font(font);
  let mut toolbar_buttons = Vec::with_capacity(TOOLBAR_PRIMITIVE.len());

  let mut pos_x = 0;
  for &(toolbar_button_text, toolbar_primitive_buttons) in TOOLBAR_PRIMITIVE {
    let text_box = TextBox::new_fit_width(&metrics, toolbar_button_text, [pos_x, 0], &PALETTE_BUTTON_TOOLBAR);
    let mut buttons = Vec::with_capacity(toolbar_primitive_buttons.len());

    let mut pos_y = text_box.height() as u32;
    for &(button_text_left, button_text_right, id) in toolbar_primitive_buttons {
      let text = [button_text_left, button_text_right];
      let text_box = TextBox::new_double_text(&metrics, text, [pos_x, pos_y], TOOLBAR_WIDTH, &PALETTE_BUTTON);
      pos_y += text_box.height() as u32;

      buttons.push(ButtonElement { text_box, id });
    };

    pos_x += text_box.width() as u32;

    toolbar_buttons.push(ToolbarButtonElement {
      text_box, buttons, enabled: false
    });
  };

  let toolbar_plate_size = [crate::WINDOW_WIDTH as f64, toolbar_buttons[0].text_box.height() as f64];
  let toolbar_plate = PlateComponent { pos: [0.0, 0.0], size: toolbar_plate_size };

  Interface {
    buttons: Vec::new(),
    toolbar_buttons,
    toolbar_plate
  }
}
