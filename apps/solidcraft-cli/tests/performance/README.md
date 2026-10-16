# Project-opening performance fixture

`HardEdgeBasin.solidcraft` is the native SolidCraft design supplied by the project
owner for this regression. It contains 58 features and produces 14 bodies. It is
kept separately from the historical format corpus: that corpus deliberately
mutates every fixture hundreds of times, which would multiply this benchmark's
expensive geometry rebuilds.

Run `cargo test --profile ci -p solidcraft-cli --test open_performance -- --nocapture`.
The test covers `doc.open`, saved/path state, the complete timeline, world-space
display meshes, and cache reuse on refresh. A child-process deadline bounds a
hang as well as a slowdown. Build time is outside the deadline.

Upstream `57e6396` supports the left/right floor fillets (IDs 102–103). Six
nonplanar-end fillets remain unsupported (IDs 104–109). The test pins those
failures explicitly; as support lands, update the exact failures and remeasure
the budget. Do not speed up the test by suppressing features or ignoring
additional errors.

For per-feature timings use `solidcraft-cli bench-open` with this fixture.
This is not a Fusion archive; the supplied F3D files remain outside the repository.
