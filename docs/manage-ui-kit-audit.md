# Manage popup audit

Source inventory for `ui/shared-ui-kit`, before the Manage port. No native application
was launched. Control recipes below are source-derived; screenshots need owner verification.
Send each start as `{"id":1,"method":"ui.start","params":{"command":"COMMAND"}}`
with the launch token on the connection's first request. Engine setup calls use
`{"method":"engine.execute","params":{"command":"COMMAND","params":{...}}}`.

| Manage entry | Before: source and surface | Existing fields, actions and nested surfaces | Recipe/setup |
| --- | --- | --- | --- |
| `parameters.change` | `params_dialog.rs`, custom PARAMETERS window, not kit | Filter; Import; Export CSV/JSON; Favorites, User Parameters, grouped Model Parameters. Columns: favorite/order, name (double-click rename), unit, expression, evaluated value/error, comment, delete. Add name/unit/expression/comment. Unit dropdowns; expression completion. Native import/save sheets; validation errors inline. | Start directly; optionally `parameters.add {"name":"width","expression":"10 mm"}` first. |
| `parameters.graph` | `lib.rs::start` falls through to engine, **no popup** | Returns dependency graph JSON. | `ui.start` opens nothing; `engine.execute parameters.graph {}` returns the graph JSON. |
| `parameters.move` | Same, **no popup** | Missing arguments report an error. Ordering UI is the Parameters window's arrows. | Open `parameters.change`; click `param:before:NAME` or `param:after:NAME`. |
| `parameters.import`, `parameters.export` | Same, **no popup** from ribbon | Actual UI is Parameters window buttons and host-native file picker (`file_picker.rs`). No import/export option panel or additional egui confirmation exists. | Open `parameters.change`; Import… / Export CSV… / Export JSON…. |
| `config.table`, `config.configure` | `dialogs_motion.rs`, legacy CONFIGURATIONS Dialog | Horizontal table: Configuration, dynamic parameter/suppression/material/appearance columns, remove column, active/Activate/Apply, inline cell fields, Delete row. New configuration, Add Row, Add Column menu with Face Appearance → Faces on BODY submenus; per-cell Library menu. Structured rule editor and preview below table. | Start either; optionally `parameters.add {"name":"width","expression":"10 mm"}`, then `config.configure {"params":["width"]}`. |
| `config.rules`, `config.preview` | `lib.rs::start` falls through, **no dedicated popup** | Rules returns data; preview requires a row. Existing UI lives below configuration table: ordered enabled rules, Edit/up/down/Delete, Add or edit rule disclosure (name, units, recursive condition, effect, Save Rule), row dropdown/Preview Row and report disclosure. | Open `config.table`; Add or edit rule / Preview Row. |
| `material.assign` | `dialogs.rs`, legacy PHYSICAL MATERIAL Dialog | Bodies selection, Material dropdown, OK/Cancel; includes document-created materials. | `solid.box {"length":10,"width":10,"height":10}`, then start; select Body1 in viewport/browser. |
| `appearance.assign` | `dialogs_appearance.rs`, legacy APPEARANCE Dialog | Objects selection; Apply To bodies/faces/component; Colour popup; Opacity; PBR toggle and metallic/roughness/IOR; Apply to selection/Remove override; local library path/open/load/search/select/apply/export and overwrite checkbox; In This Design swatches; Library category/search/swatches; drag/drop. Inline status. | Same box setup, then start. |
| `material.properties.validate` | `material_card_review.rs`, independent legacy review window | JSON multiline input, Review, validation error, card properties/conditions/sources/derived moduli disclosures and read-only report, Close. | Start directly, paste a supported candidate card. |
| `library.inspect`, `library.export`, `library.apply` | `lib.rs::start` falls through, **no dedicated popup** | Require arguments. Existing local-library UI is inside Appearance (`dialogs_library.rs`); native Open/Save sheets; overwrite is explicit checkbox, errors inline. | Start `appearance.assign`; use library:path/load/search/entry:N/apply/export handles. |
| `sheet.manage_rules` | `dialogs_sheet.rs`, legacy MANAGE RULES Dialog | Rule dropdown including New rule; conditional name; thickness, K factor, bend radius, relief width/depth, corner relief shape/size, hem gap/gap; Active rule; Delete rule; OK/Cancel. No separate deletion confirmation. | Start directly. |
| `plastic.manage_rules` | `dialogs_plastic.rs`, legacy MANAGE PLASTIC RULES Dialog | Rule dropdown including New rule; conditional name; material and rule expression fields; Active rule; OK/Cancel. | Start directly. |
| `plastic.assign_rule` | `dialogs_plastic.rs`, **already kit** | Bodies selection, Rule dropdown, OK/Cancel. | Box setup above, then start. |
| `document.units` | `lib.rs::start` falls through, **no popup** | Missing units argument reports an error. Existing unit choices are browser context menu commands. | `engine.execute` with `{"units":"in"}` changes units directly. |
| `timeline.compute_all`, `edit.checkpoint` | `lib.rs::start`, **immediate commands** | Recompute / create checkpoint, no dialog. | Start directly. |
| `edit.restore_checkpoint` | `lib.rs::start` falls through, **no popup** | Requires checkpoint id; no checkpoint picker exists. | `edit.checkpoint {}` returns id; `edit.restore_checkpoint {"checkpoint":ID}` restores directly. |

Native Open/Save boundaries need separate acceptance decisions if “all
popups on the library” includes OS-provided windows. The existing file-picking continuation
and platform behavior must remain intact; replacing those is not a paint-only port.

## Other surfaces still outside the kit (source inventory)

This list includes windows, floating panels and menus; it does not claim runtime screenshot verification.

| Surface | Source |
| --- | --- |
| Home page (full-page start surface: painted tiles, thumbnail cards, links; no kit element fits) | `home.rs` |
| Assistant transcript/composer layout, image gallery and docking chrome (specialised conversation/media surfaces; matching controls use the kit) | `agent_chat/panel.rs` |
| Host-native file/program pickers (no egui surface; the load/progress card is ported) | `file_picker.rs`; host services in `apps/solidcraft/src/main.rs` and `apps/solidcraft-web/src/main.rs` |
| Help action menu and shortcut command-result list/pin painting (menus/search results, not property forms; the search field uses the kit) | `help.rs`, `shortcut_box.rs` |
| Ribbon overflow/recent/family menus and context menus | `toolbar.rs`, `context_menu.rs`, `timeline_left.rs` |
| Motion transport controls, key strips and analysis chart; generated part thumbnails | `dialogs_motion.rs`, `dialogs_motion_plot.rs`, `dialogs_parts.rs` (matching form controls are ported) |
| Joint-browser/timeline rows and glyphs; flat-pattern canvas | `dialogs_assembly.rs`, `dialogs_sheet.rs` (dialogs and export actions are ported) |
| Slicer program icons and painted popup entries; native program picking | `dialogs_save_mesh.rs` (format, checkbox and dropdown shell are ported) |
| Canvas/decal insertion and dimension placement/geometry painting | `dim_view.rs`, `sketch_dims.rs`, `canvas.rs` (dimension text editors are ported) |
| Select Other popup and viewport display controls | `select_other.rs`, `viewport.rs` |
| Tab-strip painting and popup placement; render canvas | `documents.rs`, `render_workspace.rs` (overflow choices and matching settings controls are ported) |

## Port implementation and boundaries

The port replaces the renderers listed above; it does not add a new interaction for
ribbon entries that currently execute immediately. No command id or engine implementation
is changed. Existing import/export callbacks, overwrite handling, local-library identity
checks, parameter validation/undo/order, appearance selection/drag payloads, opacity/PBR
precision, configuration rule ordering and non-mutating preview are retained.

Kit additions are documented in `docs/ui-kit.md`: table columns/rows/cells and empty state,
wide bounded Panel and optional manager folding, stable cell/search editors, multiline data input, read-only reports,
selectable lists, colour/scalar controls, swatches, dense nested action menus and suggestion
popup rendering. Existing expression-completion keyboard routing stays in its original
module; drawing now goes through the kit. Native file sheets are unchanged. The colour
chooser is owned by the kit's `color_field`, which still delegates its internal HSV/RGB
popover to egui. No tabs/progress row was added because these Manage surfaces do not use one.

User-visible layout changes: standard kit header/hint/footer; fixed 24 pt table rows;
horizontal overflow for wide tables; the Add Parameter form uses labelled rows below the
user table; model groups use kit disclosures; review closes via standard OK/Cancel or ×.
Closing Parameters/Configurations still leaves edits already applied by their per-row
commands, as before. New catalog entries exist in every locale; new helper text currently
uses English fallback values in non-English catalogs and needs translation review.

Family 4 removes the remaining raw-widget allowances for legacy window chrome/selection controls,
Drive Joint and flat-pattern exports. Appearance RGB data and library swatch
colours retain the pre-existing colour-data exemptions; no new allowance is added.

## Ready-to-send starts for the ported cards

A small common setup (run once in a fresh design) gives assignment targets and one
configuration column. Repeat the `select.set` line before each assignment card; opening a selection card consumes the preselection.
Close each card through `dialog:close` (Parameters also retains `params:close`) before
opening the next one. Library actions are inside Appearance; configuration rules/preview
are inside Config Table. The host must authenticate the first request as described above.

```json
{"id":10,"method":"engine.execute","params":{"command":"solid.box","params":{"length":10,"width":10,"height":10}}}
{"id":11,"method":"engine.execute","params":{"command":"parameters.add","params":{"name":"width","expression":"10 mm"}}}
{"id":12,"method":"engine.execute","params":{"command":"config.configure","params":{"params":["width"]}}}
{"id":13,"method":"engine.execute","params":{"command":"select.set","params":{"items":[{"type":"body","name":"Body1"}]}}}
{"id":20,"method":"ui.start","params":{"command":"parameters.change"}}
{"id":21,"method":"ui.start","params":{"command":"config.table"}}
{"id":22,"method":"ui.start","params":{"command":"config.configure"}}
{"id":23,"method":"ui.start","params":{"command":"material.assign"}}
{"id":24,"method":"ui.start","params":{"command":"appearance.assign"}}
{"id":25,"method":"ui.start","params":{"command":"material.properties.validate"}}
{"id":26,"method":"ui.start","params":{"command":"sheet.manage_rules"}}
{"id":27,"method":"ui.start","params":{"command":"plastic.manage_rules"}}
{"id":28,"method":"ui.start","params":{"command":"plastic.assign_rule"}}
```

Useful nested handles: `dialog:Unit:cm` after `param:new:unit`; `dialog:section:Add or edit rule`;
`dialog:When:Parameter` / `dialog:Then:Range` after their dropdowns; `dialog:section:Local library`;
`library:path`, `library:load`, `library:search`, `library:entry:0`, `library:apply`, `library:export`;
`appearance:pbr`, `appearance:metallic`, `appearance:roughness`, `appearance:ior`; `review:input`.
Use `ui.at {"handle":"HANDLE"}` to resolve these into coordinates for `ui.click`/`ui.text`.

Manager cards retain their fold/unfold behavior through the kit header (`dialog:collapse`), preserving drafts and picked inputs. Rule actions also publish `config:save_rule`, `config:preview_row`, `config:rule:ID:up` and `config:preview:param:NAME` for scrolling and automation.

Wide manager cards reserve the active Sketch Palette’s space when both panels fit; tables scroll horizontally within that width.

## Family 3: settings, help and customisation

Source inventory before porting:

| Surface | Inventory and disposition | Open it |
| --- | --- | --- |
| AI settings | Master/toolbar toggles; installed/added agent cards, Test/Default/Remove, global/per-agent permissions, remembered grants, side and turn limit. Ported to kit rows, disclosures, choices, notes and actions. | Preferences → AI (`ui.prefs {"open":"AI"}`); enable AI features. |
| Add agent | Installed/custom/API choices, discovery/preset dropdowns, name/command/URL/model/key, connection test/error, Add and close. Ported to Prompt; secret remains masked and disabled without a keychain. | AI settings → Add agent, or Assistant agent selector → Add agent. |
| Assistant controls | Agent/mode/model selectors, connection/authentication, new-conversation confirmation, approval actions, attachments and queue actions use matching kit elements. Transcript, growing bottom composer container, docking/drag chrome and image gallery keep their specialised layouts. No generic property card replaces them. | Enable AI, then View → Assistant or Cmd/Ctrl+Shift+A (`ui.agent_chat {"features":true,"open":true}`). |
| About | Build identity, diagnostic copy, attribution/font/licence disclosures and virtualized third-party text. Ported to close-only Prompt. Help's action menu is a menu, not a form card. | Help (?) → About SolidCraft (`ui.help {"item":"about"}`). |
| Shortcut search | Search editor ported, retaining its focus ID and one-time focus request. Result ranking, JSON execution, keyboard selection, pins, row context menus and painted command list remain their specialised search layout. | S, or the application-bar search button. |
| Keyboard Shortcuts | Search, reset/import/export, conflict actions, command/key table, capture/clear/reset. Ported to kit table/fields/actions. Read-only Help entry remains read-only. | Preferences → Keyboard Shortcuts, File/Help → Keyboard Shortcuts (`ui.prefs {"open":"Keyboard Shortcuts"}`). |
| Toolbar customisation | Tab guidance, panel command checkboxes and Reset Toolbar. Ported to close-only Prompt. Drag/reorder menus remain in their existing toolbar surfaces. | Right-click toolbar → Customize Toolbar… (menu item `ui.toolbarCustomize`; this is a UI menu action, not an engine command). |
| Preferences | All eight page selectors and form controls use the kit. Theme/font discovery and native import/export callbacks, Cancel/Apply snapshots, scale drag deferral, unsupported MSAA choices and command-backed auto-projection are retained. Colour-vision sample is a painted preview, not a form control. Update/cursor preference controls use kit drawing through their existing helpers. | File → Preferences, gear, Cmd/Ctrl+, (`ui.prefs {"open":"General"}`). |

No guard allowance was added. Deleted the legacy settings section/note painters, About window/licence renderer, toolbar window and keymap grid, and Preferences window/grid/footers. Kit additions are documented in `ui-kit.md` and exercised in the light/dark gallery. New catalogue messages have English placeholders in other locales.

## Family 4: remaining dialogs and matching inline controls

Inventory and disposition, in port order:

| Surface | Result and retained boundary | Open it |
| --- | --- | --- |
| Contact sets | Kit Panel, name, enable checkbox, set actions and collision report. Commands and preselection remain unchanged. | `ui.start {"command":"contact.create"}`; create components with bodies to populate sets. |
| Motion Study / analysis | Kit wide Panel, study/name/steps, loop/speed, step slider, joint/key controls, plot/quantity/duration and collision actions. Transport icons, key strips and the interactive chart retain their specialised painting. Preview moves, playback, collision navigation, GIF export and restoration remain owned by the existing code. | `ui.start {"command":"motion.study"}`; create a movable joint first. To edit: include `"params":{"study":"NAME"}`. Plot and collision controls are inside this card. |
| Exploded View | Kit name and continuous scale slider; preview and cancel restoration retained. | `ui.start {"command":"explode.create"}` with assembly components. |
| Standard parts / Insert Part | Kit Panel, family/size/length/name and table-value controls. The generated part thumbnail remains an image, with its existing cache. | `ui.start {"command":"parts.insert"}` or `parts.fastener`; optionally preselect a hole wall/rim. |
| Legacy dispatcher | Deleted the independent window, grid, header, footer, width measurements and raw field/combo/selection fallbacks. Every Dialog kind now composes the kit. Save/Send and Export DXF labels and aliases are retained. | All command dialogs. |
| Drive Joint / Interference | Kit Panel, joint selector, stable focus expression editor, continuous slider, range/stopped text and compute actions. Browser/timeline joint rows, glyphs and context-menu containers retain their existing specialised rendering. | `ui.start {"command":"joint.drive"}` after creating a movable joint; `inspect.interference` for analysis. Browser → Joints → double-click edits a joint; its menu offers Drive Joint. |
| Save as mesh | Kit Panel, format, Send checkbox and slicer dropdown shell. Installed-program icons and popup rows remain painter-based; Delete/custom-program picking and host-native file continuations are retained. | Preselect bodies, then `ui.start {"command":"file.save_mesh"}`. Slicer controls require native program-picker services. |
| Current View as DXF | Kit choices, tolerance and projection/help text; remembered options and asynchronous native save retained. | `ui.start {"command":"file.export_view_dxf"}` with visible bodies. |
| Flat Pattern controls | Kit Panel, dimensions/bends and DXF/bend-table export actions. The flat-pattern canvas and navigation remain specialised. | `ui.start {"command":"sheet.flat_pattern"}` after making sheet metal. |
| Dimension editor | Kit stable-ID editor, heading and validation report. Reference insertion, selection ranges, completion and draft commit/cancel remain in their owner. | Sketch → Dimension; place a dimension, or double-click an existing driving dimension. |
| Placement fields | Kit placement editor retains drawing-tool Enter/Tab/completion ownership. Floating field positioning and canvas geometry remain specialised. | Start a sketch shape (line, circle, rectangle, etc.); type into its live dimension fields. |
| Select Other | Skipped: the entire popup is painter-based, including entity icons, hover highlighting and hit targets; there is no separate form control to replace cleanly. | Hold left-click over overlapping geometry, viewport menu → Select Other, or `ui.selectOther {"x":X,"y":Y}`. |
| Document-tab overflow | Kit selectable entries; tab-strip painting and popup placement/outside-click handling retained. | Open enough designs to overflow the tab strip, then click `tab:overflow`. |
| Render setup selector | Kit dropdown and capped name editor; the remaining matching buttons, checkboxes, dimensions, colour and progress controls also use the kit so the file needs no raw-control allowance. Render canvas and navigation remain specialised. | Enter the Render workspace (`ui.workspace {"workspace":"render"}`); setup selector is at the top of its settings panel. |

The raw-control allowance table is empty (previously 10 in `dialogs.rs`, 6 in
`dialogs_assembly.rs`, 2 in `dialogs_sheet.rs`). Native file sheets, context menus,
canvases, browser rows and painter-based controls above are retained. Kit additions
are listed under “Inline placement and motion controls” in `ui-kit.md` and in the
light/dark gallery. No command IDs or engine behavior are changed.
