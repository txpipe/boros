+++
schema = "cairn/v0"
id = "boros"

[workspace]
verify-all = "just gate"

[process]
mode = "report-only"
auto-accept = { max-radius = 2, hotspot-overlap = false, api-delta = false }
sample-rate = 0.15
mandatory = ["hotspot-overlap", "unsafe-touched", "tx-status-semantics-touched"]
+++

# Boros — workspace manifest

## Purpose

Boros is a Cardano transaction submission service ("tx omnivore"): accept raw
transactions over gRPC, queue them with priority and chaining semantics, fan
them out to Ouroboros tx-submission peers, and settle their status against the
chain (confirm, retry, rollback). See `docs/architecture/ARCHITECTURE.md`.

## Invariants

- **INV-WS-001** `[llm-judged]` — All `Transaction.status` mutations flow through the `storage` write API; no module manipulates persisted status by other means. The legal lifecycle is `Pending → InFlight → {Confirmed, Failed}`, plus `InFlight → Pending` (retry) and `Confirmed → InFlight` (rollback). *(`Validated` exists in the enum but no production path uses it — see REQ-WS-003.)*

## Requirements

- **REQ-WS-001** `[unverified]` — TODO(human): pin a `rust-version` (MSRV) in `Cargo.toml`; currently unpinned.
- **REQ-WS-002** `[unverified]` — Return `gasket` to a published upstream/crates.io release; the `construkts/gasket-rs` git fork is a stopgap. *(Decided by human, 2026-08-23.)*
- **REQ-WS-003** `[unverified]` — TODO(human): retire or wire the `Validated` transaction status; a dead state in the lifecycle enum invites drift.
- **REQ-WS-004** `[unverified]` — Commit `Cargo.lock` (this is a binary; the current `.gitignore` excludes it) and pin `pallas` exactly (`=1.0.0-alpha.2`) until upgraded deliberately. **Found 2026-08-23: a clean checkout of `main` does not compile** — the floating `1.0.0-alpha.2` requirement resolves to `pallas 1.1.1`, whose u5c/pparams API differs. The gate only ran after a local lockfile downgrade.

## Non-goals

- Boros does not build or balance transactions; it accepts fully-formed CBOR.
- Boros is not a general mempool: it tracks only transactions it accepted.

## Language

*queue* (named submission lane with weight), *chained queue* (lane serializing
dependent txs via lock tokens), *fanout* (broadcast to peers), *InFlight*
(broadcast, awaiting on-chain observation), *cursor* (last processed chain
point), *tip* (latest known chain point), *u5c* (UtxoRPC chain access).
