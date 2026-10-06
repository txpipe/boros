# Shape (census)

<!-- GENERATED 2026-08-23 by cairn-survey @ fdec4da. Numbers derived via tokei, grep, cargo metadata. Do not hand-edit numeric sections; regenerate. -->

## Generated numbers

### Size per module (tokei, lines of code)

| Module | Code lines | Files |
|---|---:|---:|
| `network` | 1,112 | 4 |
| `pipeline` | 907 | 5 |
| `storage` | 846 | 3 |
| `queue` | 642 | 4 |
| `ledger` | 501 | 5 |
| `server` | 305 | 3 |
| `signing` | 242 | 3 |
| `validation` | 96 | 1 |
| `main.rs` | 100 | 1 |
| **Total (src/)** | **3,972** | **27** |

Generated `spec` crate (protobuf bindings, excluded from analysis): 1,950 code lines / 8 files.

### Ratios

- Comment lines in `src/`: 44 (~1.1% of code) — effectively uncommented.
- Test functions: 33, concentrated in 6 files:
  `storage/sqlite.rs` (15), `queue/chaining.rs` (8), `pipeline/ingest.rs` (5), `queue/priority.rs` (3), `ledger/relay/mod.rs` (1), `network/mock_ouroboros_tx_submit_server.rs` (1).
  No integration-test target; `test/` holds fixtures only (CBOR tx files, block data).
- `unsafe`: 1 expression in first-party code (`signing/key/derive.rs:85`, `SecretKeyExtended::from_bytes_unchecked`).
  Transitive unsafe (cargo-geiger): **pending — not yet run** (full-graph compile deferred).

### Dependency weight

- 461 packages in the resolved graph for ~4k first-party lines (~8.7 lines per dependency package).
- Notable heavy subtrees: `tonic`/`prost` (gRPC), `sqlx` (SQLite), `pallas` 1.0.0-alpha.2 (Cardano primitives, `phase2` feature), `vaultrs` (HashiCorp Vault), `gasket` (pinned to a git fork: `construkts/gasket-rs`).

## Interpretation (curated)

Boros is a small, young service (v0.1.0, bin-only crate) whose weight is in its dependencies, not its own code. Three facts stand out:

1. **Test coverage is bimodal.** `storage` and `queue` are meaningfully tested; `network`, `server`, `signing`, `validation` are essentially untested. The untested set includes the one `unsafe` block (key derivation) and all I/O boundaries.
2. **The comment ratio (~1%) means the code carries no embedded rationale.** Manifests and flow docs are the only place intent can live — chartering matters more than usual here.
3. **`gasket` is pinned to a personal git fork**, a supply-chain and bus-factor liability — decision recorded in [decisions/0001](decisions/0001-return-gasket-to-published-release.md).
