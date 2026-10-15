//! Install as an app (the browser build). A browser that can install the web app (a PWA: its own
//! window and launcher icon, working offline) offers it to the page; while it does, an Install
//! button sits in the application bar beside Report a bug, and clicking it shows the browser's
//! own install prompt. An installed app, or the app running installed, gets no offer, so the
//! button is gone. The host keeps the offer (`Services::install_app`); desktop builds have none.
//!
//! Safari installs web apps too (Add to Home Screen on iPhone and iPad, Add to Dock on a Mac) but
//! offers no prompt a page can show and can't tell the page whether the app is installed. There
//! the button opens a window with the steps, which can hide the button for good (Preferences ▸
//! General brings it back).

use egui::RichText;

use crate::SolveApp;
use crate::theme::Tokens;

/// How the browser installs the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// Its own install prompt (`InstallApp::prompt`).
    Prompt,
    /// No prompt a page can show: SolidCraft shows these steps (Safari).
    Steps(Steps),
}

/// Safari's steps for installing a web app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steps {
    /// iPhone and iPad: Share ▸ Add to Home Screen.
    HomeScreen,
    /// A Mac (Safari 17 or later): File ▸ Add to Dock.
    Dock,
}

impl Steps {
    /// The steps' message ids, in order, and the one that says what follows.
    fn messages(self) -> (&'static [&'static str], &'static str) {
        match self {
            Steps::HomeScreen => (&["install.home_screen_1", "install.home_screen_2", "install.home_screen_3"], "install.home_screen_after"),
            Steps::Dock => (&["install.dock_1", "install.dock_2"], "install.dock_after"),
        }
    }
}

/// The browser's offer to install the app, kept by the host.
pub struct InstallApp {
    /// What the browser offers now; None: nothing (installed, or it can't install).
    pub offer: Box<dyn Fn() -> Option<Offer>>,
    /// Show the browser's install prompt (`Offer::Prompt`). Browsers only show it right after a
    /// click.
    pub prompt: Box<dyn Fn()>,
}

/// Safari's steps for this browser, from its `navigator.userAgent`, `navigator.vendor` and
/// `navigator.maxTouchPoints`; None: not Safari, or a Safari that can't install web apps.
/// Other iOS browsers are WebKit too (vendor Apple) but have no `Version/` and name themselves.
pub fn safari_steps(user_agent: &str, vendor: &str, touch_points: i32) -> Option<Steps> {
    let other = ["CriOS/", "FxiOS/", "EdgiOS/", "OPiOS/", "OPT/"];
    let safari = vendor.starts_with("Apple") && user_agent.contains("Safari/") && !other.iter().any(|o| user_agent.contains(o));
    if !safari {
        return None;
    }
    let version = user_agent.split("Version/").nth(1)?;
    // iPadOS Safari presents itself as a Mac; only its touch screen tells them apart.
    let mobile = ["iPhone", "iPad", "iPod"].iter().any(|d| user_agent.contains(d));
    if mobile || (user_agent.contains("Macintosh") && touch_points > 1) {
        return Some(Steps::HomeScreen);
    }
    let major: u32 = version.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    (user_agent.contains("Macintosh") && major >= 17).then_some(Steps::Dock)
}

/// What is on offer now, unless the user hid Safari's steps.
fn offered(app: &SolveApp) -> Option<Offer> {
    let offer = app.services.install_app.as_ref().and_then(|i| (i.offer)())?;
    (offer == Offer::Prompt || !app.preferences.install_steps_hidden).then_some(offer)
}

/// The application bar's Install button, at the right of `span`; returns its rect (None: not shown).
pub fn button(app: &mut SolveApp, ui: &mut egui::Ui, span: egui::Rect, labeled: bool) -> Option<egui::Rect> {
    let offer = offered(app)?;
    let (rect, response) = crate::toolbar::offer_button(ui, span, labeled, crate::i18n::tr("install.button"), "install_app", "titlebar:install")?;
    if response.on_hover_text(crate::i18n::tr("install.button_tip")).clicked() {
        match offer {
            Offer::Prompt => {
                if let Some(install) = &app.services.install_app {
                    (install.prompt)();
                }
            }
            Offer::Steps(steps) => app.install_steps = if app.install_steps.is_some() { None } else { Some(steps) },
        }
    }
    Some(rect)
}

/// The window with Safari's steps, while open.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(steps) = app.install_steps else { return };
    let t = Tokens::get();
    let (lines, after) = steps.messages();
    let mut open = true;
    let mut hide = None;
    crate::frame::window(ctx, crate::i18n::tr("install.title"), crate::frame::Width::Normal)
        .id(egui::Id::new("sc_install_steps"))
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("install.steps_intro"));
            ui.add_space(4.0);
            for (i, id) in lines.iter().enumerate() {
                ui.label(format!("{}. {}", i + 1, crate::i18n::tr(id)));
            }
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(crate::i18n::tr(after)).color(t.text_dim)).wrap());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("install.done")).clicked() {
                    hide = Some(false);
                }
                if ui.button(crate::i18n::tr("install.hide")).on_hover_text(crate::i18n::tr("install.hide_tip")).clicked() {
                    hide = Some(true);
                }
            });
        });
    if hide == Some(true) {
        app.preferences.install_steps_hidden = true;
    }
    if !open || hide.is_some() {
        app.install_steps = None;
    }
}

/// Preferences ▸ General: show or hide the Install button where Safari's steps are on offer.
pub fn prefs_rows(app: &mut SolveApp, ui: &mut egui::Ui) {
    let steps = app.services.install_app.as_ref().and_then(|i| (i.offer)());
    if !matches!(steps, Some(Offer::Steps(_))) {
        return;
    }
    ui.label(crate::i18n::tr("install.prefs_label"));
    let mut show = !app.preferences.install_steps_hidden;
    if ui.checkbox(&mut show, crate::i18n::tr("install.prefs_show")).changed() {
        app.preferences.install_steps_hidden = !show;
    }
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use serde_json::json;

    use super::*;
    use crate::scenario::{Harness, handle_at};

    const HANDLE: &str = "titlebar:install";

    fn host(h: &mut Harness, offer: Rc<Cell<Option<Offer>>>, prompts: Rc<Cell<u32>>) {
        h.app.services.install_app =
            Some(InstallApp { offer: Box::new(move || offer.get()), prompt: Box::new(move || prompts.set(prompts.get() + 1)) });
    }

    fn click_install(h: &mut Harness) {
        let at = handle_at(HANDLE).map(|p| [p.x, p.y]);
        assert!(at.is_some(), "the Install button is shown");
        assert_eq!(h.step(&json!({ "click": at })), Ok(()));
    }

    #[test]
    fn install_shows_while_offered_and_prompts_on_click() {
        let offer = Rc::new(Cell::new(None));
        let prompts = Rc::new(Cell::new(0));
        let mut h = Harness::new();
        host(&mut h, offer.clone(), prompts.clone());
        h.frames(2);
        assert!(handle_at(HANDLE).is_none(), "no offer, no button");

        offer.set(Some(Offer::Prompt));
        h.frames(2);
        let report = handle_at("titlebar:report").map_or(0.0, |p| p.x);
        assert!(handle_at(HANDLE).is_some_and(|p| p.x < report), "left of Report a bug");
        click_install(&mut h);
        assert_eq!(prompts.get(), 1, "a click shows the browser's prompt");
        assert_eq!(h.app.install_steps, None, "no steps window for a browser with a prompt");

        offer.set(None);
        h.frames(2);
        assert!(handle_at(HANDLE).is_none(), "installed: the button is gone");
    }

    #[test]
    fn safari_shows_its_steps_until_hidden() {
        let offer = Rc::new(Cell::new(Some(Offer::Steps(Steps::Dock))));
        let prompts = Rc::new(Cell::new(0));
        let mut h = Harness::new();
        host(&mut h, offer, prompts.clone());
        h.frames(2);
        click_install(&mut h);
        assert_eq!(h.app.install_steps, Some(Steps::Dock), "a click opens the steps");
        assert_eq!(prompts.get(), 0, "Safari has no prompt");
        assert_eq!(h.step(&json!({ "widget": crate::i18n::tr("install.done") })), Ok(()));
        assert_eq!(h.app.install_steps, None, "Done closes the steps");
        assert!(handle_at(HANDLE).is_some(), "and the button stays");

        click_install(&mut h);
        assert_eq!(h.step(&json!({ "widget": crate::i18n::tr("install.hide") })), Ok(()));
        h.frames(2);
        assert!(h.app.preferences.install_steps_hidden, "Don't show again is remembered");
        assert_eq!(h.app.install_steps, None);
        assert!(handle_at(HANDLE).is_none(), "the button is hidden");

        h.app.preferences.install_steps_hidden = false;
        h.frames(2);
        assert!(handle_at(HANDLE).is_some(), "the preference brings it back");
    }

    #[test]
    fn preferences_bring_safaris_button_back() {
        let mut h = Harness::new();
        host(&mut h, Rc::new(Cell::new(Some(Offer::Steps(Steps::HomeScreen)))), Rc::new(Cell::new(0)));
        h.app.preferences.install_steps_hidden = true;
        h.call("ui.prefs", json!({ "open": "General" }));
        let page = crate::scenario::rect_of("prefs:window").map(|r| r.center());
        assert!(page.is_some(), "Preferences is open");
        let [x, y] = page.map_or([0.0, 0.0], |p| [p.x, p.y]);
        h.call("ui.scroll", json!({ "x": x, "y": y, "delta": -100 }));
        h.frames(3);
        assert_eq!(h.step(&json!({ "widget": crate::i18n::tr("install.prefs_show") })), Ok(()));
        h.call("ui.prefs", json!({ "button": "ok" }));
        h.frames(2);
        assert!(!h.app.preferences.install_steps_hidden);
        assert!(handle_at(HANDLE).is_some(), "the button is back");
    }

    #[test]
    fn no_preferences_row_where_the_browser_prompts() {
        let mut h = Harness::new();
        host(&mut h, Rc::new(Cell::new(Some(Offer::Prompt))), Rc::new(Cell::new(0)));
        h.call("ui.prefs", json!({ "open": "General" }));
        assert!(h.widget(crate::i18n::tr("install.prefs_show")).is_none());
    }

    #[test]
    fn install_never_covers_the_bars_other_buttons() {
        let mut h = Harness::new();
        host(&mut h, Rc::new(Cell::new(Some(Offer::Prompt))), Rc::new(Cell::new(0)));
        for width in [320.0, 340.0, 360.0, 400.0, 639.0, 640.0, 900.0, 1600.0] {
            h.call("ui.resize", json!({ "width": width, "height": 700 }));
            h.frames(2);
            // Only a bar too narrow for even the icon leaves it out; phones are 360 px or wider.
            let Some(install) = crate::scenario::rect_of(HANDLE) else {
                assert!(width < 360.0, "{width} px: room for Install, but it isn't shown");
                continue;
            };
            for other in ["appbar:file", "appbar:undo", "appbar:redo", "appbar:help", "titlebar:report"] {
                // The bar draws Undo and Redo only when it has room for them.
                let Some(o) = crate::scenario::rect_of(other) else { continue };
                let shared = install.intersect(o);
                assert!(shared.width() <= 0.0 || shared.height() <= 0.0, "{width} px: Install ({install:?}) covers {other} ({o:?})");
            }
        }
    }

    #[test]
    fn hiding_safaris_steps_never_hides_a_browser_prompt() {
        let mut h = Harness::new();
        host(&mut h, Rc::new(Cell::new(Some(Offer::Prompt))), Rc::new(Cell::new(0)));
        h.app.preferences.install_steps_hidden = true;
        h.frames(2);
        assert!(handle_at(HANDLE).is_some());
    }

    #[test]
    fn no_install_button_without_the_host_service() {
        let mut h = Harness::new();
        h.frames(2);
        assert!(handle_at(HANDLE).is_none());
    }

    #[test]
    fn safari_steps_by_device() {
        const APPLE: &str = "Apple Computer, Inc.";
        let iphone =
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
        let mac =
            |v: &str| format!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{v} Safari/605.1.15");
        assert_eq!(safari_steps(iphone, APPLE, 5), Some(Steps::HomeScreen));
        assert_eq!(safari_steps(&mac("18.5"), APPLE, 5), Some(Steps::HomeScreen), "an iPad presents itself as a Mac");
        assert_eq!(safari_steps(&mac("18.5"), APPLE, 0), Some(Steps::Dock));
        assert_eq!(safari_steps(&mac("17.0"), APPLE, 0), Some(Steps::Dock));
        assert_eq!(safari_steps(&mac("16.6"), APPLE, 0), None, "Add to Dock came with Safari 17");
        let chrome_ios = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/138.0.7204.156 Mobile/15E148 Safari/604.1";
        assert_eq!(safari_steps(chrome_ios, APPLE, 5), None, "Chrome on iOS isn't Safari");
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
        assert_eq!(safari_steps(chrome, "Google Inc.", 0), None, "Chrome prompts instead");
        let firefox = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:144.0) Gecko/20100101 Firefox/144.0";
        assert_eq!(safari_steps(firefox, "", 0), None);
        assert_eq!(safari_steps("", APPLE, 0), None, "hostile input is just not Safari");
        assert_eq!(safari_steps("Safari/ Version/99999999999999999999 Macintosh", APPLE, 0), None);
    }
}
