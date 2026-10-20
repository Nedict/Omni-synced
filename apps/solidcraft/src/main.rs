<p align="center">
  <img src="assets/icons/app/linux/exports/hicolor/256x256/apps/ai.storyteller.solidcraft.png" width="96" height="96" alt="OmniSynced" />
</p>

<h1 align="center">OmniSynced</h1>

<p align="center">
  <strong>Parametric 3D CAD built for synced design workflows, in pure Rust.</strong><br />
  Sketch, constrain, and build parts with a parametric feature timeline. Change a dimension and watch
  your model rebuild. Run it on your desktop, in your browser, or drive it through the built-in AI tooling.
</p>

<p align="center">
  <a href="https://github.com/Nedict/Omni-synced">GitHub</a> ·
  <a href="https://github.com/Nedict/Omni-synced/issues">Issues</a> ·
  <a href="https://docs.solidcraft.io/">SolidCraft docs</a>
</p>

<p align="center">
  <a href="https://github.com/Nedict/Omni-synced/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/Nedict/Omni-synced/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://github.com/Nedict/Omni-synced/releases/latest"><img src="https://img.shields.io/github/v/release/Nedict/Omni-synced?label=release" alt="Latest release" /></a>
  <a href="https://github.com/Nedict/Omni-synced/license"><img src="https://img.shields.io/github/license/Nedict/Omni-synced" alt="Licence" /></a>
</p>

OmniSynced is a personal fork of the SolidCraft CAD stack, rebranded for my own workflows and tooling while keeping the underlying modeling engine and feature system intact.

## Build and run

```sh
cargo run --release -p omni-synced -- --sample
cargo run --release -p omni-synced-cli -- commands
cargo run --release -p omni-synced-cli -- run examples/bracket.json --out bracket.step
cargo run --release -p omni-synced-cli -- snapshot examples/bracket.json --out bracket.png
cargo run --release -p omni-synced -- part.step
cargo xtask ci
```

This repository remains based on the upstream SolidCraft codebase and keeps compatibility layers for older SolidCraft scripts and app IDs, but the product identity now reflects OmniSynced.
