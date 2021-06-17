use super::console::ConsoleHandle;
use super::canvas::{Canvas, ViewMode};
use super::format::DefinitionKind;

pub fn line(line: String, mut console: ConsoleHandle, canvas: Option<&mut Canvas>) {
  let mut arguments = line.split_whitespace();
  match arguments.next() {
    Some("help") => help(console),
    Some("commands") => commands(console),
    Some("controls") => controls(console),
    Some("select") => match (arguments.next(), canvas) {
      (Some(arg), Some(canvas)) => select(console, canvas, arg),
      (None, _) => console.push_system(Err("Command 'select' takes 2 arguments, found 1")),
      (_, None) => console.push_system(Err("No map loaded"))
    },
    Some(arg) => console.push_system(Err(format!("Invalid command '{}'", arg))),
    None => console.push_system(Err("No command"))
  };
}

fn help(mut console: ConsoleHandle) {
  console.push_system(Ok("Use the 'commands' command for a list of all commands"));
  console.push_system(Ok("Use the 'controls' command for a list of all controls and hotkeys"));
}

fn commands(mut console: ConsoleHandle) {
  for &[name, description] in COMMANDS_LIST {
    console.push_system(Ok(format!("{:<24}{:>32}", name, description)));
  };
}

fn controls(mut console: ConsoleHandle) {
  for &[name, description] in CONTROLS_LIST {
    console.push_system(Ok(format!("{:<24}{:>32}", name, description)));
  };
}

fn select(mut console: ConsoleHandle, canvas: &mut Canvas, arg: &str) {
  match canvas.view_mode() {
    ViewMode::Color => console.push_system(Err("Use SPACEBAR to select a new color instead")),
    ViewMode::Kind => match arg.parse::<DefinitionKind>() {
      Ok(kind) => canvas.brush_mut().kind_brush = Some(kind.into()),
      Err(_) => console.push_system(Err("Invalid type, expected one of 'land', 'sea', or 'lake'"))
    },
    ViewMode::Terrain => match canvas.config().terrains.contains_key(arg) {
      true => canvas.brush_mut().terrain_brush = Some(arg.to_owned()),
      false => console.push_system(Err("Invalid terrain"))
    },
    ViewMode::Continent => match arg.parse::<u16>() {
      Ok(continent) => canvas.brush_mut().continent_brush = Some(continent),
      Err(_) => console.push_system(Err("Invalid continent, expected integer"))
    },
    ViewMode::Coastal => console.push_system(Err("Coastal map mode does not support painting"))
  };
}

const COMMANDS_LIST: &[[&str; 2]] = &[
  ["help", "Basic help"],
  ["commands", "Shows the commands list"],
  ["controls", "Shows the controls list"],
];

const CONTROLS_LIST: &[[&str; 2]] = &[
  ["1", "Color map mode"],
  ["2", "Terrain map mode"],
  ["3", "Type map mode"],
  ["4", "Continent map mode"],
  ["5", "Coastal map mode"],
  ["MOUSE1 / LMB", "Paint with selected brush"],
  ["MOUSE2 / RMB", "Pan camera"],
  ["MOUSE3 / MMB", "Pick brush from map"],
  ["SCROLL", "Zoom map view"],
  ["SHIFT + SCROLL", "Resize brush"],
  ["CTRL + Z", "Undo"],
  ["CTRL + Y", "Redo"],
  ["CTRL + SHIFT + S", "Save-As"],
  ["CTRL + SHIFT + ALT + S", "Save-As Archive"],
  ["CTRL + S", "Save"],
  ["CTRL + O", "Open"],
  ["CTRL + ALT + O", "Open Archive"],
  ["SPACEBAR", "Cycle brush options"],
  ["SHIFT + C", "Re-calculate coastal provinces"],
  ["SHIFT + R", "Re-color all provinces"],
  ["SHIFT + P", "Display map errors/warnings"],
  ["H", "Reset camera view"],
  ["~", "Toggle console"]
];
