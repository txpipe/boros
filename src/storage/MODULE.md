+++
schema = "cairn/v0"
id = "storage"
short = "STORE"
role = "generic-subdomain"
purpose-status = "inferred"
parnas-secret = "how transaction/cursor state is persisted — SQLite, the schema, and all SQL live only here"

[deps]
internal-allowed = ["queue#const-only"]
external-policy = "review-new"
verify = "just check-deps storage"
+++

# storage — module manifest

## Purpose

### System contribution

Within Boros, keep the submission state durable so the service can keep
pursuing every accepted transaction and resume following the chain where it
left off. Contributes to the workspace outcomes
[OUT-WS-001, OUT-WS-002 and OUT-WS-003](../../WORKSPACE.md#system-contribution):
accepted transactions are recorded, queues can be drained in order, and
settlement state survives restarts. Storage supplies representation and
queries; it does not decide lifecycle transitions.

### Beneficiaries

| Beneficiary | Connection to outcomes | Provenance |
|---|---|---|
| `server` (boros and UtxoRPC submit services) | Records submitted batches for OUT-WS-001 | Observed caller of `create` |
| `queue` (`priority`, `chaining`) | Reads per-queue counts, ordered selections and the latest submission for OUT-WS-002 | Observed caller of `state`, `next`, `latest` |
| `pipeline` (`ingest`, `monitor`) | Writes status transitions and the cursor for OUT-WS-001 and OUT-WS-003 | Observed caller of `update`, `update_batch`, `find`, `find_to_rollback`, `SqliteCursor` |
| Operators | Need a database that migrates in place and survives restarts | Inferred; durability and retention needs were not stated anywhere |

### Responsibilities

| ID | Outcome | Responsibility | Realization | Coverage |
|---|---|---|---|---|
| RESP-STORE-001 | OUT-WS-001 | Record accepted transactions with their queue, status and declared dependencies | INV-STORE-001. Batch atomicity of `create` is observed (one database transaction) but not declared; duplicate submissions are unassessed. | partial |
| RESP-STORE-002 | OUT-WS-002, OUT-WS-003 | Answer lifecycle queries: per-queue counts and ordered selection, the in-flight set, rollback candidates, the latest submission per queue | INV-STORE-002, INV-STORE-003. `find`, `state` and `latest` have tests but no declared guarantee. | partial |
| RESP-STORE-003 | OUT-WS-003 | Persist status transitions written by callers | `update` and `update_batch` exist; no guarantee declared. Transition legality is the workspace's INV-WS-001, not checked here. | uncovered |
| RESP-STORE-004 | OUT-WS-003 | Persist the chain-follow cursor | INV-STORE-004. Atomicity with the status updates of the same chain event is unallocated. | partial |
| RESP-STORE-005 | OUT-WS-001, OUT-WS-003 | Own the schema, its migrations and the confinement of SQL | INV-STORE-005, INV-STORE-006 | partial |

### Owned information

| Information | Authority and representation ownership |
|---|---|
| Transaction records (id = transaction hash hex, raw CBOR, status, queue, slot, timestamps) | Owns the `tx` table representation; the meaning of `status` belongs to the workspace lifecycle (INV-WS-001) |
| Dependency edges | Owns `tx_dependence`; written by `create`, never read back (`Transaction.dependencies` loads as `None`) |
| Chain-follow cursor | Owns the single-row `cursor` table; `pipeline` decides when it advances |
| Schema | Authoritative: `src/storage/migrations/` |

### Boundary allocation

| Responsibility | This module | Collaborator obligation and owner | Contract gap |
|---|---|---|---|
| RESP-STORE-001 | Insert the batch and its dependency edges | `server` decodes, validates and assigns queues before calling `create` | `server` has no manifest; no expectation declared |
| RESP-STORE-001, RESP-STORE-002 | Record dependency edges | Dispatch in dependency order: `unallocated` (`next` does not consult `tx_dependence`, and no other module does) | Whether dependencies are meant to order dispatch is unknown |
| RESP-STORE-002 | Execute selections with caller-supplied limits | `queue` computes per-queue limits | No expectation declared |
| RESP-STORE-003, RESP-STORE-004 | Apply writes as given | `pipeline` chooses legal transitions and writes the cursor after its status updates | Flow claim INV-CONFIRM-004 is unverified |
| RESP-STORE-005 | Run migrations when asked | The binary entry point (`main`) migrates before serving | No manifest owns `main`; see INV-STORE-006 |

### Operating envelope

| Responsibility | Mode / condition | Scope | Scenarios / gaps |
|---|---|---|---|
| RESP-STORE-001 | Submission batch with valid dependencies | in-scope | [submit flow](../../.cairn/flows/submit.md) |
| RESP-STORE-001 | Resubmission of a transaction already stored | unknown | The primary key rejects the whole batch; intended behavior undecided |
| RESP-STORE-002 | Draining pending transactions under load | in-scope | [fanout flow](../../.cairn/flows/fanout.md) |
| RESP-STORE-002, RESP-STORE-003 | Concurrent writers (`server`, `ingest`, `monitor`) on one pool | unknown | No isolation guarantee declared |
| RESP-STORE-003, RESP-STORE-004 | Crash between status updates and cursor write | unknown | [confirm flow](../../.cairn/flows/confirm.md), INV-CONFIRM-004 unverified |
| RESP-STORE-005 | Migrating an existing database | in-scope | No scenario or test beyond the in-memory database |
| RESP-STORE-001, RESP-STORE-003 | Retention of settled transactions | unknown | No deletion path exists; growth is unbounded |

## Guarantees

- **INV-STORE-001** `[bound → just test-storage]` — Creating transactions with declared dependencies fails unless every dependency is already present (`it_should_fail_create_with_invalid_dependencies`).
  - Surface: `SqliteTransaction::create`.
  - When: The database is migrated; a transaction in the batch declares `dependencies`.
  - Then: `create` returns `Err` when a declared dependency id is not stored.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-STORE-002** `[bound → just test-storage]` — `next(status, quotas)` honors per-queue quotas and status filtering; transactions from queues no longer in config still drain (`it_should_find_next*`).
  - Surface: `SqliteTransaction::next`.
  - When: The caller supplies a status and a per-queue limit map.
  - Then: The result holds only transactions in that status, at most the limit per listed queue, oldest first within a queue.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-STORE-003** `[bound → just test-storage]` — `find_to_rollback(slot)` returns only transactions whose recorded slot is affected by a rollback to `slot` (`it_should_find_to_rollback*`).
  - Surface: `SqliteTransaction::find_to_rollback`.
  - When: The caller supplies the rollback slot.
  - Then: The result holds exactly the `Confirmed` transactions whose slot is after it.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-STORE-004** `[bound → just test-storage]` — Cursor `set` upserts: a second write updates rather than duplicates (`it_should_set_when_it_updates`).
  - Surface: `SqliteCursor::{set, current}`.
  - When: The database is migrated.
  - Then: After `set` returns `Ok`, `current` returns that cursor; there is never more than one.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.

## Invariants

- **INV-STORE-005** `[llm-judged]` — No SQL and no `sqlx` usage outside this module; consumers speak `Transaction`/`Cursor`, never rows.
  - Scope and observation: the workspace source, at every revision.
- **INV-STORE-006** `[unverified]` — Migrations run to completion before any query is served (currently upheld only by call order in `main.rs`; nothing enforces it).
  - Scope and observation: process lifetime, from startup to the first query.

## Relationships

- *(supplier)* `open-host` — storage is the shared persistence supplier for `queue`, `pipeline`, and `server`; its store types are the published surface.

## Non-goals

- No domain policy: which tx goes next (queue), when to retry (pipeline) are not storage's decisions.
