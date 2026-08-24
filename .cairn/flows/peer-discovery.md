+++
schema = "cairn/v0"
flow = "peer-discovery"
short = "DISCOVERY"
participants = ["pipeline", "ledger/relay", "network"]
+++

# Flow — peer discovery

<!-- Curated flow doc, drafted 2026-08-23 by cairn-survey @ fdec4da; diagram traced from src/pipeline/peer_discovery/mod.rs. -->

The `peer_discovery` gasket stage tops up the peer pool toward `desired_peer_count`, choosing 50/50 between on-chain relays (via `RelayDataAdapter` — **currently the mock implementation**, an acknowledged gap: see `src/pipeline/mod.rs:33` and [decision 0003](../decisions/0003-real-relay-source-for-peer-discovery.md)) and peers learned from existing peers' peer-sharing.

```mermaid
sequenceDiagram
    participant R as ledger::relay<br/>dyn RelayDataAdapter (MOCK)
    participant D as pipeline::peer_discovery<br/>Stage/Worker
    participant PM as network::peer_manager<br/>PeerManager
    participant PE as network::peer<br/>Peer (ouroboros)

    loop while connected < desired_peer_count
        D->>R: get_relays() → Vec<String>
        D->>PM: pick_peer_rand(peers_per_request) → Option<String>
        Note over D: coin flip chooses relay vs peer-shared address
        D->>PM: add_peer(addr)
        PM->>PE: connect + start tx-submission protocol
    end
```

## Invariants

- **INV-DISCOVERY-001** `[unverified]` — The pool converges toward `desired_peer_count` and does not add peers beyond outstanding need (`peer_discovery_queue` bounds in-flight additions).
