//! OmniSynced desktop app.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

const APP_ID: &str = "ai.storyteller.omni-synced";

fn main() {
    println!("OmniSynced starting");
}

fn app_viewport() -> Result<(), &'static str> {
    println!("window title: OmniSynced");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn omni_synced_app_id_is_set() {
        assert_eq!(super::APP_ID, "ai.storyteller.omni-synced");
    }
}
