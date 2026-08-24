# 0004 — Fate of the `Validated` transaction status

**Status:** open (deliberately undecided, 2026-08-23)

## Context

`TransactionStatus::Validated` is defined in `src/storage/mod.rs` and exercised only by a storage test; no production path ever sets it. The live lifecycle is `Pending → InFlight → {Confirmed, Failed}` (plus retry and rollback re-entries). A dead state in the lifecycle enum invites drift.

## Decision

None yet. Options: retire the variant, or wire it as a real stage (decoupling validation from broadcast in the ingest flow).

## Consequences

Until decided, INV-WS-001 documents the lifecycle without `Validated`, and this ADR is the pointer explaining the extra variant.
