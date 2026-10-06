# Boros — Cairn dossier

This tree is maintained under the [Cairn](https://github.com/txpipe) discipline and its layout is framework-defined: derived files are regenerated, never hand-edited; intent lives in the colocated manifests (`WORKSPACE.md`, `src/*/MODULE.md`); decisions live in [`decisions/`](decisions/). `out/` is machine exhaust and never committed.

Boros ("tx omnivore") is a Cardano transaction submission service: it accepts raw transactions over gRPC, queues them with priority and chaining semantics in SQLite, fans them out to Ouroboros tx-submission peers, and tracks their fate on-chain (confirmation, retry, rollback) via UtxoRPC chainsync.

## Reading order

1. [life-map.md](life-map.md) — where the action is: churn × complexity partition, chartering nominations. **Repo dormant since 2025-05, judged "will resume" (human, 2026-08-23).**
2. [topology.md](topology.md) — module graph, instability table, the one root cycle, confirmed layer labels. Rendered crate graph: [crate-graph.svg](crate-graph.svg).
3. The four flows — the tx lifecycle (`Pending → InFlight → Confirmed | Failed`, defined in `src/storage/mod.rs`) is the spine; each flow drives one leg:
   - [flows/submit.md](flows/submit.md) — gRPC → validated → queued (`Pending`).
   - [flows/fanout.md](flows/fanout.md) — priority drain → sign → broadcast (`InFlight`/`Failed`).
   - [flows/confirm.md](flows/confirm.md) — chainsync → `Confirmed`/retry/rollback + cursor. **Flow-contract nominee.**
   - [flows/peer-discovery.md](flows/peer-discovery.md) — relay/peer-sharing pool top-up (relay side mocked — [decision 0003](decisions/0003-real-relay-source-for-peer-discovery.md)).

Reference material, consulted rather than read in order: [shape.md](shape.md) (census: size, tests, unsafe, dependency weight), [api/](api/) (internal public-item surface — bin-only crate, so this is the operational-comprehension snapshot, not a semver surface), [context-map.md](context-map.md) (generated from manifest relationships), [decisions/](decisions/) (ADRs 0001–0004: gasket fork, dependency pinning, relay mock, `Validated` status).
