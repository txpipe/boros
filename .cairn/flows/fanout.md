+++
schema = "cairn/v0"
flow = "fanout"
short = "FANOUT"
participants = ["pipeline", "queue", "signing", "validation", "ledger/u5c", "network", "storage"]
+++

# Flow — fanout (queue → peers)

<!-- Curated flow doc, drafted 2026-08-23 by cairn-survey @ fdec4da; diagram traced from src/pipeline/ingest.rs, src/pipeline/mod.rs, src/network/. -->

The `ingest` gasket stage drains `Pending` transactions by queue priority, optionally signs them server-side, re-validates, and broadcasts them to all connected Ouroboros tx-submission peers. Broadcast marks the tx `InFlight` stamped with the tip slot.

```mermaid
sequenceDiagram
    participant P as pipeline::ingest<br/>Stage/Worker
    participant PR as queue::priority<br/>Priority
    participant SG as signing<br/>dyn SigningAdapter (Vault)
    participant V as validation
    participant L as ledger::u5c<br/>dyn U5cDataAdapter
    participant DB as storage::sqlite<br/>SqliteTransaction
    participant N as network<br/>PeerManager → Peer/Mempool

    loop schedule (cap = 50 − queued)
        P->>PR: next(Pending, cap)
        PR->>DB: weighted pick per queue config
        PR-->>P: Vec<Transaction>
    end
    loop per Transaction
        opt queue.server_signing
            P->>SG: sign(tx.raw)
            SG-->>P: signed CBOR (Vec<u8>)
        end
        P->>V: validate_tx + evaluate_tx (MultiEraTx)
        V->>L: ledger state
        alt validation fails
            P->>DB: update(status = Failed)
        else ok
            P->>N: broadcast Message<Vec<u8>> (gasket channel)
            N->>N: Peer mempools serve ouroboros tx-submission
            P->>L: fetch_tip()
            P->>DB: update(status = InFlight, slot = tip)
        end
    end
```

## Invariants

- **INV-FANOUT-001** `[unverified]` — A transaction reaches `InFlight` only after a successful broadcast, and its `slot` is set to the tip at broadcast time (this is what `monitor`'s retry window keys off).
- **INV-FANOUT-002** `[unverified]` — A transaction on a `server_signing` queue is never broadcast unsigned; absence of a signing adapter causes retry, not passthrough.
- **INV-FANOUT-003** `[unverified]` — In-flight output never exceeds the channel cap (50); scheduling backs off rather than dropping.
