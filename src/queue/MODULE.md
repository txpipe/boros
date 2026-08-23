+++
schema = "cairn/v0"
id = "queue"
role = "core-domain"
parnas-secret = "the scheduling policy — how weighted priority picks the next transactions, and how chained queues serialize dependent submissions via lock tokens"

[deps]
internal-allowed = ["storage"]
external-policy = "review-new"
verify = "just check-deps queue"
+++

# queue — module manifest

## Purpose

The scheduling heart of Boros. `priority` allocates each fanout batch across
named queues by weight; `chaining` gives dependent-transaction queues
exclusive-lock semantics (token-gated submission, streamed lock state).

## Invariants

- **INV-QUEUE-001** `[bound → just test-queue]` — Batch quota is allocated proportionally to queue weights and fully distributed (remainder included) (`it_should_calculate_quota`).
- **INV-QUEUE-002** `[bound → just test-queue]` — Transactions in queues removed from config are still drained, not stranded (`it_should_return_next_transactions_when_a_queue_is_removed_from_config`).
- **INV-QUEUE-003** `[bound → just test-queue]` — A chained queue admits one lock holder at a time; competing lockers wait or time out (`it_should_lock_queue`, `it_should_wait_timeout_to_lock_queue`, `it_should_lock_many_queue`).
- **INV-QUEUE-004** `[bound → just test-queue]` — Only the current lock token is valid for submission to a chained queue; unlock invalidates it (`it_should_return_token_*`, `it_should_unlock_queue`).
- **INV-QUEUE-005** `[llm-judged]` — Queue identity is its name alone (`Config` hashes/compares by name), so config reload with changed weights re-targets the same queue rather than creating a phantom.

## Relationships

- `customer-supplier` with **storage** (queue is the customer: priority and chaining read/write through storage's store types).

## Non-goals

- No transport, no validation, no signing: queue decides *order and admission*, nothing else.
