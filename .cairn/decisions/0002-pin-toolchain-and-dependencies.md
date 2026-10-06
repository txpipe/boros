# 0002 — Pin toolchain and dependencies

**Status:** proposed (finding from the first cairn survey, 2026-08-23; awaiting execution sign-off)

## Context

A clean checkout of `main` does not compile: `pallas = "1.0.0-alpha.2"` is a floating pre-release requirement that now resolves to `pallas 1.1.1`, whose u5c/pparams API differs from what the code was written against — and `Cargo.lock` is gitignored (the `.gitignore` template's library advice, applied to a binary), so nothing pins the resolution. The first gate run only succeeded after a local lockfile downgrade (`cargo update -p pallas --precise 1.0.0-alpha.2`). `rust-version` (MSRV) is also unpinned.

## Decision (proposed)

Commit `Cargo.lock`; pin `pallas = "=1.0.0-alpha.2"` exactly until upgraded deliberately; declare `rust-version` in `Cargo.toml`.

## Consequences

Builds become reproducible from a clean checkout; dependency upgrades become explicit diffs routed through the gate instead of silent re-resolution during dormancy.
