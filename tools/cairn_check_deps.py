#!/usr/bin/env python3
"""Bound verifier for MODULE.md [deps].internal-allowed (cairn SPEC §3.1).

Derives the module's actual internal dependencies from `cargo modules
dependencies` and asserts they are a subset of the manifest allowlist.

Known blind spots: cargo-modules does not report const-only uses (see
.cairn/topology.md); such edges must be policed by review. The frontmatter
reader understands only a single string array assigned to `internal-allowed`
inside `[deps]` (no tomllib before Python 3.11); any other form reads as an
empty allowlist, which fails closed.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOP_MODULES = {
    "ledger", "network", "pipeline", "queue",
    "server", "signing", "storage", "validation",
}


def manifest_allowlist(module_id: str) -> list[str]:
    manifest = ROOT / "src" / module_id / "MODULE.md"
    if not manifest.exists():
        sys.exit(f"FAIL: no manifest at {manifest}")
    text = manifest.read_text()
    m = re.match(r"\+\+\+\n(.*?)\n\+\+\+", text, re.DOTALL)
    if not m:
        sys.exit(f"FAIL: no +++ frontmatter in {manifest}")
    deps = re.search(r"^\[deps\]\n(.*?)(?=^\[|\Z)", m.group(1), re.DOTALL | re.MULTILINE)
    allowed = deps and re.search(r"^internal-allowed\s*=\s*\[(.*?)\]", deps.group(1), re.DOTALL | re.MULTILINE)
    if not allowed:
        return []
    return [d.split("#")[0] for d in re.findall(r'"([^"]*)"', allowed.group(1))]


def actual_deps(module_id: str) -> set[str]:
    out = subprocess.run(
        ["cargo", "modules", "dependencies", "--bin", "boros",
         "--no-externs", "--no-sysroot"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    deps: set[str] = set()
    for src, dst in re.findall(r'"(boros[^"]*)" -> "(boros[^"]*)".*"uses"', out):
        src_mod = (src.split("::") + [None])[1]
        dst_mod = (dst.split("::") + [None])[1]
        if src_mod == module_id and dst_mod in TOP_MODULES and dst_mod != module_id:
            deps.add(dst_mod)
    return deps


def main() -> None:
    module_id = sys.argv[1]
    allowed = set(manifest_allowlist(module_id))
    actual = actual_deps(module_id)
    illegal = actual - allowed
    print(f"module: {module_id}")
    print(f"allowed: {sorted(allowed) or '(none)'}")
    print(f"actual:  {sorted(actual) or '(none)'}")
    if illegal:
        sys.exit(f"FAIL: undeclared internal deps: {sorted(illegal)}")
    print("PASS: actual deps ⊆ allowlist")


if __name__ == "__main__":
    main()
