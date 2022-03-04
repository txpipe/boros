# Boros

> Cardano Omnivore

## Intro

Boros is a tool that consumes a stream of Cardano transactions from different pluggable sources and takes cares of submitting them on-chain in an orderly and resilient fashion.

## Features

- [ ] Sources
  - [ ]  Kafka
  - [ ]  JSONL File (MVP)
  - [ ]  HTTP endpoint
- [ ] Accepts
  - [ ] CBOR Pre-signed Txs (MVP)
  - [ ] Unsigned JSON Tx
  - [ ] Metadata Fragment (for oracles)
  - [ ] DID Anchor
  - [ ] AirDrop
- [ ]  Validation
  - [ ]  Check inputs
  - [ ]  Check balance
  - [ ]  Check assets?
  - [ ]  Check metadata?
- [ ]  Sorting
  - [ ]  FIFO (MVP)
  - [ ]  User-defined priority buckets
  - [ ]  Time-bound (for oracles)
  - [ ]  By Tx-size
- [ ]  Throttle
  - [ ]  Configurable mempool
  - [ ]  Size-aware submission
  - [ ]  Throughput limitation
- [ ]  Outputs
  - [ ]  Node-To-Client protocols (MVP)
