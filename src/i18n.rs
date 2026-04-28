use std::sync::atomic::{AtomicU8, Ordering};

use crate::config::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
  En,
  ZhCn,
}

impl From<Language> for Lang {
  fn from(value: Language) -> Self {
    match value {
      Language::En => Self::En,
      Language::ZhCn => Self::ZhCn,
    }
  }
}

pub fn set_lang(language: Language) {
  let lang = Lang::from(language);
  LANG.store(match lang {
    Lang::En => 0,
    Lang::ZhCn => 1,
  }, Ordering::Relaxed);
}

pub fn lang() -> Lang {
  match LANG.load(Ordering::Relaxed) {
    1 => Lang::ZhCn,
    _ => Lang::En,
  }
}

pub fn text() -> &'static Strings {
  match lang() {
    Lang::En => &STRINGS_EN,
    Lang::ZhCn => &STRINGS_ZH_CN,
  }
}

pub fn error_message(err: impl std::fmt::Display) -> String {
  format!("{}{}", text().error_prefix, err)
}

pub fn loaded_map_from(location: impl std::fmt::Display) -> String {
  format!("{}{}", text().loaded_map_from_prefix, location)
}

pub fn saved_map_to(location: impl std::fmt::Display) -> String {
  format!("{}{}", text().saved_map_to_prefix, location)
}

pub fn exported_land_map_to(path: impl std::fmt::Display) -> String {
  format!("{}{}", text().exported_land_map_to_prefix, path)
}

pub fn exported_terrain_map_to(path: impl std::fmt::Display) -> String {
  format!("{}{}", text().exported_terrain_map_to_prefix, path)
}

pub fn terrain_mode_unavailable(unknown_terrains: &str) -> String {
  format!("{}{}", text().terrain_mode_unavailable_prefix, unknown_terrains)
}

pub fn problem_message(problem: impl std::fmt::Display) -> String {
  format!("{}{}", text().problem_prefix, problem)
}

pub fn brush_set_to_color(color: &str) -> String {
  format!("{}{}", text().brush_set_to_color_prefix, color)
}

pub fn brush_set_to_type(kind: &str) -> String {
  format!("{}{}", text().brush_set_to_type_prefix, kind)
}

pub fn brush_set_to_terrain(terrain: &str) -> String {
  format!("{}{}", text().brush_set_to_terrain_prefix, terrain)
}

pub fn brush_set_to_continent(continent: u16) -> String {
  format!("{}{}", text().brush_set_to_continent_prefix, continent)
}

pub fn brush_set_to_adjacencies(kind: &str) -> String {
  format!("{}{}", text().brush_set_to_adjacencies_prefix, kind)
}

pub fn picked_color(color: &str) -> String {
  format!("{}{}", text().picked_color_prefix, color)
}

pub fn picked_type(kind: &str) -> String {
  format!("{}{}", text().picked_type_prefix, kind)
}

pub fn picked_terrain(terrain: &str) -> String {
  format!("{}{}", text().picked_terrain_prefix, terrain)
}

pub fn picked_continent(continent: u16) -> String {
  format!("{}{}", text().picked_continent_prefix, continent)
}

pub fn brush_info_color(color: &str) -> String {
  format!("{}{}", text().brush_info_color_prefix, color)
}

pub fn brush_info_type(kind: &str) -> String {
  format!("{}{}", text().brush_info_type_prefix, kind)
}

pub fn brush_info_terrain(terrain: &str) -> String {
  format!("{}{}", text().brush_info_terrain_prefix, terrain)
}

pub fn brush_info_continent(continent: u16) -> String {
  format!("{}{}", text().brush_info_continent_prefix, continent)
}

pub fn brush_info_adjacencies(kind: &str) -> String {
  format!("{}{}", text().brush_info_adjacencies_prefix, kind)
}

pub fn brush_mask(mask: &str) -> String {
  format!("{}{}", text().brush_mask_prefix, mask)
}

pub fn camera_cursor_info(x: u32, y: u32) -> String {
  format!("{}, {} {}", x, y, text().camera_cursor_suffix)
}

pub fn map_problem_invalid_x_crossing(pos: impl std::fmt::Debug) -> String {
  format!("{} {:?}", text().problem_invalid_x_crossing_prefix, pos)
}

pub fn map_problem_too_large_box(lower: impl std::fmt::Debug, upper: impl std::fmt::Debug) -> String {
  format!(
    "{} {:?} {} {:?}",
    text().problem_too_large_box_prefix,
    lower,
    text().problem_too_large_box_middle,
    upper,
  )
}

pub fn map_problem_too_few_pixels(count: u64, x: f64, y: f64) -> String {
  match lang() {
    Lang::En => format!("Province has only {} pixels around [{:.0}, {:.0}]", count, x, y),
    Lang::ZhCn => format!("省份在 [{:.0}, {:.0}] 附近仅有 {} 个像素", x, y, count),
  }
}

pub fn map_problem_lone_pixel(pos: impl std::fmt::Debug) -> String {
  format!("{} {:?}", text().problem_lone_pixel_prefix, pos)
}

pub fn map_problem_few_shared_borders(count: usize, a: &str, b: &str) -> String {
  match lang() {
    Lang::En => format!("Only {} shared borders between provinces {} and {}", count, a, b),
    Lang::ZhCn => format!("省份 {} 与 {} 之间仅有 {} 条共享边界", a, b, count),
  }
}

pub fn id_change_deleted(start: u32, end: u32) -> String {
  match lang() {
    Lang::En => format!("Deleted IDs {} through {}", start, end),
    Lang::ZhCn => format!("删除了 ID {} 到 {}", start, end),
  }
}

pub fn id_change_created(start: u32, end: u32) -> String {
  match lang() {
    Lang::En => format!("Created IDs {} through {}", start, end),
    Lang::ZhCn => format!("创建了 ID {} 到 {}", start, end),
  }
}

pub fn id_change_reassigned(from: u32, to: u32) -> String {
  match lang() {
    Lang::En => format!("Reassigned ID {} to {}", from, to),
    Lang::ZhCn => format!("将 ID {} 重新分配为 {}", from, to),
  }
}

pub fn id_change_assigned_new(id: u32) -> String {
  match lang() {
    Lang::En => format!("Assigned ID {} to new province", id),
    Lang::ZhCn => format!("将 ID {} 分配给新省份", id),
  }
}

static LANG: AtomicU8 = AtomicU8::new(0);

#[derive(Debug)]
pub struct Strings {
  pub menu_file: &'static str,
  pub menu_edit: &'static str,
  pub menu_view: &'static str,
  pub menu_debug: &'static str,
  pub open_file_archive: &'static str,
  pub open_folder: &'static str,
  pub save: &'static str,
  pub save_as_archive: &'static str,
  pub save_as: &'static str,
  pub reveal_in_file_browser: &'static str,
  pub export_land_map: &'static str,
  pub export_terrain_map: &'static str,
  pub undo: &'static str,
  pub redo: &'static str,
  pub recalculate_coastal_provinces: &'static str,
  pub recolor_provinces: &'static str,
  pub calculate_map_errors_warnings: &'static str,
  pub toggle_lasso_pixel_snap: &'static str,
  pub next_brush_mask_mode: &'static str,
  pub color_view_mode: &'static str,
  pub terrain_view_mode: &'static str,
  pub land_type_view_mode: &'static str,
  pub continents_view_mode: &'static str,
  pub coastal_view_mode: &'static str,
  pub adjacencies_view_mode: &'static str,
  pub toggle_province_ids: &'static str,
  pub toggle_province_boundaries: &'static str,
  pub toggle_rivers_overlay: &'static str,
  pub reset_zoom: &'static str,
  pub view_font_license: &'static str,
  pub validate_pixel_counts: &'static str,
  pub trigger_crash: &'static str,
  pub tooltip_paint_area_color: &'static str,
  pub tooltip_paint_area_kind: &'static str,
  pub tooltip_paint_area_terrain: &'static str,
  pub tooltip_paint_area_continent: &'static str,
  pub tooltip_paint_bucket: &'static str,
  pub tooltip_lasso: &'static str,
  pub tooltip_toggle_province_ids: &'static str,
  pub tooltip_toggle_province_boundaries: &'static str,
  pub tooltip_toggle_rivers_overlay: &'static str,
  pub drag_file_to_load: &'static str,
  pub rivers_required: &'static str,
  pub map_required: &'static str,
  pub no_adjacency_brush_selected: &'static str,
  pub loaded_map_from_prefix: &'static str,
  pub saved_map_to_prefix: &'static str,
  pub save_id_changes_line_1: &'static str,
  pub save_id_changes_line_2: &'static str,
  pub error_prefix: &'static str,
  pub bmp_filter_name: &'static str,
  pub zip_filter_name: &'static str,
  pub unsaved_changes_exit: &'static str,
  pub unsaved_changes: &'static str,
  pub reveal_file_browser_unavailable: &'static str,
  pub reloaded_config: &'static str,
  pub exported_land_map_to_prefix: &'static str,
  pub exported_terrain_map_to_prefix: &'static str,
  pub unknown_type_present: &'static str,
  pub no_map_problems_detected: &'static str,
  pub problem_prefix: &'static str,
  pub brush_set_to_color_prefix: &'static str,
  pub brush_set_to_type_prefix: &'static str,
  pub brush_set_to_terrain_prefix: &'static str,
  pub brush_set_to_continent_prefix: &'static str,
  pub brush_set_to_adjacencies_prefix: &'static str,
  pub picked_color_prefix: &'static str,
  pub picked_type_prefix: &'static str,
  pub picked_terrain_prefix: &'static str,
  pub picked_continent_prefix: &'static str,
  pub validation_successful: &'static str,
  pub validation_failed: &'static str,
  pub terrain_mode_unavailable_prefix: &'static str,
  pub brush_info_color_prefix: &'static str,
  pub brush_info_type_prefix: &'static str,
  pub brush_info_terrain_prefix: &'static str,
  pub brush_info_continent_prefix: &'static str,
  pub brush_info_adjacencies_prefix: &'static str,
  pub no_brush_suffix: &'static str,
  pub brush_info_coastal: &'static str,
  pub brush_mask_prefix: &'static str,
  pub no_mask: &'static str,
  pub camera_cursor_suffix: &'static str,
  pub problem_invalid_x_crossing_prefix: &'static str,
  pub problem_too_large_box_prefix: &'static str,
  pub problem_too_large_box_middle: &'static str,
  pub problem_too_few_pixels_fmt: &'static str,
  pub problem_invalid_width: &'static str,
  pub problem_invalid_height: &'static str,
  pub problem_lone_pixel_prefix: &'static str,
  pub problem_few_shared_borders_fmt: &'static str,
  pub id_change_deleted_fmt: &'static str,
  pub id_change_created_fmt: &'static str,
  pub id_change_reassigned_fmt: &'static str,
  pub id_change_assigned_new_fmt: &'static str,
}

pub const STRINGS_EN: Strings = Strings {
  menu_file: "File",
  menu_edit: "Edit",
  menu_view: "View",
  menu_debug: "Debug",
  open_file_archive: "Open File or Archive...",
  open_folder: "Open Folder...",
  save: "Save",
  save_as_archive: "Save As Archive...",
  save_as: "Save As...",
  reveal_in_file_browser: "Reveal in File Browser",
  export_land_map: "Export Land Map...",
  export_terrain_map: "Export Terrain Map...",
  undo: "Undo",
  redo: "Redo",
  recalculate_coastal_provinces: "Re-calculate Coastal Provinces",
  recolor_provinces: "Re-color Provinces",
  calculate_map_errors_warnings: "Calculate Map Errors/Warnings",
  toggle_lasso_pixel_snap: "Toggle Lasso Pixel Snap",
  next_brush_mask_mode: "Next Brush Mask Mode",
  color_view_mode: "Color/Province Map View Mode",
  terrain_view_mode: "Terrain/Biome Map View Mode",
  land_type_view_mode: "Land Type Map View Mode",
  continents_view_mode: "Continents Map View Mode",
  coastal_view_mode: "Coastal Provinces Map View Mode",
  adjacencies_view_mode: "Adjacencies Map View Mode",
  toggle_province_ids: "Toggle Province IDs",
  toggle_province_boundaries: "Toggle Province Boundaries",
  toggle_rivers_overlay: "Toggle Rivers Overlay",
  reset_zoom: "Reset Zoom",
  view_font_license: "View Inconsolata Open Font License",
  validate_pixel_counts: "Validate Pixel Counts",
  trigger_crash: "Trigger a Crash",
  tooltip_paint_area_color: "Paint Area: Drag to paint provinces under the brush",
  tooltip_paint_area_kind: "Paint Area: Drag to assign province types",
  tooltip_paint_area_terrain: "Paint Area: Drag to assign province terrain types",
  tooltip_paint_area_continent: "Paint Area: Drag to assign provinces to continents",
  tooltip_paint_bucket: "Paint Bucket: Fill the hovered province with the current brush",
  tooltip_lasso: "Lasso: Draw a custom selection and then apply the current brush",
  tooltip_toggle_province_ids: "Toggle Province IDs: Show or hide province IDs on the map",
  tooltip_toggle_province_boundaries: "Toggle Province Boundaries: Show or hide province borders",
  tooltip_toggle_rivers_overlay: "Toggle Rivers Overlay: Show or hide the contents of rivers.bmp",
  drag_file_to_load: "Drag a file, archive, or folder onto the application to load a map",
  rivers_required: "You must have a map with rivers.bmp to use this",
  map_required: "You must have a map loaded to use this",
  no_adjacency_brush_selected: "No Adjacency brush selected",
  loaded_map_from_prefix: "Loaded map from ",
  saved_map_to_prefix: "Saved map to ",
  save_id_changes_line_1: "The most recent save included modified province IDs, see 'id_changes.txt' for more info",
  save_id_changes_line_2: "If you do not need province IDs to be preserved, you may disable it in the config",
  error_prefix: "Error: ",
  bmp_filter_name: "24-bit Bitmap",
  zip_filter_name: "ZIP Archive",
  unsaved_changes_exit: "You have unsaved changes, would you like to save them before exiting?",
  unsaved_changes: "You have unsaved changes, would you like to save them?",
  reveal_file_browser_unavailable: "unable to reveal in file browser",
  reloaded_config: "Reloaded config",
  exported_land_map_to_prefix: "Exported land map to ",
  exported_terrain_map_to_prefix: "Exported terrain map to ",
  unknown_type_present: "Error: province with unknown type present",
  no_map_problems_detected: "No map problems detected",
  problem_prefix: "Problem: ",
  brush_set_to_color_prefix: "Brush set to color ",
  brush_set_to_type_prefix: "Brush set to type ",
  brush_set_to_terrain_prefix: "Brush set to terrain ",
  brush_set_to_continent_prefix: "Brush set to continent ",
  brush_set_to_adjacencies_prefix: "Brush set to adjacencies ",
  picked_color_prefix: "Picked color ",
  picked_type_prefix: "Picked type ",
  picked_terrain_prefix: "Picked terrain ",
  picked_continent_prefix: "Picked continent ",
  validation_successful: "Validation successful",
  validation_failed: "Validation failed",
  terrain_mode_unavailable_prefix: "Terrain mode unavailable, unknown terrains present: ",
  brush_info_color_prefix: "Color ",
  brush_info_type_prefix: "Type ",
  brush_info_terrain_prefix: "Terrain ",
  brush_info_continent_prefix: "Continent ",
  brush_info_adjacencies_prefix: "Adjacencies ",
  no_brush_suffix: "(No Brush)",
  brush_info_coastal: "Coastal",
  brush_mask_prefix: "Mask ",
  no_mask: "No Mask",
  camera_cursor_suffix: "px",
  problem_invalid_x_crossing_prefix: "Invalid X crossing at",
  problem_too_large_box_prefix: "Province has too large box from",
  problem_too_large_box_middle: "to",
  problem_too_few_pixels_fmt: "Province has only {} pixels around [{:.0}, {:.0}]",
  problem_invalid_width: "Map texture width is not a multiple of 64",
  problem_invalid_height: "Map texture height is not a multiple of 64",
  problem_lone_pixel_prefix: "Lone pixel at",
  problem_few_shared_borders_fmt: "Only {} shared borders between provinces {} and {}",
  id_change_deleted_fmt: "Deleted IDs {} through {}",
  id_change_created_fmt: "Created IDs {} through {}",
  id_change_reassigned_fmt: "Reassigned ID {} to {}",
  id_change_assigned_new_fmt: "Assigned ID {} to new province",
};

pub const STRINGS_ZH_CN: Strings = Strings {
  menu_file: "文件",
  menu_edit: "编辑",
  menu_view: "视图",
  menu_debug: "调试",
  open_file_archive: "打开文件或压缩包...",
  open_folder: "打开文件夹...",
  save: "保存",
  save_as_archive: "另存为压缩包...",
  save_as: "另存为...",
  reveal_in_file_browser: "在文件管理器中显示",
  export_land_map: "导出陆地区域图...",
  export_terrain_map: "导出地形图...",
  undo: "撤销",
  redo: "重做",
  recalculate_coastal_provinces: "重新计算沿海省份",
  recolor_provinces: "重新着色省份",
  calculate_map_errors_warnings: "检查地图错误和警告",
  toggle_lasso_pixel_snap: "切换套索像素吸附",
  next_brush_mask_mode: "切换下一种画笔遮罩模式",
  color_view_mode: "颜色/省份视图模式",
  terrain_view_mode: "地形/生物群系视图模式",
  land_type_view_mode: "地块类型视图模式",
  continents_view_mode: "大陆视图模式",
  coastal_view_mode: "沿海省份视图模式",
  adjacencies_view_mode: "邻接关系视图模式",
  toggle_province_ids: "切换省份 ID 显示",
  toggle_province_boundaries: "切换省份边界显示",
  toggle_rivers_overlay: "切换河流覆盖层显示",
  reset_zoom: "重置缩放",
  view_font_license: "查看 Inconsolata 字体许可",
  validate_pixel_counts: "校验像素计数",
  trigger_crash: "触发崩溃",
  tooltip_paint_area_color: "区域绘制：拖动以在画笔覆盖范围内涂改省份",
  tooltip_paint_area_kind: "区域绘制：拖动以设置省份类型",
  tooltip_paint_area_terrain: "区域绘制：拖动以设置省份地形类型",
  tooltip_paint_area_continent: "区域绘制：拖动以设置省份所属大陆",
  tooltip_paint_bucket: "油漆桶：用当前画笔填充悬停的省份",
  tooltip_lasso: "套索：绘制自定义选区，然后应用当前画笔",
  tooltip_toggle_province_ids: "切换省份 ID：显示或隐藏地图上的省份 ID",
  tooltip_toggle_province_boundaries: "切换省份边界：显示或隐藏省份边界线",
  tooltip_toggle_rivers_overlay: "切换河流覆盖层：显示或隐藏 rivers.bmp 的内容",
  drag_file_to_load: "将文件、压缩包或文件夹拖到程序窗口中以加载地图",
  rivers_required: "当前地图必须包含 rivers.bmp 才能使用此功能",
  map_required: "必须先加载地图才能使用此功能",
  no_adjacency_brush_selected: "尚未选择邻接画笔",
  loaded_map_from_prefix: "已加载地图：",
  saved_map_to_prefix: "已保存地图到：",
  save_id_changes_line_1: "本次保存包含省份 ID 变更，详情请查看 'id_changes.txt'",
  save_id_changes_line_2: "如果不需要保留省份 ID，可以在配置中关闭该选项",
  error_prefix: "错误：",
  bmp_filter_name: "24 位位图",
  zip_filter_name: "ZIP 压缩包",
  unsaved_changes_exit: "有未保存的更改，退出前是否保存？",
  unsaved_changes: "有未保存的更改，是否保存？",
  reveal_file_browser_unavailable: "无法在文件管理器中显示该路径",
  reloaded_config: "已重新加载配置",
  exported_land_map_to_prefix: "已导出陆地区域图到：",
  exported_terrain_map_to_prefix: "已导出地形图到：",
  unknown_type_present: "错误：存在未知类型的省份",
  no_map_problems_detected: "未发现地图问题",
  problem_prefix: "问题：",
  brush_set_to_color_prefix: "画笔颜色已设为 ",
  brush_set_to_type_prefix: "画笔类型已设为 ",
  brush_set_to_terrain_prefix: "画笔地形已设为 ",
  brush_set_to_continent_prefix: "画笔大陆已设为 ",
  brush_set_to_adjacencies_prefix: "画笔邻接类型已设为 ",
  picked_color_prefix: "已拾取颜色 ",
  picked_type_prefix: "已拾取类型 ",
  picked_terrain_prefix: "已拾取地形 ",
  picked_continent_prefix: "已拾取大陆 ",
  validation_successful: "校验成功",
  validation_failed: "校验失败",
  terrain_mode_unavailable_prefix: "地形模式不可用，存在未知地形：",
  brush_info_color_prefix: "颜色 ",
  brush_info_type_prefix: "类型 ",
  brush_info_terrain_prefix: "地形 ",
  brush_info_continent_prefix: "大陆 ",
  brush_info_adjacencies_prefix: "邻接 ",
  no_brush_suffix: "(未选择)",
  brush_info_coastal: "沿海",
  brush_mask_prefix: "遮罩 ",
  no_mask: "无遮罩",
  camera_cursor_suffix: "像素",
  problem_invalid_x_crossing_prefix: "无效 X 交叉位置",
  problem_too_large_box_prefix: "省份包围盒过大，范围从",
  problem_too_large_box_middle: "到",
  problem_too_few_pixels_fmt: "省份在 [{:.0}, {:.0}] 附近仅有 {} 个像素",
  problem_invalid_width: "地图纹理宽度不是 64 的倍数",
  problem_invalid_height: "地图纹理高度不是 64 的倍数",
  problem_lone_pixel_prefix: "孤立像素位置",
  problem_few_shared_borders_fmt: "省份 {} 与 {} 之间仅有 {} 条共享边界",
  id_change_deleted_fmt: "删除了 ID {} 到 {}",
  id_change_created_fmt: "创建了 ID {} 到 {}",
  id_change_reassigned_fmt: "将 ID {} 重新分配为 {}",
  id_change_assigned_new_fmt: "将 ID {} 分配给新省份",
};
