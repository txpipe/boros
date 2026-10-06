+++
schema = "cairn/v0"
id = "boros"
short = "WS"
purpose-status = "inferred"

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

### System contribution

Boros is a Cardano transaction submission service ("tx omnivore"): accept raw
transactions over gRPC, queue them with priority and chaining semantics, fan
them out to Ouroboros tx-submission peers, and settle their status against the
chain (confirm, retry, rollback). See `.cairn/README.md`.

- OUT-WS-001: A submitter that hands Boros a fully-formed transaction has it delivered to the Cardano network without running its own node connections.
- OUT-WS-002: Submitters control the relative order of their submissions: weighted queues share dispatch capacity, and chained queues serialize dependent transactions.
- OUT-WS-003: An accepted transaction is pursued until it settles on-chain: rebroadcast while not included, confirmed when included, reopened when a rollback removes it.

Whether submitters must be able to observe settlement is unknown: the UtxoRPC
`WaitForTx` and `WatchMempool` methods are unimplemented, and no status query
exists.

### Beneficiaries

| Beneficiary | Connection to outcomes | Provenance |
|---|---|---|
| Clients of the `boros.v1` submit service | Submit transactions (OUT-WS-001) and use queues and chain locks (OUT-WS-002) | Observed service surface; actual clients (dApp backends, wallets) are inferred |
| Clients of the UtxoRPC submit service | Submit transactions to the default queue (OUT-WS-001) | Observed service surface |
| Operators | Configure queues, peers, the UtxoRPC endpoint and Vault signing; run the service | Observed config schema; operational needs are unknown |
| Cardano network peers | Receive transactions over tx-submission | Observed protocol use |

### Responsibilities

| ID | Outcome | Responsibility | Realization | Coverage |
|---|---|---|---|---|
| RESP-WS-001 | OUT-WS-001 | Accept, decode, optionally validate and durably queue submitted transactions | `server` (no manifest; flow claims INV-SUBMIT-001..003 unverified); storage: RESP-STORE-001 | partial |
| RESP-WS-002 | OUT-WS-002 | Order dispatch across weighted queues and serialize chained queues | queue: RESP-QUEUE-001, RESP-QUEUE-002, RESP-QUEUE-003; pipeline: RESP-PIPE-001 | partial |
| RESP-WS-003 | OUT-WS-001 | Sign for server-signing queues, re-validate and broadcast to peers | pipeline: RESP-PIPE-001, RESP-PIPE-003; `signing`, `validation`, `network` have no manifests | partial |
| RESP-WS-004 | OUT-WS-003 | Settle transactions against the chain and resume after restart | pipeline: RESP-PIPE-002; storage: RESP-STORE-003, RESP-STORE-004 | uncovered |
| RESP-WS-005 | OUT-WS-001, OUT-WS-003 | Own the transaction lifecycle | INV-WS-001; decision 0004 leaves `Validated` open | partial |

### Owned information

| Information | Authority and representation ownership |
|---|---|
| Transaction lifecycle (status meaning and legal transitions) | Authoritative here (INV-WS-001); `storage` owns the representation |
| Service configuration (`boros.toml`, `/etc/boros/config.toml`, `BOROS_*` environment) | Root `Config` in `main.rs`; each module owns its section's schema |

### Boundary allocation

| Responsibility | This module | Collaborator obligation and owner | Contract gap |
|---|---|---|---|
| RESP-WS-001, RESP-WS-003 | Validate against current ledger state | A UtxoRPC endpoint (external, operator-chosen) supplies protocol parameters, UTxOs, tip and chain events | No workspace adapter manifest for `ledger` |
| RESP-WS-003 | Sign for server-signing queues | Hashicorp Vault (external) holds keys | No adapter manifest for `signing` |
| RESP-WS-003 | Broadcast | Cardano peers (external) accept tx-submission; a relay source supplies candidates: `unallocated` (mocked, decision 0003) | No manifest for `network` |
| RESP-WS-001 | Accept fully-formed transactions | Submitters build, balance and (except on server-signing queues) sign them | Stated as a non-goal |

### Operating envelope

| Responsibility | Mode / condition | Scope | Scenarios / gaps |
|---|---|---|---|
| RESP-WS-001 | Submission over either gRPC service | in-scope | [submit flow](.cairn/flows/submit.md) |
| RESP-WS-002, RESP-WS-003 | Steady-state dispatch | in-scope | [fanout flow](.cairn/flows/fanout.md) |
| RESP-WS-004 | Chain advance, retry, rollback | in-scope | [confirm flow](.cairn/flows/confirm.md) |
| RESP-WS-004 | Restart or crash recovery | in-scope | Cursor resume is observed; INV-CONFIRM-004 is unverified |
| RESP-WS-003 | Peer discovery from real relays | unknown | Mocked (decision 0003) |
| RESP-WS-001, RESP-WS-004 | UtxoRPC endpoint, Vault or peers unavailable | unknown | No accepted degraded behavior |
| RESP-WS-002 | Several Boros instances sharing one database | unknown | Chain locks are per process |
| RESP-WS-005 | `Validated` status | unknown | Decision 0004 |
| RESP-WS-001 | Building or balancing transactions | excluded | [Non-goals](#non-goals); owned by submitters |

## Composition

| Child | Manifest | Allocation | Exposed surface | Integration |
|---|---|---|---|---|
| pipeline | [pipeline](src/pipeline/MODULE.md) | RESP-WS-002 → RESP-PIPE-001; RESP-WS-003 → RESP-PIPE-001, RESP-PIPE-003; RESP-WS-004 → RESP-PIPE-002 | `pipeline::run` (internal) | Runs beside `server` in `main`; reads and writes through `storage`, schedules through `queue` |
| storage | [storage](src/storage/MODULE.md) | RESP-WS-001 → RESP-STORE-001; RESP-WS-004 → RESP-STORE-003, RESP-STORE-004 | `storage::{Transaction, Cursor}`, `storage::sqlite::{SqliteStorage, SqliteTransaction, SqliteCursor}` (internal) | One SQLite pool shared by all writers; no cross-module transaction |
| queue | [queue](src/queue/MODULE.md) | RESP-WS-002 → RESP-QUEUE-001, RESP-QUEUE-002, RESP-QUEUE-003 | `queue::{Config, priority::Priority, chaining::TxChaining}` (internal) | Shared by `server` and `pipeline`; lock state lives in one process |

`server`, `network`, `ledger`, `signing`, `validation` and `main` have no
manifests yet; their work stays under this manifest and appears above as gaps.
No joint transaction spans the children.

## Invariants

- **INV-WS-001** `[llm-judged]` — All `Transaction.status` mutations flow through the `storage` write API; no module manipulates persisted status by other means. The legal lifecycle is `Pending → InFlight → {Confirmed, Failed}`, plus `InFlight → Pending` (retry) and `Confirmed → InFlight` (rollback). *(`Validated` exists in the enum but no production path uses it — see `.cairn/decisions/0004`.)*
  - Scope and observation: every write of a persisted transaction status, across the workspace.

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
