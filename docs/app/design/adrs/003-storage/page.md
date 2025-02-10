---
title: Storage
sidebarTitle: Storage
---

# Decision to choose a storage

**status**: draft

## Context

The storage needs to be used to persist the in-flight transactions, and the pipeline needs to use the storage as a source to execute the stages, so each stage consumes the in-flight transactions in a different state, therefore each stage executes a query to select the next transaction to be processed.

**Options:**
  - **[Redb](https://github.com/cberner/redb)**: portable, high-performance, ACID, embedded key-value store written in rust.
  - **SQLite**: SQL database, portable, ACID, embedded.

## Decision

We chose **SQLite** for its possibility to execute SQL queries.

**Rationale**
  - SQL queries.
  - Supported by [SQLX](https://github.com/launchbadge/sqlx) library.

