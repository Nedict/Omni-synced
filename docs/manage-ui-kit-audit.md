# UI kit coverage boundaries

Feature dialogs, manager cards, settings/help/customisation windows, confirmations
and matching inline controls now compose `ui_kit`. The raw-control allowance table
is empty. The cleanup removes unused frame chrome, row-publication state, appearance
wrappers and the plastic editor fallback. Shared controls and their options are
listed in [the kit reference](ui-kit.md).

These specialised surfaces remain in their owners:

| Surface / source | Why it remains outside the kit |
| --- | --- |
| Start page: `home.rs` | Painted start tiles, thumbnails and links rather than form cards |
| Assistant transcript/composer containers, gallery and docking: `agent_chat/panel.rs` | Conversation/media layout, rich selectable content, drag/docking behavior; matching controls use the kit |
| Native file/program pickers: `file_picker.rs`, host services | Host-native sheets have no egui control surface; load/progress cards use the kit |
| Help menu and command-result ranking/list/pins: `help.rs`, `shortcut_box.rs` | Menu/search layout; search editor uses the kit |
| Ribbon, browser/timeline rows, context menus: `toolbar.rs`, `browser.rs`, `context_menu.rs`, `timeline_left.rs` | Specialised navigation, painting, menus and inline rename interactions |
| Motion transport, key strips and chart: `dialogs_motion.rs`, `dialogs_motion_plot.rs` | Playback/chart interaction; matching form controls use the kit |
| Generated part thumbnails: `dialogs_parts.rs` | Cached images rather than controls |
| Joint-browser glyphs and flat-pattern canvas: `dialogs_assembly.rs`, `dialogs_sheet.rs` | Model navigation and geometry; dialog/export actions use the kit |
| Slicer program popup entries: `dialogs_save_mesh.rs` | Painted installed-program icons/entries and native program picking; dropdown shell uses the kit |
| Dimension/placement geometry: `dim_view.rs`, `sketch_dims.rs`, `canvas.rs` | Canvas positioning, completion ownership and geometry painting; matching dimension/placement editors use the kit |
| Select Other and viewport display controls: `select_other.rs`, `viewport.rs` | Painter-based entity icons, hit testing and hover highlighting |
| Document tabs and render canvas: `documents.rs`, `render_workspace.rs` | Tab-strip/canvas painting and popup positioning; overflow choices/settings controls use the kit |
| Native colour chooser inside `color_field` | Kit owns the field; egui owns its HSV/RGB popup internals |

Immediate commands such as `parameters.graph`, `parameters.move`, `config.rules`,
`config.preview` and `library.*` keep their existing JSON execution behavior. Their
related controls live in Parameters, Configurations and Appearance; the kit creates
no extra command dialogs. Command IDs, model operations and scenario handles are
unchanged by cleanup.
