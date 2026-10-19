//! Dense panels compose kit elements. Legacy, non-dense paths have bounded allowances.
use std::path::Path;
use syn::visit::{self, Visit};

/// Counts may shrink; additions need a specific reason. Dense-only files have no allowance.
const ALLOWED: &[(&str, usize, &str)] = &[
    ("dialogs.rs", 13, "retired non-dense window and explicitly guarded legacy selection/field helpers"),
    ("dialogs_assembly.rs", 6, "non-dense Drive Joint slider, Motion Study compute and joint browser actions"),
    ("dialogs_plastic.rs", 1, "non-dense Manage Rules active-rule checkbox"),
    ("dialogs_sheet.rs", 4, "non-dense Manage Rules and flat-pattern export actions"),
];

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
            "checkbox" | "button" | "small_button" | "selectable_label" | "selectable_value" | "radio_value" | "toggle_value"
        ) {
            self.found.push(format!(".{}", call.method));
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let names: Vec<_> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if names.iter().any(|n| matches!(n.as_str(), "ComboBox" | "Button" | "Slider" | "Checkbox" | "RadioButton") || self.aliases.contains(n)) {
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
            syn::UseTree::Rename(r) if matches!(r.ident.to_string().as_str(), "ComboBox" | "Button" | "Slider" | "Checkbox" | "RadioButton") => {
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
    assert!(failures.is_empty(), "Use crate::ui_kit, or document a bounded legacy-path allowance:\n{}", failures.join("\n"));
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
