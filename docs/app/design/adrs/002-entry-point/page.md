---
title: Entry point
sidebarTitle: Entry point
---

# Decision to choose an entry point server

**status**: draft

## Context

Boros needs to expose an API where users can be able to send a transaction to be processed and watch for events.

**Options:**
  - **HTTP**: most common protocol.
  - **GRPC**: support bidirectional events.

## Decision

We chose GRPC because we already have protobuf in UTxORPC for transaction submission, furthermore, the users can use Dolos or just change the URL and start using Boros.

**Rationale**
  - Support to bidirectional that will be used to notify events.
  - Reuse UTxORPC protobuf.


## Consequences
  - **Positive Impact:** flexibility to use Dolos or Boros to submit transactions, [good documentation](https://utxorpc.org/).
  - **Potential Drawbacks:** frontend can't connect directly without a proxy or backend.
