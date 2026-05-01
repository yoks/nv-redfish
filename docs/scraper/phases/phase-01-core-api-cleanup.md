# Scraper phase 1: core API cleanup

## Purpose

Finish small API refinements before behavior implementation starts. Keep the
already-green API, feature-gating, and compile-fail tests green.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Implementation phases](../implementation-phases.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not rewrite tests, weaken assertions, ignore
behavior tests, or update trybuild `.stderr` files just to make this phase pass.
Change tests only if a requirement changes or a test is demonstrably wrong.

## Frozen tests to preserve

- [api_bounds.rs](../../../scraper/tests/api_bounds.rs)
- [feature_gating.rs](../../../scraper/tests/feature_gating.rs)
- [default_no_redfish_adapter.rs](../../../scraper/tests/trybuild/default_no_redfish_adapter.rs)
- [default_no_runtime_event.rs](../../../scraper/tests/trybuild/default_no_runtime_event.rs)
- [no_detached_redfish_command.rs](../../../scraper/tests/trybuild/no_detached_redfish_command.rs)

## Scope

- Keep the public runtime API generic over `E` and `Err`.
- Keep ids opaque and Redfish-independent.
- Keep `RuntimeEvent` hidden without the `runtime-events` feature.
- Keep the Redfish adapter hidden without the `redfish-adapter` feature.
- Remove or tighten accidental placeholder API only when no frozen test or
  requirement depends on it.

## Target commands

```sh
cargo check -p nv-redfish-scraper
cargo clippy -p nv-redfish-scraper
cargo test -p nv-redfish-scraper --test api_bounds
cargo test -p nv-redfish-scraper --test feature_gating
```

## Done

- The target commands pass.
- No behavior implementation is hidden in this phase.
- No Redfish dependency leaks into runtime-only builds.
