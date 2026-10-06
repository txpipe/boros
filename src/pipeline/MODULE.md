+++
schema = "cairn/v0"
id = "pipeline"
short = "PIPE"
role = "core-domain"
purpose-status = "inferred"
parnas-secret = "the runtime topology — which gasket stages exist, how they are scheduled, and how backpressure (channel cap) is applied"

[deps]
internal-allowed = ["ledger", "network", "queue", "signing", "storage", "validation"]
external-policy = "review-new"
verify = "just check-deps pipeline"
+++

# pipeline — module manifest

## Purpose

### System contribution

The orchestrator: three gasket stages wired in `run()` — `ingest` (drain
Pending by priority → sign → validate → broadcast → InFlight), `monitor`
(chainsync → Confirmed / retry / rollback + cursor), `peer_discovery` (peer
pool top-up). Contributes to the workspace outcomes
[OUT-WS-001 and OUT-WS-003](../../WORKSPACE.md#system-contribution): queued
transactions reach the network and are pursued until settled. Temporal
behavior across these stages is specified by the flow docs `fanout`,
`confirm` and `peer-discovery` (see `.cairn/flows/`).

### Beneficiaries

| Beneficiary | Connection to outcomes | Provenance |
|---|---|---|
| `main` | Starts the pipeline beside the gRPC server | Observed caller of `run` |
| Submitters | Their queued transactions are broadcast and settled (OUT-WS-001, OUT-WS-003) | Inferred indirect beneficiaries |
| Cardano peers | Receive transactions over Ouroboros tx-submission | Observed through `network`; peer-side needs are unknown |
| Operators | Configure peers, retry window and signing | Observed config; operational needs (metrics, health) are unknown |

### Responsibilities

| ID | Outcome | Responsibility | Realization | Coverage |
|---|---|---|---|---|
| RESP-PIPE-001 | OUT-WS-001 | Drain pending transactions, sign those on server-signing queues, re-validate, broadcast, and mark each `InFlight` or `Failed` | INV-PIPE-001, INV-PIPE-002; flow claims INV-FANOUT-001..003 are unverified | partial |
| RESP-PIPE-002 | OUT-WS-003 | Follow the chain and settle in-flight transactions: confirm, retry, revert on rollback, persist the cursor | No module guarantee; flow claims INV-CONFIRM-001..004 are unverified | uncovered |
| RESP-PIPE-003 | OUT-WS-001 | Keep the tx-submission peer pool topped up | Flow claim INV-DISCOVERY-001 is unverified; the relay source is mocked (decision 0003) | uncovered |
| RESP-PIPE-004 | OUT-WS-001, OUT-WS-003 | Own the runtime topology: stage wiring, scheduling, failure policy and backpressure | INV-PIPE-002, INV-PIPE-003; INV-FANOUT-003 is unverified | partial |

### Owned information

| Information | Authority and representation ownership |
|---|---|
| Stage topology and the broadcast channel cap (50) | Authoritative, in code |
| Monitor retry window (`retry_slot_diff`) | Owns the config schema; operators supply the value |
| Transaction status and cursor values | Decides the transitions it writes; `storage` owns their representation and the workspace owns the lifecycle (INV-WS-001) |

### Boundary allocation

| Responsibility | This module | Collaborator obligation and owner | Contract gap |
|---|---|---|---|
| RESP-PIPE-001 | Choose, sign, validate and dispatch | `queue` picks the batch; `signing` signs (Hashicorp Vault, external); `validation` judges against ledger state from `ledger` (UtxoRPC endpoint, external) | `signing`, `validation`, `ledger` have no manifests; no expectations declared |
| RESP-PIPE-001 | Put the transaction on the broadcast channel | `network` delivers it to connected peers | Delivery is not acknowledged back; `InFlight` means sent to the channel |
| RESP-PIPE-002 | Map chain events to status updates and the cursor | `ledger::u5c` supplies roll-forward and rollback events in order from the cursor; `storage` persists | Ordering and gap-freedom of the event stream are not declared |
| RESP-PIPE-003 | Schedule pool top-up | `network::PeerManager` connects; a relay source supplies candidates: `unallocated` (mock in use) | Decision 0003 |

### Operating envelope

| Responsibility | Mode / condition | Scope | Scenarios / gaps |
|---|---|---|---|
| RESP-PIPE-001 | Normal drain and broadcast | in-scope | [fanout flow](../../.cairn/flows/fanout.md) |
| RESP-PIPE-001 | Transaction fails validation or evaluation | in-scope | Marked `Failed` (INV-PIPE-002) |
| RESP-PIPE-001 | Transaction that does not decode, or whose queue is missing from config | unknown | The stage errors or retries the whole batch; whether that wedges the stage is unassessed |
| RESP-PIPE-001 | Server-signing queue with no signer configured | in-scope | Retry, never unsigned broadcast (INV-FANOUT-002, unverified) |
| RESP-PIPE-002 | Roll-forward, retry timeout, rollback | in-scope | [confirm flow](../../.cairn/flows/confirm.md); flow-contract nominee |
| RESP-PIPE-002 | Crash between status updates and cursor write | in-scope | INV-CONFIRM-004, unverified |
| RESP-PIPE-003 | Peer pool below target | in-scope | [peer-discovery flow](../../.cairn/flows/peer-discovery.md); relay side mocked |
| RESP-PIPE-004 | UtxoRPC endpoint or all peers unavailable | unknown | Gasket retry policy is the default; no accepted behavior |

## Guarantees

- **INV-PIPE-001** `[bound → just test-ingest]` — Phase-1 validation and phase-2 evaluation accept known-valid Conway transactions and reject invalid ones (fixture-backed, `ingest_tests`).
  - Surface: `validation::{validate_tx, evaluate_tx}` as called by the `ingest` stage.
  - When: Ledger state comes from the test UtxoRPC mock; fixtures are Conway transactions.
  - Then: The valid fixture passes both; the unwitnessed fixture fails validation; the bad-script fixture fails evaluation.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged. The functions live in `validation`, which has no manifest.
- **INV-PIPE-002** `[llm-judged]` — Stage failures are contained by gasket retry policy; a failing transaction is marked `Failed` rather than wedging the stage (refines INV-WS-001).
  - Surface: the `ingest` stage.
  - When: A transaction fails validation or evaluation.
  - Then: It is written `Failed` and the stage continues with the next transaction.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.

## Invariants

- **INV-PIPE-003** `[unverified]` — Temporal invariants of this module's stages are owned by the flow docs (INV-FANOUT-*, INV-CONFIRM-*, INV-DISCOVERY-*); this entry records that they are not yet verified anywhere.
  - Scope and observation: every execution of the three stages.

## Relationships

- `customer-supplier` with **storage**, **queue**, **signing** (pipeline is the customer).
- `conformist` toward **ledger** (u5c): pipeline consumes the UtxoRPC event/point vocabulary as-is, no translation layer.
- `customer-supplier` with **network** (pipeline feeds the broadcast channel network peers consume).

## Non-goals

- No policy decisions: what to pick (queue), what is valid (validation), how to persist (storage) are supplied, not owned.

## Notes

`run()` currently wires `MockRelayDataAdapter` into peer discovery
(`src/pipeline/mod.rs:33`) — an acknowledged gap, see `.cairn/decisions/0003`.
The `main ⇄ pipeline` cycle through the root `Config` type is documented in
`.cairn/topology.md`; moving shared config out of `main.rs` is an open idea,
not a decision.

INV-PIPE-003 is a pointer rather than an enduring constraint; it is kept
because IDs are immutable, and retiring it is an owner's call.
