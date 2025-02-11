---
title: Pipeline
sidebarTitle: Pipeline
---

# Decision to choose a way to have a data pipeline process

**status**: draft

## Context

Boros requires a data pipeline to process user requests and events from the blockchain. The pipeline must be reliable, and each stage will schedule the tasks based on storage, so if the application restarts, the pipeline can start from the last point.

**Options:**
  - **Gasket:** library supports, input, output, standalone and retries.

## Decision

We chose **Gasket** due to it has been used in other applications, and it was made by a member of the team.

**Rationale**
  - The team already use Gasket.
  - Native support and configurable for retries.
  - Flexible.
