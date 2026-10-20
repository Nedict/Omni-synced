<p align="center">
  <img src="assets/icons/app/linux/exports/hicolor/256x256/apps/ai.storyteller.solidcraft.png" width="96" height="96" alt="OmniSynced" />
</p>

<h1 align="center">OmniSynced</h1>

<p align="center">
  <strong>Parametric 3D CAD for your synced design workflows, built in pure Rust.</strong><br />
  Sketch, constrain, and build parts with a parametric feature timeline. Change a dimension and watch
  your designs rebuild. Run it on your desktop or through the built-in AI agent tooling.
</p>

<div align="center">

[![Download for Windows][badge-windows]][windows-msi]
[![Download for macOS][badge-macos]][macos-dmg]
[![Download for Linux][badge-linux]][linux-appimage]
[![Get it from the Snap Store][badge-snap]][snap-store]
[![Try it in your browser][badge-web]][web-app]

</div>

<p align="center">
  <a href="https://github.com/Nedict/Omni-synced">GitHub</a> ·
  <a href="https://docs.solidcraft.io/">Documentation</a> ·
  <a href="https://github.com/bherbruck/solidcraft">Base project</a>
</p>

<p align="center">
  <a href="https://github.com/Nedict/Omni-synced/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/Nedict/Omni-synced/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://github.com/Nedict/Omni-synced/releases/latest"><img src="https://img.shields.io/github/v/release/Nedict/Omni-synced?label=release" alt="Latest release" /></a>
  <a href="https://github.com/Nedict/Omni-synced/releases"><img src="https://img.shields.io/github/downloads/Nedict/Omni-synced/total?label=downloads" alt="Downloads" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/Nedict/Omni-synced" alt="Licence" /></a>
</p>

**OmniSynced** is a personalized fork of the SolidCraft CAD engine and UI, adapted for my own workflows and tooling. It keeps the same modeling core while presenting itself as a custom-branded project.

## Build and run

```sh
cargo run --release -p omni-synced -- --sample
cargo run --release -p omni-synced-cli -- commands
cargo run --release -p omni-synced-cli -- run examples/bracket.json --out bracket.step
cargo run --release -p omni-synced-cli -- snapshot examples/bracket.json --out bracket.png
cargo run --release -p omni-synced -- part.step
cargo xtask ci
```

This project remains built on top of the upstream SolidCraft codebase, but the app and CLI branding now reflect OmniSynced.
