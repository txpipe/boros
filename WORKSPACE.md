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
chain (confirm, retry, rollback). See `.cairn/README.md`.

## Invariants

- **INV-WS-001** `[llm-judged]` — All `Transaction.status` mutations flow through the `storage` write API; no module manipulates persisted status by other means. The legal lifecycle is `Pending → InFlight → {Confirmed, Failed}`, plus `InFlight → Pending` (retry) and `Confirmed → InFlight` (rollback). *(`Validated` exists in the enum but no production path uses it — see `.cairn/decisions/0004`.)*

## Non-goals

- Boros does not build or balance transactions; it accepts fully-formed CBOR.
- Boros is not a general mempool: it tracks only transactions it accepted.

## Language

*queue* (named submission lane with weight), *chained queue* (lane serializing
dependent txs via lock tokens), *fanout* (broadcast to peers), *InFlight*
(broadcast, awaiting on-chain observation), *cursor* (last processed chain
point), *tip* (latest known chain point), *u5c* (UtxoRPC chain access).

## Notes

Open workspace-level concerns live as ADRs, not constraints: gasket git-fork
pin (`.cairn/decisions/0001`, accepted), toolchain/dependency pinning — clean
checkouts of `main` currently do not compile (`.cairn/decisions/0002`,
proposed), dead `Validated` status (`.cairn/decisions/0004`, open).
