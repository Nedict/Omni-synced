# SolidCraft UI kit

`crates/ui-egui/src/ui_kit/` is the tokenised library used to assemble dense feature dialogs and the sketch palette. This extraction preserves their existing layout, painting, focus, accessibility labels and scenario identities. Sketch tool panels use the same composition and elements.

## Module tree

```text
ui_kit/
  mod.rs                 overview and the one caller import path
  metrics.rs             row/header dimensions, sanitising and label-column maths
  text.rs                weighted, clipped, pixel-snapped text
  layout.rs              form scope, aligned rows, captions, hints, sections, dividers, picked lists
  panel.rs               bounded windows, surfaces, feature/palette composition and compute status
  fields.rs              expressions, plain text, units, dropdowns, selections and paired values
  choice.rs              text/icon segments, checkboxes, wrapped modes and icon toggles
  actions.rs             buttons, equal secondary pairs, keyboard and OK/Apply/Cancel footers
  status.rs              supplied name/status/icon and constraint meter
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
- Scenario names and accessibility labels are API: preserve them. New elements publish geometry without overwriting the native accessible label or checked state.
- `footer(..., None)` retains the original OK/Cancel layout. `Some("Apply")` adds the third action and returns `Action::Apply`; the panel decides whether to keep the operation open.

## Elements

All calls below use `crate::ui_kit::`; snippets assume local values and normal enclosing form scope. `control_*` means `control_fill`, `control_border` and/or `control_radius` as used by that element.

| Module / element | Purpose | Tokens | Example |
| --- | --- | --- | --- |
| `metrics::row_height` | Row height for controls | `control_height` | `row_height()` |
| `metrics::header_height` | Feature heading height | `form_header_height` | `header_height()` |
| `layout::scope` | Inherited form styling and label-column identity | `space_4/8, form_gap_y, control_height, font_label, accent, selected_bg, dialog_rule, control_fill/border/radius, text` | `scope(ui, \|ui\| { /* elements */ })` |
| `layout::row` | Aligned label/control row, 24 pt today | `control_height, form_label_width, font_label, dialog_label, space_8` | `row(ui, "Radius", \|ui\| number_field(ui, &mut radius, "mm"))` |
| `layout::caption` | Dim helper beneath a row control | `form_icon_size, font_small, text_dim, space_8; label-column tokens` | `caption(ui, "Line Type", "Construction")` |
| `layout::section` | Persisted disclosure with a rule and uppercase title | `control_height, space_2, radius_sm, hover, dialog_rule, form_section_icon_size, form_gap_y, font_caption, font_section_tracking, text_dim` | `section(ui, "advanced", "Advanced", false, \|ui\| {})` |
| `layout::hint` | Helper text below a card header | `control_height, font_small, text_dim` | `hint(ui, "pick", "Pick a point")` |
| `layout::note` | Wrapping explanatory text in a card body, for rules or caveats too long for a hint | `font_small, text_dim, space_4` | `note(ui, "pattern", "Counts include the original.")` |
| `layout::list_row` | Picked-item row with a remove action | `control_height, space_8, font_label, text; icon-button tokens` | `list_row(ui, "point-1", "Point 1").clicked()` |
| `layout::divider` | Full-width rule without extra padding | `dialog_rule` | `divider(ui, "group")` |
| `panel::header` | Feature title/icon/close band | `form_header_height, form_icon_size, form_close_size, form_inset, font_label, text, dialog_rule, space_8/10; icon_button_at tokens` | `header(ui, "fillet", "Fillet", || {})` |
| `fields::field` | Expression editor; the value carries its unit text ("20 mm", "45 deg"), there is no unit suffix; returns accepted Enter | `control_height, control_fill, form_inset, font_label, text_dim, space_4; scope tokens` | `field(ui, &mut expression)` |
| `fields::focused_field` | Single-value expression popup retaining keyboard focus | `field tokens` | `focused_field(ui, &mut expression)` |
| `fields::hint_field` | Expression editor with empty-value hint | `same as field` | `hint_field(ui, &mut depth, "To the next face")` |
| `fields::text_field` | Plain text editor without expression completion; returns (Enter, changed) | `same as field` | `text_field(ui, &mut name, "Name")` |
| `fields::multiline_text_field` | Three-line plain-text row with stable focus ID; caller routes Enter | `control_height, control_fill, form_inset, font_label; row tokens` | `multiline_text_field(ui, "Text", id, &mut text)` |
| `fields::searchable_dropdown`, `SearchOption` | Search/list dropdown with optional custom-value action and loading message | `control_height, space_24, font_label; scope control tokens` | `searchable_dropdown(ui, "font", current, &mut filter, &choices, custom, pending)` |
| `fields::value_pair_named` | Paired X/Y captions retaining independent accessible/scenario names | `value_pair tokens` | `value_pair_named(ui, "Position", [x, y], ["X", "Y"], ["Distance X", "Distance Y"], "mm")` |
| `choice::text_emphasis` | Compact bold/italic toggles with localized accessible names | `scope font/control tokens` | `text_emphasis(ui, &mut bold, &mut italic)` |
| `fields::dropdown` | Native dropdown with kit arrow and scenario option handles | `text_dim; scope control/font tokens` | `dropdown(ui, "operation", &mut operation, &["Join", "Cut"])` |
| `fields::selection_field` | Filled point/curve/edge selection target | `control_height, control_radius, selected_bg/ink, accent, form_inset, space_24/8, font_label, text_dim` | `selection_field(ui, "Point 1", active)` |
| `fields::selection_field_state` | Selection target with explicit filled state | `same as selection_field` | `selection_field_state(ui, "Select", active, false)` |
| `fields::value_pair` | Two labelled numeric fields in one row; returns accepted Enter | `row/field tokens, space_8, dialog_label` | `value_pair(ui, "Position", [&mut x, &mut y], ["X", "Y"], "mm")` |
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
| `panel::Panel` | Docked/floating window and both card compositions | `form_card_width`, frame/header/body/footer tokens above | `Panel::new(title, id, anchor, viewport, side, frame).window()` |
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
| `dialogs.rs` | 13 | Retired window controls, legacy selection chips, the legacy combo and its choices, guarded legacy checkboxes and parameter-table Add. Dense error actions and pattern toggles now use kit elements. |
| `dialogs_assembly.rs` | 6 | Non-dense Drive Joint slider, Motion Study compute action and joint-browser actions. |
| `dialogs_plastic.rs` | 1 | Non-dense Manage Rules active-rule checkbox. |
| `dialogs_sheet.rs` | 4 | Non-dense Manage Rules checkbox/delete and flat-pattern exports. |

Other retired non-sketch dialog sources keep their existing raw controls; they are outside this dense-panel port. Sketch modify, text, text move and attachment panels have no raw-control allowance. Dense adapters, the sketch palette and future sources using the kit are scanned recursively. Kit implementations themselves own the raw widgets and require no allowances. Guarded legacy text editors in `dialogs.rs` and `dialogs_plastic.rs` also remain for the old path.

## Verification and legacy boundaries

`ui_kit::gallery` renders every element in light and dark without a native window. Moved tests cover geometry, labels, field focus/completion, disclosures and actions. Panel tests cover both card layouts and short-height scrolling. `ui_kit_guard_tests` rejects raw controls in dense sources; the small allowance table names the remaining retired dialog paths. Standard font/colour guards, clippy, wasm, assets and layers still apply.
