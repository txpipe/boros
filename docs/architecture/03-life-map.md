# 03 — Life map

<!-- GENERATED 2026-08-23 by cairn-survey @ fdec4da via git log --numstat + scc. Partition is curated from generated data; see deviation note. -->

> **Deviation note:** the skill's 12-month churn window is empty — the repo's last commit is 2025-05-27 (~15 months before this survey). 42 of 44 lifetime commits landed Jan–May 2025. The partition below is derived from **lifetime** churn × complexity instead, and the whole repo carries a `dormant` flag. Whether dormancy demotes everything to fossil is a human judgment recorded after this survey.

## Generated: churn × complexity (lifetime)

Top files by churn (total lines added+deleted across history; renames noted):

| File | Churn | Complexity (scc) | Note |
|---|---:|---:|---|
| `pipeline/ingest.rs` | 1,086 | 11 | churn leader |
| `storage/sqlite.rs` | 914 | 14 | |
| `network/peer.rs` | ~700 | **23** | complexity leader; churned as `pipeline/fanout/tx_submit_peer.rs` pre-rename |
| `ledger/u5c/mod.rs` | 580 | 12 | |
| `queue/chaining.rs` | 425 | 8 | |
| `network/mempool.rs` | ~314 | — | churned as `pipeline/fanout/mempool.rs` |
| `server/submit.rs` | 225 | 11 | |

Commits per top module (lifetime): pipeline 25, server 12, storage 11, ledger 6, network 6, queue 3, signing 3, validation 2.

## Curated: partition

| Tier | Modules | Basis |
|---|---|---|
| **Active core** | `pipeline`, `storage`, `network`, `queue` | high churn × complexity intersection; `network` inherits the churn of its pre-rename `fanout` history |
| **Stable periphery** | `ledger`, `server` | moderate churn, adapter/API shells |
| **Thin periphery** | `signing`, `validation`, `main` | low churn, small, single-purpose |
| **Fossil** | *(none within repo)* | repo is young; but **entire repo dormant since 2025-05** |

## Chartering nomination

Per the adoption path (charter the 2–3 active-core modules first): **`pipeline`**, **`storage`**, **`queue`** — with `network` as the fourth if appetite allows (highest complexity, weakest tests: 1 test function, and it's in the mock).

Change-coupling analysis (code-maat): **pending — tool not installed**; noted per skill guardrail.
Age strata (git-of-theseus): **pending — tool not installed**.
