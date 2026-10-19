//! Kit callers use shared controls; removed legacy implementations must stay removed.
use std::path::Path;
use syn::visit::{self, Visit};

/// No caller may bypass the kit for a matching control.
const ALLOWED: &[(&str, usize, &str)] = &[];

#[derive(Default)]
struct Guard {
    found: Vec<String>,
    aliases: Vec<String>,
}

impl<'ast> Visit<'ast> for Guard {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let attrs = match item {
            syn::Item::Mod(i) => &i.attrs,
            syn::Item::Fn(i) => &i.attrs,
            _ => return visit::visit_item(self, item),
        };
        if !attrs
            .iter()
            .any(|a| a.path().is_ident("test") || (a.path().is_ident("cfg") && a.parse_args::<syn::Path>().is_ok_and(|p| p.is_ident("test"))))
        {
            visit::visit_item(self, item);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if matches!(
            call.method.to_string().as_str(),
            "checkbox"
                | "button"
                | "small_button"
                | "selectable_label"
                | "selectable_value"
                | "radio_value"
                | "toggle_value"
                | "text_edit_singleline"
                | "text_edit_multiline"
        ) {
            self.found.push(format!(".{}", call.method));
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let names: Vec<_> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            // Loading an editor's cursor/focus state does not create a raw widget.
            let state_access = names.last().is_some_and(|n| matches!(n.as_str(), "load_state" | "store_state"));
            let kit_options = names.iter().any(|n| n == "ui_kit");
            if !state_access
                && !kit_options
                && names.iter().any(|n| {
                    matches!(n.as_str(), "ComboBox" | "Button" | "Slider" | "Checkbox" | "RadioButton" | "TextEdit" | "DragValue" | "ProgressBar")
                        || self.aliases.contains(n)
                })
            {
                self.found.push(names.join("::"));
            }
        }
        visit::visit_expr_call(self, call);
    }
}

fn violations(source: &str) -> Vec<String> {
    fn aliases(tree: &syn::UseTree, out: &mut Vec<String>) {
        match tree {
            syn::UseTree::Path(p) => aliases(&p.tree, out),
            syn::UseTree::Group(g) => {
                for tree in &g.items {
                    aliases(tree, out);
                }
            }
            syn::UseTree::Rename(r)
                if matches!(
                    r.ident.to_string().as_str(),
                    "ComboBox" | "Button" | "Slider" | "Checkbox" | "RadioButton" | "TextEdit" | "DragValue" | "ProgressBar"
                ) =>
            {
                out.push(r.rename.to_string())
            }
            _ => {}
        }
    }
    let ast = syn::parse_file(source).unwrap();
    let mut guard = Guard::default();
    for item in &ast.items {
        if let syn::Item::Use(u) = item {
            aliases(&u.tree, &mut guard.aliases);
        }
    }
    guard.visit_file(&ast);
    guard.found
}

#[test]
fn dense_panels_use_ui_kit_elements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut failures = Vec::new();
    fn scan(dir: &Path, prefix: &str, failures: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_str().unwrap();
            let relative = format!("{prefix}{name}");
            if path.is_dir() {
                if name != "ui_kit" {
                    scan(&path, &format!("{relative}/"), failures);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            // Covers dense adapters and every future Panel caller, including imported Panel aliases.
            if name != "dialogs_dense.rs" && name != "sketch_palette.rs" && !source.contains("ui_kit") {
                continue;
            }
            let found = violations(&source);
            let allowed = ALLOWED.iter().find(|(file, _, _)| *file == relative).map_or(0, |(_, count, _)| *count);
            if found.len() > allowed {
                failures.push(format!("{relative}: {} raw widgets, at most {allowed} allowed: {found:?}", found.len()));
            }
        }
    }
    scan(&root, "", &mut failures);
    failures.sort();
    assert!(failures.is_empty(), "Use crate::ui_kit controls:\n{}", failures.join("\n"));
}

#[test]
fn guard_detects_multiline_and_aliased_widget_calls_but_skips_tests() {
    assert_eq!(
        violations(
            r#"
        use egui::ComboBox as Combo;
        fn show(ui: &mut Ui) {
            ui.checkbox(
                value, "Label"
            );
            ui.button("Go");
            ui.selectable_label(false, "Item");
            ui.add(egui::Slider::new(value, range));
            ui.add(egui::Button::new("Go"));
            egui::ComboBox::from_id_salt("id");
            Combo::from_id_salt("alias");
        }
        #[cfg(test)] mod tests { fn sample() { ui.button("ignored"); } }
        fn after() { ui.small_button("Remove"); }
    "#
        )
        .len(),
        8
    );
}

// Declaration checks catch reintroduced implementations even before a caller reaches them.
fn legacy_declarations(source: &str, removed: &[&str]) -> Vec<String> {
    struct Declarations<'a> {
        removed: &'a [&'a str],
        found: Vec<String>,
    }
    impl<'ast> Visit<'ast> for Declarations<'_> {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            let name = item.sig.ident.to_string();
            if self.removed.contains(&name.as_str()) {
                self.found.push(name);
            }
            visit::visit_item_fn(self, item);
        }
    }
    let mut guard = Declarations { removed, found: Vec::new() };
    guard.visit_file(&syn::parse_file(source).unwrap());
    guard.found
}

#[test]
fn removed_legacy_helpers_stay_removed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files: &[(&str, &[&str])] = &[
        ("frame.rs", &["docked", "docked_on", "heading", "add_outlined", "content_width", "remember_width", "right_aligned"]),
        ("dialogs.rs", &["form_row", "publish_row"]),
        ("dialogs_appearance.rs", &["swatch", "swatch_rows"]),
        (
            "ui_kit/actions.rs",
            &["secondary_button", "small_button", "enabled_button", "footer_impl", "footer_with_status", "footer_selected", "icon_button"],
        ),
        ("ui_kit/choice.rs", &["segments", "segmented_states", "segmented_compact"]),
        ("ui_kit/fields.rs", &["selection_field_state", "value_pair_named"]),
        ("ui_kit/layout.rs", &["hint", "note", "text_block", "inline_text", "contains"]),
        ("ui_kit/table.rs", &["cell_text"]),
    ];
    for (file, removed) in files {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        assert!(legacy_declarations(&source, removed).is_empty(), "{file}: removed helper reintroduced");
    }
    assert!(ALLOWED.is_empty(), "raw-control allowance table must stay empty");
}

// Main added these floating windows after the panels branch. The helper is live for them;
// keep the bounded constructor until they migrate, and reject any new legacy callers.
#[test]
fn floating_window_helper_has_only_its_live_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let frame = std::fs::read_to_string(root.join("frame.rs")).unwrap();
    assert_eq!(legacy_declarations(&frame, &["window"]), ["window"]);
    let mut callers = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            // The guard's own expected-call string is test code.
            if path.file_name().unwrap() != "ui_kit_guard_tests.rs" && source.contains("crate::frame::window(") {
                callers.push(path.file_name().unwrap().to_str().unwrap().to_owned());
            }
        }
    }
    callers.sort();
    assert_eq!(callers, ["bom_panel.rs", "messages.rs", "space_mouse_calibration.rs"]);
}

#[test]
fn guard_detects_editor_constructors_aliases_and_legacy_declarations() {
    assert_eq!(
        violations(
            r#"
        use egui::TextEdit as Edit;
        fn draw(ui: &mut Ui) {
            ui.add(Edit::singleline(value));
            ui.add(egui::TextEdit::multiline(value));
            ui.add(egui::DragValue::new(value));
            ui.add(egui::ProgressBar::new(0.5));
            ui.text_edit_singleline(value);
            egui::TextEdit::load_state(ctx, id);
        }
    "#
        )
        .len(),
        5
    );
    assert_eq!(legacy_declarations("fn docked_on() {}", &["docked_on"]), ["docked_on"]);
}
