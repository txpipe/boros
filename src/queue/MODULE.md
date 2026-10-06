+++
schema = "cairn/v0"
id = "queue"
short = "QUEUE"
role = "core-domain"
purpose-status = "inferred"
parnas-secret = "the scheduling policy — how weighted priority picks the next transactions, and how chained queues serialize dependent submissions via lock tokens"

[deps]
internal-allowed = ["storage"]
external-policy = "review-new"
verify = "just check-deps queue"
+++

# queue — module manifest

## Purpose

### System contribution

The scheduling heart of Boros. Contributes to the workspace outcome
[OUT-WS-002](../../WORKSPACE.md#system-contribution): submitters control the
relative order of their submissions. `priority` allocates each dispatch batch
across named queues by weight; `chaining` gives dependent-transaction queues
exclusive-lock semantics (token-gated submission, streamed lock state).

### Beneficiaries

| Beneficiary | Connection to outcomes | Provenance |
|---|---|---|
| `pipeline::ingest` | Asks `Priority::next` for each dispatch batch (OUT-WS-002) | Observed caller |
| `server` (boros submit service) | Locks, validates tokens on and unlocks chained queues (OUT-WS-002) | Observed caller of `TxChaining` |
| Submitters building transaction chains | Need to read the latest queued transaction and append the next one without a competing writer | Inferred from the `LockState` RPC and the token check; no client was inspected |
| Operators | Declare queues, weights, chaining and server signing in config | Observed config schema; operator needs beyond that are unknown |

### Responsibilities

| ID | Outcome | Responsibility | Realization | Coverage |
|---|---|---|---|---|
| RESP-QUEUE-001 | OUT-WS-002 | Allocate each dispatch batch across queues in proportion to their weights | INV-QUEUE-001, INV-QUEUE-002. INV-QUEUE-001's claim that the cap is fully distributed is contradicted by the code: rounding can over- or undershoot the cap, and leftover from the last queue visited is dropped. Fairness across successive batches is not declared. | partial |
| RESP-QUEUE-002 | OUT-WS-002 | Serialize submissions to chained queues through exclusive, expiring lock tokens | INV-QUEUE-003, INV-QUEUE-004. Lock state is held in memory only. | partial |
| RESP-QUEUE-003 | OUT-WS-002 | Define queue identity and the default queue | INV-QUEUE-005 | partial |

### Owned information

| Information | Authority and representation ownership |
|---|---|
| Queue configuration (`name`, `weight`, `chained`, `server_signing`) and the `default` queue | Owns the schema and identity rules; operators supply values; `main` inserts the default queue when absent |
| Chained-queue lock tokens | Authoritative, in memory, per process; lost on restart |
| Per-queue batch quotas | Derived per call from storage counts; not persisted |

### Boundary allocation

| Responsibility | This module | Collaborator obligation and owner | Contract gap |
|---|---|---|---|
| RESP-QUEUE-001 | Compute per-queue limits | `storage` returns counts and selections honoring them (its INV-STORE-002) | No expectation declared |
| RESP-QUEUE-001 | Hand back the batch | `pipeline::ingest` dispatches it; it retries a transaction whose queue is missing from config | Interaction with INV-QUEUE-002 is unassessed |
| RESP-QUEUE-002 | Issue, check and expire tokens | `server` checks the token before `create` and unlocks after it | `server` has no manifest; no expectation declared |
| RESP-QUEUE-002 | Stream the latest queued transaction with the token | `storage::latest` returns the most recent submission in the queue | It may return a `Failed` transaction (TODO in source) |

### Operating envelope

| Responsibility | Mode / condition | Scope | Scenarios / gaps |
|---|---|---|---|
| RESP-QUEUE-001 | Normal drain across configured queues | in-scope | [fanout flow](../../.cairn/flows/fanout.md) |
| RESP-QUEUE-001 | Queue removed from config while it holds transactions | in-scope | INV-QUEUE-002; downstream signing lookup unassessed |
| RESP-QUEUE-001 | Weights do not split the cap into whole shares | in-scope | Gap: shares are rounded independently, so the batch can exceed or fall short of the cap; no claim or test covers it |
| RESP-QUEUE-001 | A queue holds fewer transactions than its share | in-scope | Gap: unused capacity passes to queues in hash order, and the last queue's leftover is dropped, so the batch size depends on that order; no test covers it |
| RESP-QUEUE-001 | Starvation of low-weight queues under sustained load | unknown | No claim or scenario |
| RESP-QUEUE-002 | Lock, submit, unlock within the timeout | in-scope | [submit flow](../../.cairn/flows/submit.md) |
| RESP-QUEUE-002 | Lock holder exceeds the 30 s timeout | in-scope | Lock expires; a late submission is rejected (INV-QUEUE-004) |
| RESP-QUEUE-002 | Process restart while a lock is held | unknown | Tokens are lost; intended behavior undecided |
| RESP-QUEUE-002 | Several Boros instances serving one queue | unknown | Locks are per process |
| RESP-QUEUE-003 | Config change at runtime | unknown | Config is read once at startup |

## Guarantees

- **INV-QUEUE-001** `[bound → just test-queue]` — Batch quota is allocated proportionally to queue weights and fully distributed (remainder included) (`it_should_calculate_quota`).
  - Surface: `Priority::next` (quota computation).
  - When: At least one queue holds transactions in the requested status.
  - Then: Each queue's share is its weight's fraction of the cap, rounded to the nearest whole transaction; `it_should_calculate_quota` checks one exact split (weights 1/2/2, cap 10 → 2/4/4). Capacity a queue cannot use is added to the share of the queue visited after it.
  - Contradiction: the claim's "fully distributed (remainder included)" does not hold. Rounding each share can exceed or fall short of the cap (weights 1/1, cap 5 → 3 + 3 = 6; weights 1/1/1, cap 10 → 9). Queues are visited in hash order and the last one's leftover is dropped (cap 50, two weight-1 queues holding 100 and 1 → a batch of 26 or 50). The claim and tier stay as declared: demoting needs a human-approved change (§4), and fixing the code is outside this charter.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-QUEUE-002** `[bound → just test-queue]` — Transactions in queues removed from config are still drained, not stranded (`it_should_return_next_transactions_when_a_queue_is_removed_from_config`).
  - Surface: `Priority::next`.
  - When: Stored transactions belong to a queue absent from config.
  - Then: That queue is scheduled with the default weight.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-QUEUE-003** `[bound → just test-queue]` — A chained queue admits one lock holder at a time; competing lockers wait or time out (`it_should_lock_queue`, `it_should_wait_timeout_to_lock_queue`, `it_should_lock_many_queue`).
  - Surface: `TxChaining::lock`.
  - When: The queue is configured `chained`.
  - Then: A second locker receives its token only after the first unlocks or its lock times out.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.
- **INV-QUEUE-004** `[bound → just test-queue]` — Only the current lock token is valid for submission to a chained queue; unlock invalidates it (`it_should_return_token_*`, `it_should_unlock_queue`).
  - Surface: `TxChaining::{is_valid_token, unlock}`.
  - When: The queue is configured `chained`.
  - Then: `is_valid_token` is true only for the token most recently issued and not yet released or expired.
  - Migration: moved from Invariants under SPEC §3.4; ID and tier unchanged.

## Invariants

- **INV-QUEUE-005** `[llm-judged]` — Queue identity is its name alone (`Config` hashes/compares by name), so config reload with changed weights re-targets the same queue rather than creating a phantom.
  - Scope and observation: every set of queue `Config` values, at construction.

## Relationships

- `customer-supplier` with **storage** (queue is the customer: priority and chaining read/write through storage's store types).

## Non-goals

- No transport, no validation, no signing: queue decides *order and admission*, nothing else.
