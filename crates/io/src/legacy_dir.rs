[package]
name = "omni-synced-cli"
description = "OmniSynced headless CLI"
version.workspace = true
edition.workspace = true
license.workspace = true
default-run = "omni-synced-cli"

[lints]
workspace = true

[[bin]]
name = "omni-synced-cli"
path = "src/main.rs"

# Legacy compatibility alias kept for older scripts and MCP setups.
[[bin]]
name = "solvecraft-cli"
path = "src/alias.rs"

[dependencies]
solidcraft-engine = { workspace = true }
solidcraft-gpu = { workspace = true }
solidcraft-mcp = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
image = { workspace = true }
