# Boros — Architecture

Boros ("tx omnivore") is a Cardano transaction submission service: it accepts raw transactions over gRPC, queues them with priority and chaining semantics in SQLite, fans them out to Ouroboros tx-submission peers, and tracks their fate on-chain (confirmation, retry, rollback) via UtxoRPC chainsync.

This dossier is maintained under the [Cairn](https://github.com/txpipe) discipline: derived files are regenerated, never hand-edited; intent lives in module manifests (`MODULE.md`) and `WORKSPACE.md`.

## The dossier

- [00 — Shape (census)](00-shape.md) — size, test distribution, unsafe count, dependency weight.
- [01 — Topology](01-topology.md) — module graph, instability table, the one root cycle, confirmed layer labels. Rendered crate graph: [01-crate-graph.svg](01-crate-graph.svg).
- [02 — API surface](02-api/boros-internal-surface.txt) — internal public-item snapshot (bin-only crate; see deviation note inside).
- [03 — Life map](03-life-map.md) — lifetime churn × complexity partition; **repo dormant since 2025-05, judged "will resume" (human, 2026-08-23)**; chartering nominations.
- Flows (curated sequence diagrams, type-labeled):
  - [submit](flows/submit.md) — gRPC → validated → queued.
  - [fanout](flows/fanout.md) — priority drain → sign → broadcast → InFlight.
  - [confirm](flows/confirm.md) — chainsync → Confirmed/retry/rollback + cursor. **Flow-contract nominee.**
  - [peer-discovery](flows/peer-discovery.md) — relay/peer-sharing pool top-up (relay side currently mocked).
- [context-map.md](context-map.md) — generated from manifests (pending charter).

## Reading order for newcomers

Life map → topology → the four flows. The tx lifecycle (`Pending → Validated → InFlight → Confirmed | Failed`, defined in `src/storage/mod.rs`) is the spine: submit produces `Pending`, fanout moves to `InFlight`/`Failed`, monitor settles `Confirmed`/retries. (`Validated` is defined but currently unused by any flow — see survey findings.)
