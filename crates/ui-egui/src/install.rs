//! Install as an app (the browser build). A browser that can install the web app (a PWA: its own
//! window and launcher icon, working offline) offers it to the page; while it does, an Install
//! button sits in the application bar beside Report a bug, and clicking it shows the browser's
//! own install prompt. An installed app, or the app running installed, gets no offer, so the
//! button is gone. The host keeps the offer (`Services::install_app`); desktop builds have none.

use crate::SolveApp;

/// The browser's offer to install the app, kept by the host.
pub struct InstallApp {
    /// The browser offers to install the app now.
    pub offered: Box<dyn Fn() -> bool>,
    /// Show the browser's install prompt. Browsers only show it right after a click.
    pub prompt: Box<dyn Fn()>,
}

/// The application bar's Install button, right of `right`; returns its rect (None: not shown).
pub fn button(app: &mut SolveApp, ui: &mut egui::Ui, right: f32, top: f32, labeled: bool) -> Option<egui::Rect> {
    let install = app.services.install_app.as_ref().filter(|i| (i.offered)())?;
    let (rect, response) =
        crate::toolbar::offer_button(ui, right, top, labeled, crate::i18n::tr("install.button"), "install_app", "titlebar:install");
    if response.on_hover_text(crate::i18n::tr("install.button_tip")).clicked() {
        (install.prompt)();
    }
    Some(rect)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use serde_json::json;

    use super::*;
    use crate::scenario::{Harness, handle_at};

    const HANDLE: &str = "titlebar:install";

    #[test]
    fn install_shows_while_offered_and_prompts_on_click() {
        let offered = Rc::new(Cell::new(false));
        let prompts = Rc::new(Cell::new(0));
        let mut h = Harness::new();
        let (o, p) = (offered.clone(), prompts.clone());
        h.app.services.install_app = Some(InstallApp { offered: Box::new(move || o.get()), prompt: Box::new(move || p.set(p.get() + 1)) });
        h.frames(2);
        assert!(handle_at(HANDLE).is_none(), "no offer, no button");

        offered.set(true);
        h.frames(2);
        let at = handle_at(HANDLE).map(|p| [p.x, p.y]);
        assert!(at.is_some(), "an offer shows the button");
        let report = handle_at("titlebar:report").map_or(0.0, |p| p.x);
        assert!(at.is_some_and(|[x, _]| x < report), "left of Report a bug");
        assert_eq!(h.step(&json!({ "click": at })), Ok(()));
        assert_eq!(prompts.get(), 1, "a click shows the browser's prompt");

        offered.set(false);
        h.frames(2);
        assert!(handle_at(HANDLE).is_none(), "installed: the button is gone");
    }

    #[test]
    fn no_install_button_without_the_host_service() {
        let mut h = Harness::new();
        h.frames(2);
        assert!(handle_at(HANDLE).is_none());
    }
}
