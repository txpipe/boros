# 01 — Topology

<!-- GENERATED 2026-08-23 by cairn-survey @ fdec4da via cargo-depgraph + cargo-modules. Edge list and metrics are derived; layer labels are curated intent (pending human confirmation). -->

> **Deviation note:** SPEC §2 expects crate-level topology. Boros is a single bin crate + one generated `spec` crate, so the crate graph (`01-crate-graph.svg`) is trivial. This file carries the *module-level* topology, which is where the structure actually lives.

## Generated: module dependency edges

Top-level module edges (submodules collapsed into parents; derived from `cargo modules dependencies`):

```text
main    → ledger, network, pipeline, queue, server, storage
pipeline → main, ledger, network, queue, signing, storage, validation
server  → main, ledger, queue, storage, validation
queue   → storage
validation → ledger
ledger, network, signing, storage → (no internal deps)
```

## Generated: instability table (module level)

Ca = afferent (modules depending on it), Ce = efferent (internal modules it depends on), I = Ce/(Ca+Ce).

| Module | Ca | Ce | I | Reading |
|---|---:|---:|---:|---|
| `ledger` | 4 | 0 | 0.00 | maximally stable |
| `storage` | 4 | 0 | 0.00 | maximally stable |
| `network` | 2 | 0 | 0.00 | stable |
| `signing` | 1 | 0 | 0.00 | stable |
| `queue` | 3 | 1 | 0.25 | stable-ish |
| `validation` | 2 | 1 | 0.33 | stable-ish |
| `main` (root) | 2 | 6 | 0.75 | orchestrator — **but Ca should be 0** |
| `server` | 1 | 5 | 0.83 | orchestrator |
| `pipeline` | 1 | 7 | 0.88 | orchestrator |

## Generated: cycle detection

One cycle at module level, through the crate root:

- `main → pipeline → main` (and `main → server → main`): `Config` is defined in `main.rs` (`src/main.rs:72`) and imported by `pipeline::ingest` (`src/pipeline/ingest.rs:12`). The composition root is also a shared-type supplier.

No cycles among the eight domain modules per `cargo-modules` — the domain graph is a clean DAG. **Tooling caveat:** `cargo-modules` misses const-only uses; source text shows `storage/mod.rs:6` importing `queue::DEFAULT_QUEUE`, making `storage ⇄ queue` a weak two-way coupling (queue → storage is the load-bearing direction; storage → queue is one default-value const).

## Curated: layer labels (confirmed by human, 2026-08-23)

| Layer | Modules | Intent |
|---|---|---|
| **Orchestration** | `main`, `pipeline`, `server` | composition root; gasket stage graph; gRPC API |
| **Domain** | `queue`, `validation` | tx queueing policy (priority, chaining); tx validation |
| **Foundation / adapters** | `ledger`, `network`, `storage`, `signing` | chain access (u5c/relay), Ouroboros peers, SQLite, key/Vault signing |
