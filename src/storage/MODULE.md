+++
schema = "cairn/v0"
id = "storage"
role = "generic-subdomain"
parnas-secret = "how transaction/cursor state is persisted — SQLite, the schema, and all SQL live only here"

[deps]
internal-allowed = ["queue#const-only"]
external-policy = "review-new"
verify = "just check-deps storage"
+++

# storage — module manifest

## Purpose

Persistence for the two durable facts Boros owns: the transaction queue
(`Transaction`, `TransactionStatus`) and the chainsync `Cursor`. Exposes typed
stores (`SqliteTransaction`, `SqliteCursor`) over a migrated SQLite database.

## Invariants

- **INV-STORE-001** `[bound → just test-storage]` — Creating transactions with declared dependencies fails unless every dependency is already present (`it_should_fail_create_with_invalid_dependencies`).
- **INV-STORE-002** `[bound → just test-storage]` — `next(status, quotas)` honors per-queue quotas and status filtering; transactions from queues no longer in config still drain (`it_should_find_next*`).
- **INV-STORE-003** `[bound → just test-storage]` — `find_to_rollback(slot)` returns only transactions whose recorded slot is affected by a rollback to `slot` (`it_should_find_to_rollback*`).
- **INV-STORE-004** `[bound → just test-storage]` — Cursor `set` upserts: a second write updates rather than duplicates (`it_should_set_when_it_updates`).
- **INV-STORE-005** `[llm-judged]` — No SQL and no `sqlx` usage outside this module; consumers speak `Transaction`/`Cursor`, never rows.

## Requirements

- **REQ-STORE-001** `[unverified]` — Migrations run to completion before any query is served (currently by call order in `main.rs`; nothing enforces it).

## Relationships

- *(supplier)* `open-host` — storage is the shared persistence supplier for `queue`, `pipeline`, and `server`; its store types are the published surface.

## Non-goals

- No domain policy: which tx goes next (queue), when to retry (pipeline) are not storage's decisions.
