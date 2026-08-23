+++
schema = "cairn/v0"
id = "pipeline"
role = "core-domain"
parnas-secret = "the runtime topology — which gasket stages exist, how they are scheduled, and how backpressure (channel cap) is applied"

[deps]
internal-allowed = ["ledger", "network", "queue", "signing", "storage", "validation"]
external-policy = "review-new"
verify = "just check-deps pipeline"
+++

# pipeline — module manifest

## Purpose

The orchestrator: three gasket stages wired in `run()` — `ingest` (drain
Pending by priority → sign → validate → broadcast → InFlight), `monitor`
(chainsync → Confirmed / retry / rollback + cursor), `peer_discovery` (peer
pool top-up). Temporal behavior across these stages is specified by the flow
contracts: `fanout`, `confirm`, `peer-discovery` (see `docs/architecture/flows/`).

## Invariants

- **INV-PIPE-001** `[bound → just test-ingest]` — Phase-1 validation and phase-2 evaluation accept known-valid Conway transactions and reject invalid ones (fixture-backed, `ingest_tests`).
- **INV-PIPE-002** `[llm-judged]` — Stage failures are contained by gasket retry policy; a failing transaction is marked `Failed` rather than wedging the stage (refines INV-WS-001).
- **INV-PIPE-003** `[unverified]` — Temporal invariants of this module's stages are owned by the flow docs (INV-FANOUT-*, INV-CONFIRM-*, INV-DISCOVERY-*); this entry records that they are not yet verified anywhere.

## Requirements

- **REQ-PIPE-001** `[unverified]` — `run()` wires `MockRelayDataAdapter` into production peer discovery (`src/pipeline/mod.rs:33`); this is a known gap that must be replaced with a real on-chain relay source. *(Decided by human, 2026-08-23.)*
- **REQ-PIPE-002** `[unverified]` — TODO(human): `pipeline` imports `Config` from the crate root (the `main ⇄ pipeline` cycle in `01-topology.md`); consider moving shared config types out of `main.rs`.

## Relationships

- `customer-supplier` with **storage**, **queue**, **signing** (pipeline is the customer).
- `conformist` toward **ledger** (u5c): pipeline consumes the UtxoRPC event/point vocabulary as-is, no translation layer.
- `customer-supplier` with **network** (pipeline feeds the broadcast channel network peers consume).

## Non-goals

- No policy decisions: what to pick (queue), what is valid (validation), how to persist (storage) are supplied, not owned.
