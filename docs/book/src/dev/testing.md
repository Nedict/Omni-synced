# Test harnesses

`cargo xtask ci` runs everything a pull request must pass: fmt, `xtask assets`,
`xtask licences --check`, `xtask layers`, clippy with `-D warnings`, the tests with the `ci`
profile and the wasm build check.

| Harness | Where | What it catches |
|---|---|---|
| Unit and engine tests | next to the code, `*_tests.rs` | measured results: volume, area, topology counts against derived values |
| Hostile input | `hostile_params_never_panic`, `crates/engine/src/tests.rs` | every command × every documented parameter × junk values: errors, never panics |
| Sketch fuzz | `crates/engine/src/cmd/sketch_fuzz_tests.rs` | random sketches through the solver and profile finder |
| UI scenarios | `crates/ui-egui/tests/scenarios/*.json` | the real app, headless: clicks, keys, drags, assertions |
| Layout tests | `crates/ui-egui/tests/ui_layout.rs`, `window_widths.rs` | panels and dialogs at every window width |
| Design-file corpus | `apps/solvecraft-cli/tests/corpus` | every saved file version still opens and evaluates the same |
| Kill mid-save | `apps/solvecraft-cli/tests/kill_mid_save.rs` | a save killed at any moment leaves a readable file |
| Reference tests | `cargo xtask oracle` → [docs/oracle.md](oracle.md) | parts rebuilt from `tests/oracle/*/recipe.json`, compared with analytic or FreeCAD expected values |

## UI scenarios

A scenario is a JSON list of steps run against the real app with a headless egui context:

```json
[
  {"start": "sketch.create"},
  {"click": {"plane": "XY"}},
  {"start": "sketch.rectangle.two_point"},
  {"click": {"sketch": [0, 0]}},
  {"click": {"sketch": [40, 30]}},
  {"expect": {"errors": 0}}
]
```

A new file in `crates/ui-egui/tests/scenarios/` is picked up automatically. To record a bug
before it's fixed, add a `{"pending": "why"}` step; its test is then ignored with that reason, and
the fix removes the step.

## CI profile

CI runs the tests with `--profile ci`: light optimisation, line tables, no LTO and debug
assertions off. Some truck debug assertions fail on valid geometry; the `ci` profile disables
them while building faster than `--release`. If a test fails only in a plain debug build, say
so in the PR.

Run `cargo xtask test` to build with this profile and run the test binaries concurrently,
with doc tests alongside. Arguments after `--` go to each test binary, for example
`cargo xtask test -- shell`. To run the test binaries sequentially instead, use
`cargo test --workspace --profile ci`.
