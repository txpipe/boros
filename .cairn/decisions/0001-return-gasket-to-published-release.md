# 0001 — Return gasket to a published release

**Status:** accepted (human decision, 2026-08-23)

## Context

`Cargo.toml` pins `gasket` to a personal git fork (`construkts/gasket-rs`), a supply-chain and bus-factor liability surfaced by the census (`.cairn/shape.md`).

## Decision

The fork is a stopgap. Boros returns to a published upstream/crates.io `gasket` release as soon as one carries what the fork provides.

## Consequences

Until executed, the fork pin is visible debt: `external-policy = "review-new"` plus a future `cargo deny` sources check (SPEC §7.1) will keep flagging it. Executing this decision closes that finding.
