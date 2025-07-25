# HOI4 Province Editor
[Latest Release](https://github.com/ScottyThePilot/hoi4_province_editor/releases/latest)

This program is designed to simplify or replace needing to manually edit `provinces.bmp` and `definition.csv` when
editing HOI4 Maps. The idea behind this program is that it unifies editing both files in one place with a graphical
editor, as well as attempting to guarantee that all province maps created by this program will load correctly into
the game.

This program is not a complete replacement for MapGen, it is intended to be used to edit a map you have already
generated with MapGen, or for making tweaks to an already complete map.

Please make backups of your maps before using HOI4PE, and regularly while using HOI4PE.

![Province Map Mode](images/hoi4pe_color.png)
![Terrain Map Mode](images/hoi4pe_terrain.png)

To load a map, you can do one of the following:
- Drag a folder and it will look for a `provinces.bmp` and `definition.csv` inside that folder
- Drag a file and if its name is `provinces.bmp` or `definition.csv`, it will look in the same folder for the other file
- Drag a ZIP archive, and it will try to load `provinces.bmp` and `definition.csv` from the archive
- Use `Ctrl-O` or `Ctrl-Alt-O` to load a folder or archive using the file browser

By default, HOI4PE will scramble all of the province IDs in your `definition.csv`. If you are editing a pre-existing
map, this will probably mess up states, strategic regions, etc. In order to mitigate this, you can set the
`preserve-ids` key to `true` in `hoi4pe_config.toml`; this will attempt to keep the ID scrambling to a minimum, and if
IDs do change, they will be logged to `id_changes.txt`.

In the terrain/biome map mode, the colors are based on what MapGen/ProvGen takes as input for terrain maps.
In the coastal map mode, darker colors represent provinces that are not coastal, while lighter colors are coastal.

When painting continent IDs, you cannot paint continent 0 on land, and sea can only have continent 0.

## Controls
- `1` Color/province map view mode
- `2` Terrain/biome map view mode
- `3` Land type map view mode
- `4` Continents map view mode
- `5` Coastal provinces map view mode
- `6` Adjacencies map view mode
- `Left-click` will draw with a color or map data while a color or some data is selected
- `Right-click` will grab and pan the camera around
- `Middle-click` will pick whatever color or map data that you are pointing at
- `Scroll` will zoom the map view
- `Shift-Scroll` will resize your brush when in color mode
- `Ctrl-Z` and `Ctrl-Y` are Undo and Redo, respectively
- `Ctrl-Shift-S` will Save-As, adding `Alt` will allow you to save as an archive
- `Ctrl-S` will Save, overwriting whatever map files you had imported
- `Ctrl-O` will let you open a `map` folder, adding `Alt` will allow you to select archives
- `Spacebar` will give you a new color/type/terrain/continent to paint with depending on map mode
- `Shift-C` will re-calculate coastal provinces
- `Shift-R` will randomly re-color all of the provinces on the map
- `Shift-P` will calculate and display symbols indicating map errors/warnings
- `A` switches to the area/brush tool
- `B` switches to the bucket/fill tool
- `L` switches to the lasso tool
- `H` resets the camera view
- `Tab` show all recent informative alert messages
- `Escape` to cancel a lasso
- `Enter` to complete a lasso

Adjacencies may be created by dragging from one province to another with `Left-click`.
Note that HOI4 requires `sea` and `land` adjacencies to have a "through province" which you will have to specify manually.

## Features
- Map viewing, editing, manupulation, importing and exporting
- Flood-fill and polygonal lasso tools
- Support for custom terrain types via `hoi4pe_config.toml`
- Seeing map errors/warnings graphically (via `Shift-P`)
- Auto-generating which provinces are coastal (via `Shift-C`)
- Exporting terrain or land type view modes for MapGen/ProvGen
- Preserving province IDs (in order to not break maps)
- Viewing province ID numbers on the map
- Support for creating/editing adjacencies

## Manually Building
1. [Install Rust](https://www.rust-lang.org/tools/install)
2. Clone this repository to a folder and navigate there in your terminal
3. Run `cargo build --release` in that folder, wait for it to complete
4. The resulting executable should be located in `/target/release`
