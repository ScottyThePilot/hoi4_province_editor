//! Code regarding buttons and interactive elements on the screen
use graphics::Transformed;
use graphics::context::Context;
use graphics::types::Color;
use image::{DynamicImage, GenericImageView, RgbaImage};
use image::codecs::png::PngDecoder;
use once_cell::sync::{Lazy, OnceCell};
use opengl_graphics::{Texture, TextureSettings, GlGraphics};
use vecmath::Vector2;

use crate::font::{self, FONT_SIZE};
use super::canvas::ViewMode;
use super::colors;
use super::{FontGlyphCache, InterfaceDrawContext};

use std::sync::Arc;
use std::fmt;

pub const PADDING: Vector2<f64> = [6.0, 4.0];

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
  toolbar_buttons: Vec<ToolbarButtonElement>,
  toolbar_plate: PlateComponent,
  sidebar_buttons: Vec<ButtonElement>,
  sidebar_plate: PlateComponent
}

impl Interface {
  /// Called when the mouse is clicked to act on the interface and change its state.
  /// If a button was clicked, `Ok` is returned with the appropriate button ID.
  /// If a button was not clicked, a boolean is returned indicating whether or not
  /// the input just processed should be deferred to something below the interface.
  pub fn on_mouse_click(&mut self, pos: Vector2<f64>) -> Result<ButtonId, bool> {
    for sidebar_button in &self.sidebar_buttons {
      if sidebar_button.base.test(pos) {
        return Ok(sidebar_button.id);
      };
    };

    for toolbar_button in &mut self.toolbar_buttons {
      if toolbar_button.base.test(pos) {
        toolbar_button.enabled = !toolbar_button.enabled;
        return Err(false);
      };

      if toolbar_button.enabled {
        for button in &toolbar_button.buttons {
          if button.base.test(pos) {
            toolbar_button.enabled = false;
            return Ok(button.id);
          };
        };
      };
    };

    let hit_deadzone = self.toolbar_plate.test(pos) || self.sidebar_plate.test(pos);

    Err(!hit_deadzone)
  }

  pub fn on_mouse_position(&mut self, pos: Vector2<f64>) {
    for toolbar_button in &mut self.toolbar_buttons {

      if toolbar_button.enabled && !toolbar_button.test(pos) {
        toolbar_button.enabled = false;

        for toolbar_button in &mut self.toolbar_buttons {
          if toolbar_button.base.test(pos) {
            toolbar_button.enabled = true;
          };
        };

        break;
      };
    };
  }

  pub fn draw(
    &self,
    ctx: Context,
    ictx: InterfaceDrawContext,
    pos: Option<Vector2<f64>>,
    glyph_cache: &mut FontGlyphCache,
    gl: &mut GlGraphics
  ) {
    let sidebar_colors = self.sidebar_buttons[0].base.colors();
    self.sidebar_plate.draw(ctx, false, sidebar_colors, gl);
    for (i, sidebar_button) in self.sidebar_buttons.iter().enumerate() {
      let selected_tool = match (ictx.view_mode, i) {
        (Some(ViewMode::Color), _) => ictx.selected_tool,
        (Some(ViewMode::Coastal), _) => continue,
        (Some(ViewMode::Adjacencies), _) => continue,
        (Some(_), 0) => Some(0),
        (_, _) => continue
      };

      let hover = Some(i) == selected_tool || sidebar_button.base.test_maybe(pos);
      sidebar_button.base.draw(ctx, hover, glyph_cache, gl);
    };

    let toolbar_colors = self.toolbar_buttons[0].base.colors();
    self.toolbar_plate.draw(ctx, false, toolbar_colors, gl);

    for toolbar_button in &self.toolbar_buttons {
      if toolbar_button.enabled {
        toolbar_button.base.draw(ctx, true, glyph_cache, gl);

        for button in &toolbar_button.buttons {
          button.draw(ctx, pos, glyph_cache, gl);
        };
      } else {
        let hover = toolbar_button.base.test_maybe(pos);
        toolbar_button.base.draw(ctx, hover, glyph_cache, gl);
      };
    };
  }
}

#[derive(Debug, Clone)]
struct ButtonElement {
  base: ButtonBase,
  id: ButtonId
}

impl ButtonElement {
  fn test(&self, pos: Vector2<f64>) -> bool {
    self.base.test(pos)
  }

  fn draw(&self, ctx: Context, pos: Option<Vector2<f64>>, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    self.base.draw(ctx, self.base.test_maybe(pos), glyph_cache, gl);
  }
}

#[derive(Debug, Clone)]
struct ToolbarButtonElement {
  base: ButtonBase,
  buttons: Vec<ButtonElement>,
  enabled: bool
}

impl ToolbarButtonElement {
  fn test(&self, pos: Vector2<f64>) -> bool {
    self.base.test(pos) || self.buttons.iter().any(|button| button.test(pos))
  }
}

#[derive(Debug, Clone)]
enum ButtonBase {
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
  },
  BoxTexture {
    texture: TextureComponent,
    plate: PlateComponent,
    colors: &'static Palette
  }
}

impl ButtonBase {
  fn new_fit_width(text: &'static str, pos: Vector2<u32>, colors: &'static Palette) -> Self {
    let v_metrics = font::get_v_metrics();
    let text_pos = [pos[0] as f64 + PADDING[0], pos[1] as f64 + PADDING[1] + v_metrics.ascent];
    let plate_pos = [pos[0] as f64, pos[1] as f64];
    let plate_width = (font::get_width_metric_str(text) + PADDING[0] * 2.0).round();
    let plate_height = (v_metrics.ascent - v_metrics.descent + PADDING[1] * 2.0).round();
    ButtonBase::BoxFitWidth {
      text: TextComponent { pos: text_pos, text },
      plate: PlateComponent { pos: plate_pos, size: [plate_width, plate_height] },
      colors
    }
  }

  fn new_double_text(text: [&'static str; 2], pos: Vector2<u32>, width: u32, colors: &'static Palette) -> Self {
    let v_metrics = font::get_v_metrics();
    let text_y = pos[1] as f64 + PADDING[1] + v_metrics.ascent;
    let text_pos_left = [pos[0] as f64 + PADDING[0], text_y];
    let text_width_right = font::get_width_metric_str(text[1]);
    let text_pos_right = [pos[0] as f64 + width as f64 - text_width_right - PADDING[0], text_y];
    let plate_pos = [pos[0] as f64, pos[1] as f64];
    let plate_height = (v_metrics.ascent - v_metrics.descent + PADDING[1] * 2.0).round();
    ButtonBase::BoxDoubleText {
      text_left: TextComponent { pos: text_pos_left, text: text[0] },
      text_right: TextComponent { pos: text_pos_right, text: text[1] },
      plate: PlateComponent { pos: plate_pos, size: [width as f64, plate_height] },
      colors
    }
  }

  fn new_texture(sprite_coords: [u32; 4], pos: Vector2<u32>, colors: &'static Palette) -> Self {
    let pad = f64::min(PADDING[0], PADDING[1]);
    let texture = Arc::new(get_sprite(sprite_coords));
    let texture_pos = [pos[0] as f64 + pad, pos[1] as f64 + pad];
    let plate_pos = [pos[0] as f64, pos[1] as f64];
    let size = [sprite_coords[2] as f64 + pad * 2.0, sprite_coords[3] as f64 + pad * 2.0];
    ButtonBase::BoxTexture {
      texture: TextureComponent { pos: texture_pos, texture },
      plate: PlateComponent { pos: plate_pos, size },
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
      ButtonBase::BoxFitWidth { text, plate, colors } => {
        plate.draw(ctx, hover, colors, gl);
        text.draw(ctx, colors, glyph_cache, gl);
      },
      ButtonBase::BoxDoubleText { text_left, text_right, plate, colors } => {
        plate.draw(ctx, hover, colors, gl);
        text_left.draw(ctx, colors, glyph_cache, gl);
        text_right.draw(ctx, colors, glyph_cache, gl);
      },
      ButtonBase::BoxTexture { texture, plate, colors } => {
        plate.draw(ctx, hover, colors, gl);
        texture.draw(ctx, gl);
      }
    }
  }

  fn plate(&self) -> &PlateComponent {
    match self {
      ButtonBase::BoxFitWidth { plate, .. } => plate,
      ButtonBase::BoxDoubleText { plate, .. } => plate,
      ButtonBase::BoxTexture { plate, .. } => plate
    }
  }

  fn colors(&self) -> &'static Palette {
    match self {
      ButtonBase::BoxFitWidth { colors, .. } => colors,
      ButtonBase::BoxDoubleText { colors, .. } => colors,
      ButtonBase::BoxTexture { colors, .. } => colors
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

#[derive(Clone)]
struct TextureComponent {
  pos: Vector2<f64>,
  texture: Arc<Texture>
}

impl TextureComponent {
  fn draw(&self, ctx: Context, gl: &mut GlGraphics) {
    let transform = ctx.transform.trans_pos(self.pos);
    graphics::image(&*self.texture, transform, gl);
  }
}

impl fmt::Debug for TextureComponent {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("TextureComponent")
      .field("pos", &self.pos)
      .field("texture", &format_args!("..."))
      .finish()
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
  ToolbarFileExportLandMap,
  ToolbarFileExportTerrainMap,
  ToolbarEditUndo,
  ToolbarEditRedo,
  ToolbarEditCoastal,
  ToolbarEditRecolor,
  ToolbarEditProblems,
  ToolbarEditToggleLassoSnap,
  ToolbarEditNextMaskMode,
  ToolbarViewMode1,
  ToolbarViewMode2,
  ToolbarViewMode3,
  ToolbarViewMode4,
  ToolbarViewMode5,
  ToolbarViewMode6,
  ToolbarViewToggleIds,
  ToolbarViewResetZoom,
  #[cfg(debug_assertions)]
  ToolbarDebugValidatePixelCounts,
  #[cfg(debug_assertions)]
  ToolbarDebugTriggerCrash,
  SidebarToolPaintArea,
  SidebarToolPaintBucket,
  SidebarToolLasso
}

type ToolbarButtonPrimitive<'a> = (&'a str, &'a [(&'a str, &'a str, ButtonId)]);
type ToolbarPrimitive<'a> = &'a [ToolbarButtonPrimitive<'a>];
type SidebarPrimitive<'a> = &'a [([u32; 4], ButtonId)];

const TOOLBAR_DROPDOWN_WIDTH: u32 = 320;
const TOOLBAR_PRIMITIVE: ToolbarPrimitive<'static> = &[
  ("File", &[
    ("Open File or Archive...", "Ctrl+Alt+O", ButtonId::ToolbarFileOpenFileArchive),
    ("Open Folder...", "Ctrl+O", ButtonId::ToolbarFileOpenFolder),
    ("Save", "Ctrl+S", ButtonId::ToolbarFileSave),
    ("Save As Archive...", "Ctrl+Shift+Alt+S", ButtonId::ToolbarFileSaveAsArchive),
    ("Save As...", "Ctrl+Shift+S", ButtonId::ToolbarFileSaveAsFolder),
    ("Reveal in File Browser", "Ctrl+Alt+R", ButtonId::ToolbarFileReveal),
    ("Export Land Map...", "", ButtonId::ToolbarFileExportLandMap),
    ("Export Terrain Map...", "", ButtonId::ToolbarFileExportTerrainMap)
  ]),
  ("Edit", &[
    ("Undo", "Ctrl+Z", ButtonId::ToolbarEditUndo),
    ("Redo", "Ctrl+Y", ButtonId::ToolbarEditRedo),
    ("Re-calculate Coastal Provinces", "Shift+C", ButtonId::ToolbarEditCoastal),
    ("Re-color Provinces", "Shift+R", ButtonId::ToolbarEditRecolor),
    ("Calculate Map Errors/Warnings", "Shift+P", ButtonId::ToolbarEditProblems),
    ("Toggle Lasso Pixel Snap", "", ButtonId::ToolbarEditToggleLassoSnap),
    ("Next Brush Mask Mode", "Shift+M", ButtonId::ToolbarEditNextMaskMode)
  ]),
  ("View", &[
    ("Color/Province Map View Mode", "1", ButtonId::ToolbarViewMode1),
    ("Terrain/Biome Map View Mode", "2", ButtonId::ToolbarViewMode2),
    ("Land Type Map View Mode", "3", ButtonId::ToolbarViewMode3),
    ("Continents Map View Mode", "4", ButtonId::ToolbarViewMode4),
    ("Coastal Provinces Map View Mode", "5", ButtonId::ToolbarViewMode5),
    ("Adjacencies Map View Mode", "6", ButtonId::ToolbarViewMode6),
    ("Toggle Province IDs", "", ButtonId::ToolbarViewToggleIds),
    ("Reset Zoom", "H", ButtonId::ToolbarViewResetZoom)
  ]),
  #[cfg(debug_assertions)]
  ("Debug", &[
    ("Validate Pixel Counts", "", ButtonId::ToolbarDebugValidatePixelCounts),
    ("Trigger a Crash", "", ButtonId::ToolbarDebugTriggerCrash)
  ])
];

const SIDEBAR_PRIMITIVE: SidebarPrimitive<'static> = &[
  ([0,  0, 20, 20], ButtonId::SidebarToolPaintArea),
  ([20, 0, 20, 20], ButtonId::SidebarToolPaintBucket),
  ([40, 0, 20, 20], ButtonId::SidebarToolLasso)
];

static TOOLBAR_HEIGHT: OnceCell<u32> = OnceCell::new();
static SIDEBAR_WIDTH: OnceCell<u32> = OnceCell::new();

pub fn construct_interface() -> Interface {
  let mut pos_x = 0;
  let mut toolbar_height = 0;
  let mut toolbar_buttons = Vec::with_capacity(TOOLBAR_PRIMITIVE.len());
  for &(toolbar_button_text, toolbar_primitive_buttons) in TOOLBAR_PRIMITIVE {
    let mut buttons = Vec::with_capacity(toolbar_primitive_buttons.len());
    let base = ButtonBase::new_fit_width(toolbar_button_text, [pos_x, 0], &PALETTE_BUTTON_TOOLBAR);

    let mut pos_y = base.height();
    for &(button_text_left, button_text_right, id) in toolbar_primitive_buttons {
      let text = [button_text_left, button_text_right];
      let base = ButtonBase::new_double_text(text, [pos_x, pos_y], TOOLBAR_DROPDOWN_WIDTH, &PALETTE_BUTTON);
      pos_y += base.height();

      buttons.push(ButtonElement { base, id });
    };

    toolbar_height = base.height();
    pos_x += base.width();

    toolbar_buttons.push(ToolbarButtonElement {
      base, buttons, enabled: false
    });
  };

  let mut pos_y = toolbar_height;
  let mut sidebar_width = 0;
  let mut buttons = Vec::with_capacity(SIDEBAR_PRIMITIVE.len());
  for &(sprite_coords, id) in SIDEBAR_PRIMITIVE {
    let base = ButtonBase::new_texture(sprite_coords, [0, pos_y], &PALETTE_BUTTON);

    sidebar_width = base.width();
    pos_y += base.height();

    buttons.push(ButtonElement { base, id });
  };

  let toolbar_plate_size = [crate::WINDOW_WIDTH as f64, toolbar_height as f64];
  let toolbar_plate = PlateComponent { pos: [0.0, 0.0], size: toolbar_plate_size };

  let sidebar_plate_size = [sidebar_width as f64, crate::WINDOW_HEIGHT as f64];
  let sidebar_plate = PlateComponent { pos: [0.0, toolbar_height as f64], size: sidebar_plate_size };

  let _ = TOOLBAR_HEIGHT.set(toolbar_height);
  let _ = SIDEBAR_WIDTH.set(sidebar_width);

  Interface {
    sidebar_buttons: buttons,
    toolbar_buttons,
    toolbar_plate,
    sidebar_plate
  }
}

#[inline]
pub fn get_toolbar_height() -> f64 {
  TOOLBAR_HEIGHT.get().map_or(20, u32::clone) as f64
}

#[inline]
pub fn get_sidebar_width() -> f64 {
  SIDEBAR_WIDTH.get().map_or(28, u32::clone) as f64
}

fn get_sprite(sprite_coords: [u32; 4]) -> Texture {
  const SPRITESHEET_DATA: &[u8] = include_bytes!("../../assets/spritesheet.png");
  static SPRITESHEET: Lazy<RgbaImage> = Lazy::new(|| {
    let decoder = PngDecoder::new(SPRITESHEET_DATA)
      .expect("unable to decode spritesheet");
    let img = DynamicImage::from_decoder(decoder)
      .expect("unable to decode spritesheet");
    img.to_rgba8()
  });

  let [x, y, width, height] = sprite_coords;
  let view = SPRITESHEET.view(x, y, width, height);
  Texture::from_image(&view.to_image(), &TextureSettings::new())
}
