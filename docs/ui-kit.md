# SolidCraft UI kit

`crates/ui-egui/src/ui_kit/` is the tokenised library used to assemble dense feature dialogs, preferences, property/prompt cards and the sketch palette. This extraction preserves their existing layout, painting, focus, accessibility labels and scenario identities. Sketch tool panels use the same composition and elements.

## Module tree

```text
ui_kit/
  mod.rs                 overview and the one caller import path
  metrics.rs             row/header dimensions, sanitising and label-column maths
  table.rs               measured columns, headers, fixed-height selectable rows and cells
  text.rs                weighted, clipped, pixel-snapped text
  layout.rs              form scope, aligned rows, captions, hints, notes, sections, dividers, picked lists, text_block, inline_text
  panel.rs               bounded windows, surfaces, feature/palette composition and compute status
  fields.rs              expressions, plain text, cell/search editors, dropdowns, selections, paired values, colour/scalar/slider
  choice.rs              text/icon segments, checkboxes, wrapped modes, icon toggles, popup_list, swatches and list_picker
  actions.rs             buttons, equal secondary pairs, keyboard and OK/Apply/Cancel footers
  status.rs              supplied name/status/icon, constraint meter and progress bar
  gallery.rs             test-only light/dark element examples and geometry/interaction checks
  *_tests.rs             moved concern tests, Panel regressions and shared test input helpers
```

## Rules

- Use one caller import path: `crate::ui_kit::{...}`. Implementation modules are organised by concern; panels do not import their internals.
- Colours, fonts, dimensions and spacing come from `theme::Tokens`. No local font sizes, families, colours or design metrics.
- Rows use `control_height` (24 pt today). A shared label column starts at `form_label_width`, widens to at most 1.6 times that value and never exceeds half the row. Label measurements settle across frames.
- Build cards with `Panel`. Right-hand feature cards keep `space_8` clearance from the viewport edge. It owns the bounded window, feature header/drag target, scrolling body padding, pinned actions, compute status and keyboard routing. Its palette composition retains the existing palette padding and whole-card scrolling on short viewports. Supply `FeatureOptions::apply_label` for a tool card that also needs Apply. Use `feature_frame` or `palette_frame` for the existing docked/floating surfaces.
- Panels use kit elements instead of raw egui controls. Application logic supplies values, command metadata and enabled state; it receives responses/actions and runs the existing commands.
- Arguments put `ui` first, then the label/key (when not inherited from `row`), then the value and options. Buttons/targets return `Response`; choices report clicks; expression editors report accepted Enter (`text_field` additionally returns changed). Containers return their closure result. Existing Enter semantics intentionally distinguish completion from submission.
- Scenario names and accessibility labels are API: preserve them. New elements publish geometry without overwriting the native accessible label or checked state. `Prompt` publishes `dialog:window`; the footer also publishes `dialog:footer:actions` for its padded action bounds. Callers may publish stable surface aliases for those rectangles.
- `footer(..., None)` retains the original OK/Cancel layout. `Some("Apply")` adds the third action and returns `Action::Apply`; the panel decides whether to keep the operation open.

## Elements

All calls below use `crate::ui_kit::`; snippets assume local values and normal enclosing form scope. `control_*` means `control_fill`, `control_border` and/or `control_radius` as used by that element.

| Module / element | Purpose | Tokens | Example |
| --- | --- | --- | --- |
| `metrics::row_height` | Row height for controls | `control_height` | `row_height()` |
| `metrics::header_height` | Feature heading height | `form_header_height` | `header_height()` |
| `layout::scope` | Inherited form styling and label-column identity | `space_4/8, form_gap_y, control_height, font_label, accent, selected_bg, dialog_rule, control_fill/border/radius, text` | `scope(ui, \|ui\| { /* elements */ })` |
| `layout::row` | Aligned label/control row, 24 pt today | `control_height, form_label_width, font_label, dialog_label, space_8` | `row(ui, "Radius", \|ui\| field(ui, &mut radius))` |
| `layout::caption` | Dim helper beneath a row control | `form_icon_size, font_small, text_dim, space_8; label-column tokens` | `caption(ui, "Line Type", "Construction")` |
| `layout::section` | Persisted disclosure with a rule and uppercase title | `control_height, space_2, radius_sm, hover, dialog_rule, form_section_icon_size, form_gap_y, font_caption, font_section_tracking, text_dim` | `section(ui, "advanced", "Advanced", false, \|ui\| {})` |
| `layout::hint` | Helper text below a card header | `control_height, font_small, text_dim` | `hint(ui, "pick", "Pick a point")` |
| `layout::note` | Wrapping explanatory text in a card body, for rules or caveats too long for a hint | `font_small, text_dim, space_4` | `note(ui, "pattern", "Counts include the original.")` |
| `layout::list_row` | Picked-item row with a remove action | `control_height, space_8, font_label, text; icon-button tokens` | `list_row(ui, "point-1", "Point 1").clicked()` |
| `layout::divider` | Full-width rule without extra padding | `dialog_rule` | `divider(ui, "group")` |
| `collapsible_header`, `Panel::feature_collapsible` | Optional manager fold action retaining inputs, the common header/footer and legacy chevron labels | header tokens, `space_4` | `collapsible_header(ui, icon, title, &mut collapsed, close)` |
| `panel::header` | Feature title/icon/close band | `form_header_height, form_icon_size, form_close_size, form_inset, font_label, text, dialog_rule, space_8/10; icon_button_at tokens` | `header(ui, "fillet", "Fillet", || {})` |
| `fields::field` | Expression editor; the value carries its unit text ("20 mm", "45 deg"), there is no unit suffix; returns accepted Enter | `control_height, control_fill, form_inset, font_label, text_dim, space_4; scope tokens` | `field(ui, &mut expression)` |
| `fields::focused_field` | Single-value expression popup retaining keyboard focus | `field tokens` | `focused_field(ui, &mut expression)` |
| `fields::hint_field` | Expression editor with empty-value hint | `same as field` | `hint_field(ui, &mut depth, "To the next face")` |
| `dropdown_menu` | Dropdown with caller-supplied kit choices/actions, disabled entries and stable aliases; use `dropdown` for indexed lists | dropdown tokens | `dropdown_menu(ui, "msaa", current, \|ui\| { /* enabled list_picker choices */ })` |
| `password_field` | Stable-ID secret input; never publishes the secret in scenario/accessibility metadata | `control_height/fill`, `form_inset`, `font_label` | `password_field(ui, id, &mut key, hint)` |
| `number_field` | Integer/decimal field retaining the caller's range, drag step and unit suffix | scope control/font tokens | `number_field(ui, &mut minutes, 0.0..=120.0, 0.5, " min")` |
| `formatted_slider` | Stepped preference slider retaining percent/multiplier formatting and numeric accessibility value | `control_height`, `space_24/4`, scope control/font tokens | `formatted_slider(ui, &mut scale, 0.5..=2.5, 0.05, \|v\| format!("{:.0} %", v * 100.0))` |
| `message_field` | Growing plain-text message editor; parent retains scroll container and Enter routing | `control_fill`, `form_inset`, `font_label`, `space_4` | `message_field(ui, id, &mut message, hint)` |
| `text_lines` | Virtualized read-only licence/report lines with horizontal overflow; only visible lines are laid out | `font_small`, `text`, `control_height` | `text_lines(ui, "licences", report)` |
| `fields::text_field` | Plain text editor without expression completion; returns (Enter, changed) | `same as field` | `text_field(ui, &mut name, "Name")` |
| `fields::multiline_text_field` | Three-line plain-text row with stable focus ID; caller routes Enter | `control_height, control_fill, form_inset, font_label; row tokens` | `multiline_text_field(ui, "Text", id, &mut text)` |
| `fields::searchable_dropdown`, `SearchOption` | Search/list dropdown with optional custom-value action and loading message | `control_height, space_24, font_label; scope control tokens` | `searchable_dropdown(ui, "font", current, &mut filter, &choices, custom, pending)` |
| `fields::value_pair_named` | Paired X/Y captions retaining independent accessible/scenario names | `value_pair tokens` | `value_pair_named(ui, "Position", [&mut x, &mut y], ["X", "Y"], ["Distance X", "Distance Y"])` |
| `choice::text_emphasis` | Compact bold/italic toggles with localized accessible names | `scope font/control tokens` | `text_emphasis(ui, &mut bold, &mut italic)` |
| `fields::dropdown` | Native dropdown with kit arrow and scenario option handles | `text_dim; scope control/font tokens` | `dropdown(ui, "operation", &mut operation, &["Join", "Cut"])` |
| `fields::selection_field` | Filled point/curve/edge selection target | `control_height, control_radius, selected_bg/ink, accent, form_inset, space_24/8, font_label, text_dim` | `selection_field(ui, "Point 1", active)` |
| `fields::selection_field_state` | Selection target with explicit filled state | `same as selection_field` | `selection_field_state(ui, "Select", active, false)` |
| `fields::value_pair` | Two labelled numeric fields in one row; returns accepted Enter | `row/field tokens, space_8, dialog_label` | `value_pair(ui, "Position", [&mut x, &mut y], ["X", "Y"])` |
| `fields::fraction_field` | Locale-aware variable-radius position editor | `scope control/font tokens` | `fraction_field(ui, &mut position)` |
| `choice::segmented` | Equal text choices, falling back to dropdown when labels cannot fit | `control_height, control_fill/border/radius, font_label, selected_bg/ink, hover, text, space_8` | `segmented(ui, &mut mode, &["One", "Two"])` |
| `choice::segmented_compact` | Always-visible short choices; long translations clip and retain tooltips and per-choice handles | `segmented tokens, space_4` | `segmented_compact(ui, &mut alignment, &labels)` |
| `choice::segmented_states` | Segmented choices with mixed per-choice state | `same as segmented` | `segmented_states(ui, &mut mode, &["One", "Two"], &states)` |
| `choice::segmented_icons` | Equal icon choices with mixed state | `segmented tokens, space_4/12, icon/fill, accent; icon palette` | `segmented_icons(ui, &mut mode, &icons, &states)` |
| `choice::checkbox_grid` | Compact checkbox options in bounded columns | `form_checkbox_height, control_fill/border, radius_sm, accent, on_accent, font_body, text, space_2/3/6/8/14` | `checkbox_grid(ui, &mut [("Flip", &mut flip)], 2)` |
| `choice::options` | Expanded two-column Options disclosure | `section and checkbox_grid tokens` | `options(ui, "options", &mut [("Flip", &mut flip)])` |
| `choice::checkbox_row` | Full-row target with label left and box right; returns clicked | `control_height, control_radius, hover, radius_sm, accent, control_fill/border, on_accent, font_label, text, space_2/3/6/14` | `checkbox_row(ui, "snap", "Snap", snap)` |
| `choice::checkbox` | Inline native checkbox retaining existing accessibility and sizing | `scope control/font/accent tokens` | `checkbox(ui, "Flip", &mut flip)` |
| `choice::tool_choices` | Wrapped command-variant icons, supplied with enabled state | `sketch_mode_size/radius/border/tint, sketch_accent, control_fill/border, icon/fill, space_4/6` | `tool_choices(ui, "Line", current, &choices)` |
| `actions::icon_button_at` | Icon action inside an existing rectangle | `form_icon_inset, text, text_dim; icon palette` | `icon_button_at(ui, rect, "close", "Close")` |
| `actions::keyboard` | Esc cancels; unowned Enter submits only when valid | `none` | `keyboard(ui.ctx(), valid)` |
| `actions::footer` | Pinned OK / optional Apply / Cancel; returns Action | `form_footer_height, form_button_height, dialog_footer/rule, radius_lg, control_*, accent, on_accent, font_label, space_6/10/12/16` | `footer(ui, "OK", "Cancel", valid, Some("Apply"))` |
| `actions::footer_with_status` | Footer with stable button IDs and compute text | `footer tokens, text_dim, space_4/8` | `footer_with_status(ui, "OK", "Cancel", valid, None, status)` |
| `actions::button_pair` | Equal-width secondary actions with optional icons and selected state | `form_button_height, control_*, selected_bg, hover, font_label, text, icon/fill, accent, sketch_footer_icon_size, space_4/6/10` | `button_pair(ui, "card", states, buttons)` |
| `actions::secondary_button` | Ordinary secondary action preserving native sizing | `scope font/control/spacing tokens` | `secondary_button(ui, "Add")` |
| `actions::primary_button` | Accent action outside the footer | `accent, on_accent; scope tokens` | `primary_button(ui, "Create")` |
| `actions::small_button` | Compact inline action preserving native sizing | `scope font/control tokens` | `small_button(ui, "×")` |
| `actions::icon_button` | Allocated control-height icon action | `control_height; icon_button_at tokens` | `icon_button(ui, "Remove", "close")` |
| `status::status_header` | Name, status dot/text and meter for a card | `font_label, text/dim, accent, dialog_rule, sketch_accent, form_icon_size, space_3/4/6/8/10` | `status_header(ui, "card", "Sketch 1", status)` |
| `table`, `Column`, `Table::row`, `TableRow::cell` | Horizontal table with measured column widths, header, 24 pt selectable body rows, inline controls/actions and empty state; compose an Add action after it | `form_label_width`, label-column maths, `control_height`, `font_label`, `dialog_label/rule`, `space_4/8`, `text/dim`, `selected_bg`, `hover` | `table(ui, "parameters", &columns, empty, \|ui, table\| table.row(ui, "width", false, \|row\| row.cell(\|ui\| field(ui, &mut value))))` |
| `inline_text` | Read-only text beside compact actions | `font_label, text` | `inline_text(ui, "Active")` |
| `label_actions` | Clipped row name beside compact actions | `control_height`, scope font/spacing tokens | `label_actions(ui, name, \|ui\| {})` |
| `PopupOption`, `popup_list` | Focus-neutral completion/suggestion popup; owner retains keyboard routing | `space_2, form_label_width, control_height, text_dim`, scope tokens | `popup_list(ctx, id, anchor, selected, &items, hint)` |
| `cell_text` | Read-only clipped table value with full tooltip/accessibility label | `control_height, font_label, text` | `cell_text(ui, name)` |
| `cell_editor` | Stable-ID inline editor; caller owns completion and commit/blur semantics | `control_height/fill, form_inset, font_label, text/error` | `cell_editor(ui, id, &mut value, "Expression", invalid)` |
| `search_field` | Stable-ID search input | `cell_editor` tokens | `search_field(ui, id, &mut query, "Filter")` |
| `text_area` | Bounded multiline JSON/data input, caller owns keyboard and validation | `control_height/fill, form_inset, font_label` | `text_area(ui, label, id, &mut json, 5, 262144)` |
| `text_block` | Wrapping read-only report or validation error; values include their units | `font_label, text/error` | `text_block(ui, "validation", report, invalid)` |
| `list_picker` | Selectable list row with native checked state and caller identity | `control_height`, scope control/font/selection tokens | `list_picker(ui, id, name, selected)` |
| `color_field` | Document RGB chooser under form styling (the internal native egui chooser remains) | scope colour/font tokens; RGB is document data | `color_field(ui, &mut rgb)` |
| `slider`, `scalar_field` | Continuous (display only, two decimals; the stored value is never rounded) / locale-aware bounded dimensionless values | `control_height`, scope control/font tokens | `slider(ui, &mut opacity, 0.0..=1.0)` |
| `color_swatch`, `swatch_grid` | Wrapped draggable document-colour samples retaining caller payload | `control_height/radius, border, accent, hover, space_4` | `color_swatch(ui, id, color, picked, tip, payload)` |
| `enabled_button`, `action_menu` | Enabled secondary action (`label`, then `enabled`) and nested dense action menu | scope control/font tokens | `enabled_button(ui, "Add", valid)`, `action_menu(ui, "Add Column", \|ui\| {})` |
| `panel::Prompt` | Floating card (1.5 card widths) for confirmations, offers, notices and short property forms: header, padded body, pinned footer with up to three actions (`actions(ok, cancel, apply)`; an empty `ok` leaves the primary one out), `anchor` / `placed` position, `selected` mark for the action Enter gives, `fields()` when Enter belongs to editable fields; returns (close pressed, `Action`) | `form_card_width` (x1.5, `cards(n)` for other widths), `dialog_body`, header/footer tokens, `space_6/8/10` | `Prompt::new(id, "delete", title).actions("Delete", "Cancel", None).show(ctx, true, \|ui\| text_block(ui, "why", text, false))` |
| `status::progress` | Determinate progress bar (a download, a file load), full width, `space_6` tall; fraction clamped; pair with a `text_block` naming the step | `space_3/6, dialog_rule, accent` | `progress(ui, "download", 0.4)` |
| `panel::Panel::actions` | Footer primary/closing labels of a docked card when they are not OK/Cancel ("Delete"/"Cancel"; `"", "Close"` for a card that only closes) | footer tokens | `Panel::new(..).actions("", "Close")` |
| `panel::Prompt::keep_open_on_escape` | Esc does not close the card (header close and the closing action still do): for a card holding a long text being edited (issue report) | footer tokens | `Prompt::new(..).fields().keep_open_on_escape()` |
| `actions::footer_selected` | Footer marking one action as the one Enter gives (used by `Prompt`) | footer tokens, `selected_bg`, `accent` | `footer_selected(ui, "Save", "Cancel", true, Some("Don't Save"), Some(Action::Apply))` |
| `fields::focused_text_field` | Plain text editor keeping keyboard focus, for a one-name card (Rename); returns accepted Enter | `field` tokens | `focused_text_field(ui, &mut name, "")` |
| `fields::length_field`, `fields::angle_field` | Draggable/typeable length held in millimetres (shown in the design's units) and angle in degrees | `control_height`, scope control/font tokens | `length_field(ui, &mut x, "in", 2)`, `angle_field(ui, &mut deg)` |
| `panel::Panel` | Docked/floating window and both card compositions | `form_card_width` (`wide()` uses three card widths), frame/header/body/footer tokens above | `Panel::new(title, id, anchor, viewport, side, frame).window()` |
| `choice::toggle_icon` | Independent pattern direction/centering icon | `control_height`, `space_3/4/24`, `icon/fill`, `accent`, scope tokens | `toggle_icon(ui, "Alignment", "Center", &mut on, "pattern_center")` |
| `choice::IconSegment`, `ToolChoice` | Supplied icon/command-choice metadata | Read by their drawing element | `IconSegment { key, icon, tip }` |
| `actions::Action`, `PairButton` | Footer result and equal-action metadata | Read by their drawing element | `Action::Apply` |
| `status::StatusHeader` | Supplied icon, status text, completeness and meter fraction | Read by `status_header` | `StatusHeader { icon, text, complete, fraction }` |

## Adding an element

1. Add its one implementation to the appropriate concern file. Document what it is, when to use it and the tokens it reads. Re-export it from `mod.rs`; keep panel callers on the single import path.
2. Add a light/dark example to `gallery.rs`, with stable scenario geometry/handles. Cover row height, label alignment and effective paint bounds, plus interaction if the element introduces an action. The moved regression tests live beside each concern in `*_tests.rs`.
3. Add an element-table row here. Reuse existing artwork; any new file asset needs an exact `ATTRIBUTION.md` row and the asset authoring workflow.
4. Run the gallery and widget/font/colour guards. The font metric guard scans the entire kit; the colour guard scans subdirectories too. The panel guard discovers sources that use the kit, including new Panel callers. Allowances are only for documented legacy non-dense paths; a new dense element needs no guard allowance because its raw implementation lives inside the kit.

## Remaining legacy raw controls

The bounded allowances in `ui_kit_guard_tests.rs` cover:

| Source | Maximum calls | Reason |
| --- | ---: | --- |
| `dialogs.rs` | 10 | Retired window controls, legacy selection chips, the legacy combo and its choices, and guarded legacy checkboxes. Dense error actions and pattern toggles now use kit elements; Measure, Section Analysis, Rename and the feature deletion dialog are dense cards. |
| `dialogs_assembly.rs` | 6 | Non-dense Drive Joint slider, Motion Study compute action and joint-browser actions. |
| `dialogs_sheet.rs` | 2 | Flat-pattern exports remain outside this Manage port. |

Other retired non-sketch dialog sources keep their existing raw controls; they are outside this dense-panel port. Sketch modify, text, text move and attachment panels have no raw-control allowance. Dense adapters, the sketch palette and future sources using the kit are scanned recursively. Kit implementations themselves own the raw widgets and require no allowances. Guarded legacy text editors in `dialogs.rs` and `dialogs_plastic.rs` also remain for the old path.

## Verification and legacy boundaries

`ui_kit::gallery` renders every element in light and dark without a native window. Moved tests cover geometry, labels, field focus/completion, disclosures and actions. Panel tests cover both card layouts and short-height scrolling. `ui_kit_guard_tests` rejects raw controls in dense sources; the small allowance table names the remaining retired dialog paths. Standard font/colour guards, clippy, wasm, assets and layers still apply.

## Manage cards

Parameters, configurations (including rules and preview), physical material, appearance
(including local library), sheet/plastic rule managers and material-card review compose
these elements through `Panel`. Editable tables use `wide()` and horizontal scrolling
when columns exceed the bounded viewport. No tabs or progress rows are needed by these
existing workflows. Read [the Manage audit](manage-ui-kit-audit.md) for command recipes,
the initial inventory, immediate command entries and the remaining application surfaces.

Host-native Open/Save sheets remain in the host service. Expression completion and the
colour chooser retain their shared popup behavior; the latter is owned by `color_field`
but its internal HSV/RGB controls still use egui's built-in picker.

## Inline placement and motion controls

- `timeline_slider(ui, value, range, whole_steps)` draws a slider without a numeric editor. Motion timelines supply `true` for integer steps; Drive Joint supplies `false` and retains its expression editor and range caption. Tokens: `control_height`, `space_4`. Values are rounded only by manipulation, never just by rendering.
- `placement_field(ui, id, value, hint)` retains the caller's focus ID and leaves Enter and Tab with the drawing tool, so rejecting an expression keeps the field available for correction. Completion stays with the caller. Tokens: `control_fill`, `text`, `font_body`, `control_height`, `space_2/4/6`. The floating canvas container and placement calculation remain outside the kit.
- `bounded_cell_editor(ui, id, value, hint, invalid, limit)` is the stable cell editor with a Unicode character cap, used for render setup names (128 characters). Tokens: `cell_editor` tokens. `cell_editor` uses the same implementation without an additional cap.

All three have light/dark gallery examples; keyboard and character-cap checks cover the two editors.
