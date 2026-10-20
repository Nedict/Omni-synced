# SolidCraft UI kit

`crates/ui-egui/src/ui_kit/` contains the tokenised controls used by feature dialogs,
preferences, manager cards, prompts and the sketch palette. Import through
`crate::ui_kit::{...}`. The API is crate-visible; implementation modules are private.

## Composition and rules

- Colours, typography, dimensions and spacing come from `theme::Tokens`.
- `scope(ui, add)` supplies form styling. `row(ui, label, add)` aligns labels and
  controls at `control_height` (24 pt today). Labels widen from `form_label_width`
  to at most 1.6 times that width and never exceed half the row.
- `expr_row`, `hint_row`, `text_row` and `choice_row` compose `row` with the existing
  expression, hinted expression, plain text and dropdown controls. They preserve row order,
  labels, focus IDs and completion; callers keep Enter/change ownership and explicit dropdown IDs.
- `Panel` owns bounded windows, feature/palette composition, scrolling bodies,
  pinned actions, compute status and keyboard routing. `wide()` uses three card
  widths. `feature_frame` and `palette_frame` retain docked/floating surfaces.
  `FeatureOptions::apply_label` adds Apply; `Panel::actions` supplies custom labels.
- `Prompt` owns floating confirmation/property cards. Use `actions(ok, cancel,
  apply)`, `anchor`/`placed`, `cards`, `selected`, `fields` and
  `keep_open_on_escape` for existing prompt behavior. Empty `ok` omits the primary
  action. It returns `(close_pressed, Action)`.
- Callers supply data and enabled state, then dispatch existing commands. Editors
  leave their established completion, Enter, Tab and commit ownership unchanged.
- Scenario handles and accessible labels are API. Consolidated options preserve
  the former geometry, wrapping, native metadata and publication keys.

## Elements

Module names below describe implementation locations, not caller import paths.
Every inline element/variant has one entry in the light/dark `gallery::ELEMENTS`
table. Panel/Prompt compositions also have dedicated geometry and interaction tests.

| Module / element | Use and options | Main tokens |
| --- | --- | --- |
| metrics: `row_height`, `header_height` | Shared control/header heights | `control_height`, `form_header_height` |
| layout: `scope`, `row` | Form styling and aligned label/control columns | `form_label_width`, `form_gap_y`, `control_*`, `font_label`, `dialog_label`, `space_4/8` |
| `caption` | Dim caption aligned beneath a control | `font_small`, `text_dim`, label-column tokens |
| `section`, `divider` | Persisted disclosure and full-width rule | `control_height`, `dialog_rule`, `font_caption`, tracking, spacing |
| `list_row` | Picked item with a remove target | `control_height`, `font_label`, icon/spacing tokens |
| `text`, `Text` | Hint, note, report, inline or cell text; see variants below | `font_small/label`, `text/dim/error`, `control_height`, `space_4` |
| `text_lines` | Virtualized read-only licence/report lines, horizontal overflow | `font_small`, `text`, `control_height` |
| fields: `field`, `hint_field`, `focused_field` | Expression editors, optional empty hint or retained focus; returns accepted Enter | `control_height/fill`, `form_inset`, `font_label` |
| `text_field`, `focused_text_field` | Plain name editor, optionally retaining focus | field tokens |
| `multiline_text_field` | Three-line plain-text row with stable focus ID | field/row tokens |
| `password_field` | Stable secret editor; excludes the secret from scenario/accessibility metadata | field tokens |
| `message_field` | Growing message editor; owner retains scroll and Enter routing | field tokens |
| `cell_editor`, `bounded_cell_editor`, `search_field` | Stable-ID inline/search editors; bounded variant caps Unicode characters | field tokens, `text/error` |
| `text_area` | Bounded multiline JSON/data editor | field tokens |
| `dropdown`, `dropdown_menu` | Indexed choices or caller-supplied menu controls | `control_height`, `text_dim`, form tokens |
| `searchable_dropdown`, `SearchOption` | Filtered choices, custom-value action and pending message | dropdown tokens |
| `selection_field`, `Selection` | Selection chip with explicit `active` and `filled` state | `selected_bg/ink`, `accent`, `control_*`, `form_inset`, `space_8/24` |
| `value_pair` | Paired captions/expressions; optional independent field names | row/field tokens, `space_8` |
| `fraction_field`, `scalar_field` | Locale-aware bounded dimensionless values | form control/font tokens |
| `number_field` | Integer/decimal field with caller range, drag speed and suffix | form control/font tokens |
| `length_field`, `angle_field` | Length in design units, angle in degrees | form control/font tokens |
| `slider`, `formatted_slider` | Continuous slider or stepped preference slider with caller formatting | `control_height`, form control/font tokens |
| `timeline_slider` | Slider without editor; optional whole-step manipulation | `control_height`, `space_4` |
| `placement_field` | Stable focus ID; drawing tool owns Enter/Tab validation and completion | `control_fill`, `font_body`, `text`, `control_height`, spacing |
| `color_field` | Document RGB chooser, retaining egui's internal HSV/RGB popup | form control/font tokens |
| choice: `segmented`, `Segments` | Equal text choices; `states` supplies mixed selection, `compact` clips instead of falling back to a dropdown | `control_*`, `selected_bg/ink`, `font_label`, `space_4/8` |
| `segmented_icons`, `IconSegment` | Equal icon choices and mixed states | segment tokens, icon palette |
| `checkbox`, `checkbox_row`, `checkbox_grid`, `options` | Inline, full-row, bounded-column or disclosed checkbox choices | `control_*`, `accent`, `on_accent`, typography/spacing |
| `text_emphasis` | Bold/italic with translated accessible names | form control/font tokens |
| `tool_choices`, `ToolChoice`, `toggle_icon` | Wrapped command modes or independent icon toggles | sketch mode/icon tokens, `accent` |
| `list_picker` | Selectable row with caller identity and native checked state | `control_height`, form selection tokens |
| `popup_list`, `PopupOption` | Focus-neutral suggestions; owner retains keyboard routing | `space_2`, `form_label_width`, `control_height`, form tokens |
| `color_swatch`, `swatch_grid` | Wrapped draggable document colours with caller payloads | `control_height/radius`, `border`, `accent`, `hover`, `space_4` |
| actions: `button`, `Button` | Secondary action: `Normal`, `Small`, or `Enabled(bool)` | native sizing, form font/control tokens |
| `primary_button` | Accent action outside a footer | `accent`, `on_accent`, form tokens |
| `icon_button_at` | Icon action inside a supplied rectangle | `form_icon_inset`, `text/dim`, icon palette |
| `button_pair`, `PairButton` | Equal secondary actions, optional icons and selected state | `form_button_height`, control/selection/icon tokens |
| `footer`, `Footer`, `Action` | OK/Cancel with optional Apply, compute status or selected action | `form_footer/button_height`, `dialog_footer/rule`, `control_*`, `accent`, spacing |
| `keyboard` | Esc cancels, unowned Enter submits when valid | none |
| `action_menu` | Nested dense action menu | form control/font tokens |
| table: `table`, `Column`, `Table::row`, `Table::column_actions`, `TableRow::cell` | Left-aligned headers/cells, horizontal overflow, selectable rows, attached column-action band and empty state; `Column::new(label, width)` uses label-column multiples, `min_content_width(points)` reserves measured control content | label-column tokens, `control_height`, `dialog_rule/footer`, `selected_bg`, spacing |
| `label_actions` | Clipped row name beside compact actions | `control_height`, form font/spacing tokens |
| status: `status_header`, `StatusHeader` | Supplied icon/name/status and constraint meter | `font_label`, `text/dim`, `accent`, sketch/header tokens |
| `progress` | Clamped determinate progress | `space_3/6`, `dialog_rule`, `accent` |

`text(ui, text, options)` preserves five existing text behaviors:

| Option | Layout and scenario publication |
| --- | --- |
| `Text::Hint(key)` | One fixed row; tooltip, rect and handle `dialog:hint:KEY` |
| `Text::Note(key)` | Wrapping small dim text with vertical padding; rect `dialog:note:KEY` |
| `Text::Report(key, error)` | Native wrapping label, optional error colour; rect/handle `dialog:report:KEY` |
| `Text::Inline` | Native truncated label; rect/handle `dialog:inline:TEXT` |
| `Text::Cell` | Fixed-height clipped clickable cell with tooltip; rect/handle `dialog:cell:TEXT` |

Examples:

```rust
segmented(ui, &mut mode, &["One", "Two"], Segments::default());
selection_field(ui, "Select", Selection { active: true, filled: false });
value_pair(ui, "Position", [&mut x, &mut y], ["X", "Y"], Some(["Distance X", "Distance Y"]));
button(ui, "Add", Button::Enabled(valid));
footer(ui, "OK", "Cancel", valid, Footer { apply: Some("Apply"), ..Default::default() });
text(ui, report, Text::Report("validation", invalid));
```

## Extending and checking the kit

Add one implementation, document its tokens/options here, re-export it through
`mod.rs`, and add one table entry in `gallery::ELEMENTS`. Keep implementation-only
helpers private. Add interaction tests when the element introduces an action.

The gallery checks geometry, label alignment, handles and effective paint bounds in
both themes. Field/footer/panel tests retain focus, completion and action coverage.
`ui_kit_guard_tests` scans kit callers recursively, rejects raw control constructors
and methods (including aliases), and rejects removed legacy helpers. The raw-control
allowance table is empty and guarded against additions. Standard colour/font guards,
Clippy, wasm, i18n, assets, icons and layers still apply.

See [remaining coverage boundaries](manage-ui-kit-audit.md) for specialised surfaces
that remain outside the kit and why.
