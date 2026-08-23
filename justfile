set shell := ["bash", "-o", "pipefail", "-cu"]

# Cairn verifier verbs (SPEC §7): exit 0 = pass, stdout = evidence.
# Every verb here is referenced by a manifest; renaming one without
# updating its references is a lint error (L5).

# Run all bound constraints (workspace verify-all)
gate: check-deps-all test-storage test-queue test-ingest
    @echo "gate: all bound constraints passed"

# --- dependency constraints -------------------------------------------------

# Check a module's internal deps against its MODULE.md allowlist
check-deps id:
    python3 tools/cairn_check_deps.py {{id}}

check-deps-all:
    python3 tools/cairn_check_deps.py storage
    python3 tools/cairn_check_deps.py queue
    python3 tools/cairn_check_deps.py pipeline

# --- test-backed invariants -------------------------------------------------

test-storage:
    cargo test storage:: -- --nocapture 2>&1 | tail -20

test-queue:
    cargo test queue:: -- --nocapture 2>&1 | tail -20

test-ingest:
    cargo test ingest_tests:: -- --nocapture 2>&1 | tail -20
