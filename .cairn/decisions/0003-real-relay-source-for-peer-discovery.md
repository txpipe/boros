# 0003 — Peer discovery requires a real relay source

**Status:** accepted (human decision, 2026-08-23)

## Context

Production wiring injects `MockRelayDataAdapter` into the peer-discovery stage (`src/pipeline/mod.rs:33`); the on-chain half of peer discovery is fake, leaving only config-listed peers and peer-sharing.

## Decision

This is a known gap, not intended design: a real on-chain relay data source must replace the mock before Boros is production-ready.

## Consequences

The `peer-discovery` flow doc notes the gap; the `ledger/relay` adapter trait is the implementation seam. Closing this decision removes the mock from `pipeline::run()`.
