---
title: Fanout
---

# Boros - Fanout Mechanism

This document provides a comprehensive overview of the fanout mechanism employed by Boros

## Context

In high-volume decentralized applications (dApps), direct peer-to-peer transaction submission is challenging due to factors such as network variability, high transaction loads, and the need for resilience. Instead of dApps managing complex network connections themselves, Boros acts as a dedicated **Transaction Provider**. It ingests, validates, and then fans out transactions to multiple peers simultaneously, thereby abstracting the complexity of direct peer connectivity and ensuring that transactions are efficiently and reliably propagated across the Cardano network.

## Boros Fanout Mechanism Overview

Boros employs a fanout mechanism to:
- **Maximize Throughput:** By sending copies of each validated transaction through multiple outbound peer addresses concurrently.
- **Enhance Resilience:** By ensuring redundancy across several peer connections.
- **Simplify Integration:** Allowing dApps to adopt a "fire-and-forget" model without needing to manage low-level peer connectivity, retries, or error handling.

## Detailed Fanout Process

### 1. Transaction Preparation
- Although `ingest` is a whole different `stage` from the `fanout stage`, this step is critical for the transactions propagated in the `fanout stage`
- The `fanout stage` will only fanout transactions that are marked as `Validated` in the transaction storage.
- The `ingest stage` will schedule and execute each incoming transactions, marking them from `Pending` to `Validated`.

### 2. Peer Manager Initialization
- The stage starts by `bootstrapping` the configuration-provided peer addresses.
- These peer addresses will be passed through the `peer manager`.
- The `peer manager` will be the one to instantiate a connection for each peer, initializing a `peer client`.
- Each `peer client` will run concurrently with each other, and will start listening for a request from the `peer server`.
- The stage `worker` will now have an instance of the peer manager for the next processes.

### 3. Transaction Fanout
- After the bootstrap process, the `worker` will now `schedule` each `validated transactions` in the transaction storage of Boros.
- After the schedule process, the `worker` will now `execute` the scheduled `transaction unit`.
- The instance of the peer manager is used to add the transactions to each and every peer client's `per-peer FIFO queue` which in this case we call it `mempool`.
- The transaction status will now be marked as `Inflight`.
- Every peer client maintains its own FIFO queue to temporarily store the added transactions until the connected peer server requests them.
- Since each queue operates independently, issues with one peer connection do not disrupt the entire distribution process.

### 4. Peer-to-Peer Transaction Processing
- The initialized `peer client` will initiate the `tx-submission mini-protocol` by sending a `MsgInit` to the `peer server`.
- The peer client will process `outstanding requests` or `unfulfilled requests` first.
- These outstanding requests represents a number of unfulfilled requests from the peer server.
- If there's no outstanding requests, the peer client will now then listen for the `peer server's next request`.
- This request will either be a `MsgRequestTxIdsBlocking`, `MsgRequestTxIdsNonBlocking`, or `MsgRequestTxs`.
- The peer server will always request for `transaction ids` first, either `blocking` or `non-blocking`.
- `If the FIFO is empty, the consumer must use a blocking request; otherwise, it must be a non-blocking request.`
- The peer client will reply to the corresponding request made by peer server.
- This will always be the case until the peer server decides to request for `transaction bodies` via `MsgRequestTxs` using its received transaction ids.
- After the peer client's `MsgReplyTxs`, the peer client will then `acknowledge` its own copy of the transaction.

## Visualization: Boros as a Transaction Provider

Boros can be visualized as a comprehensive Transaction Provider comprising the following components:

- **Transaction Storage:**  
  A persistent queue system that holds incoming transactions submitted by dApps.

- **Fanout Mechanism:**  
  The core mechanism that copies each validated transaction and distributes these copies concurrently to multiple outbound connections (peer clients).

- **Per-Peer FIFO Queues:**  
  Each outbound channel maintains its own FIFO queue to manage the transaction propagation process independently.

- **Dynamic Connectivity:**  
  Integration of both static **(on-chain relay data)** and dynamic **(p2p discovery via the peer sharing mini‑protocol)** methods to establish and maintain robust peer connections.

- **Monitoring and Feedback:**  
  A system that tracks acknowledgments from peer servers, providing real-time status updates and enabling adaptive behavior to ensure optimal propagation.

This visualization shows Boros as a `middle man` that accepts transactions from dApps and reliably distributes them across the network, effectively decoupling the dApps from the complexities of direct peer management.

## Advantages of the Fanout Mechanism

- **High Throughput:**  
  Concurrent distribution across multiple channels reduces latency and speeds up network-wide transaction propagation.

- **Resilience:**  
  Redundant peer connections ensure that transaction delivery is maintained even if one or more channels experience failures.

- **Load Balancing:**  
  Distributing transactions evenly prevents any single connection from becoming a bottleneck, optimizing resource usage across the network.

- **Simplified Integration for dApps:**  
  dApps can submit transactions without having to manage complex network connectivity, relying on Boros to handle retries, error management, and dynamic peer selection.

## Conclusion

Boros’s fanout mechanism is essential for its role as a Transaction Provider. By efficiently copying and concurrently distributing transactions through multiple peer clients—with independent FIFO queues and dynamic connectivity—Boros delivers high-volume transaction submissions to the Cardano network with enhanced throughput, resilience, and simplicity.
