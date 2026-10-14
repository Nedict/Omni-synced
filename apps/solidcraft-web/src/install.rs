//! Install as an app (`solidcraft_ui_egui::install`). The browser offers it with a
//! `beforeinstallprompt` event, which can come before this code runs: index.html keeps the event
//! as `window.solidcraftInstall` (and drops it once the app is installed). The offer is good for
//! one prompt; a browser that is still willing makes a new one later.

use solidcraft_ui_egui::install::InstallApp;
use wasm_bindgen::{JsCast as _, JsValue};

/// Where index.html keeps the browser's offer.
const OFFER: &str = "solidcraftInstall";

/// The browser's offer, kept by index.html (None: none, or it was used).
fn offer() -> Option<js_sys::Object> {
    let window = web_sys::window()?;
    js_sys::Reflect::get(&window, &OFFER.into()).ok()?.dyn_into().ok()
}

/// Show the browser's install prompt (from the Install button's click, so the page may).
fn prompt() {
    let Some(offer) = offer() else { return };
    if let Some(window) = web_sys::window() {
        let _ = js_sys::Reflect::set(&window, &OFFER.into(), &JsValue::NULL);
    }
    let shown = js_sys::Reflect::get(&offer, &"prompt".into())
        .and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from))
        .and_then(|f| f.call0(&offer))
        .and_then(|p| p.dyn_into::<js_sys::Promise>().map_err(JsValue::from));
    match shown {
        // Accepted or dismissed, the browser handles it; a refusal is only logged.
        Ok(p) => wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = wasm_bindgen_futures::JsFuture::from(p).await {
                log::warn!("install prompt: {e:?}");
            }
        }),
        Err(e) => log::warn!("install prompt: {e:?}"),
    }
}

/// The host service for the Install button; the app bar is drawn again whenever the browser
/// makes or withdraws its offer (even with nothing else happening).
pub fn service(ctx: &egui::Context) -> InstallApp {
    if let Some(window) = web_sys::window() {
        for event in ["beforeinstallprompt", "appinstalled"] {
            let ctx = ctx.clone();
            let redraw = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || ctx.request_repaint());
            let _ = window.add_event_listener_with_callback(event, redraw.as_ref().unchecked_ref());
            // The listener lives as long as the page.
            redraw.forget();
        }
    }
    InstallApp { offered: Box::new(|| offer().is_some()), prompt: Box::new(prompt) }
}
