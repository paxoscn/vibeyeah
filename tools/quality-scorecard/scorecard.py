#!/usr/bin/env python3
"""
Compose the weighted scorecard from saved judgments + deterministic metrics.

No inference happens here: out/raw.json is the cache, so weights, thresholds and
corrections can be re-tuned and the scorecard regenerated for free.

Usage:
    python3 tools/quality-scorecard/harness.py     # once, to produce out/raw.json
    python3 tools/quality-scorecard/scorecard.py   # as often as you like
"""

import json
import sys

# harness.py owns the unit definitions and output location; importing it keeps
# one source of truth. It only calls the API from main(), which is __main__-guarded.
from harness import OUT
from harness import UNITS as ALL_UNITS

raw = json.loads((OUT / "raw.json").read_text())
met = json.loads((OUT / "metrics.json").read_text())
R = raw["responses"]
A = {k: v.get("answers", {}) for k, v in R.items()}

CROSS = ("documentation", "infrastructure", "tests")
DIMS = ["error_handling", "readability", "modularity", "security", "testability"]
UNITS = [u for u in raw["units"] if u not in CROSS]

# Relative importance of each dimension in the headline number. These are the
# only numbers you need to touch to re-tune the scorecard.
WEIGHTS = {
    "security": 0.20,
    "error_handling": 0.15,
    "test_coverage": 0.15,
    "modularity": 0.12,
    "infrastructure": 0.10,
    "readability": 0.10,
    "testability": 0.10,
    "documentation": 0.08,
}

# ---------------------------------------------------------------------------
# Corrections: places where the model's answer was checked against the code and
# found to be reading lexical shape rather than meaning. Each one is a claim
# verified by hand; the raw judgment is kept in raw.json either way.
#
# Re-check these when the code moves — they are pinned to a specific revision.
# ---------------------------------------------------------------------------
CORRECTIONS = [
    {
        "id": "data_layer.hardcoded_secret",
        "raw": A.get("data_layer", {}).get("hardcoded_secret", {}).get("noul"),
        "corrected": 0.02,
        "why": "Flagged on the identifiers `lark_app_secret` / `image_pull_secret`, "
               "which are database column names, not credentials. Verified by reading "
               "entity/organization.rs and entity/agent.rs: no literal secret is present.",
    },
    {
        "id": "infrastructure.secret_hygiene",
        "raw": A.get("infrastructure", {}).get("secret_hygiene", {}).get("score", 0) / 3,
        "corrected": 1 / 3,
        "why": "Placed on the worst level because deploy/test/init.sql contains the "
               "literal `REPLACE_WITH_YOUR_LARK_APP_SECRET` and deployment.yaml carries "
               "`postgres:postgres`. Both are placeholders in a *test* manifest, and "
               "init.sql explicitly warns against committing real credentials, so "
               "'secrets are written into committed files' is not literally true. "
               "Level 1 is fair: configs/.hermes/.env is force-un-ignored in .gitignore, "
               "which invites a real key to be committed later.",
    },
]
CORR = {c["id"]: c["corrected"] for c in CORRECTIONS}

missing = [u for u in ALL_UNITS if u[0] not in A or "error" in R.get(u[0], {})]
if missing:
    sys.exit(f"raw.json is missing or errored for: {[m[0] for m in missing]}\n"
             f"re-run harness.py before scoring.")


def sc(unit, dim):
    return A[unit][dim]["score"] / 3.0


def conf(unit, dim):
    return A[unit][dim]["confidence"]


# --- unit-level dimensions -------------------------------------------------
unit_scores = {d: {u: CORR.get(f"{u}.{d}", sc(u, d)) for u in UNITS} for d in DIMS}

# Unweighted mean: every module counts once, so boilerplate cannot dominate.
# LOC-weighted is reported alongside as a sensitivity check.
dims = {}
for d in DIMS:
    vals = unit_scores[d]
    plain = sum(vals.values()) / len(vals)
    w = {u: met["per_unit"][u]["loc"] ** 0.5 for u in UNITS}
    tw = sum(w.values())
    dims[d] = {"plain": plain,
               "loc_weighted": sum(vals[u] * w[u] for u in UNITS) / tw,
               "mean_confidence": sum(conf(u, d) for u in UNITS) / len(UNITS),
               "unreviewed": [u for u in UNITS if conf(u, d) < 0.50]}

# --- cross-cutting dimensions ---------------------------------------------
docm = A["documentation"]
dims["documentation"] = {
    "plain": (docm["doc_completeness"]["score"] / 3 + docm["doc_specificity"]["score"] / 3) / 2,
    "mean_confidence": (docm["doc_completeness"]["confidence"]
                        + docm["doc_specificity"]["confidence"]) / 2,
    "unreviewed": [],
}

infra = A["infrastructure"]
dims["infrastructure"] = {
    "plain": (infra["build_reproducibility"]["score"] / 3
              + infra["deployment_safety"]["score"] / 3
              + CORR["infrastructure.secret_hygiene"]) / 3,
    "mean_confidence": (infra["build_reproducibility"]["confidence"]
                        + infra["deployment_safety"]["confidence"]
                        + infra["secret_hygiene"]["confidence"]) / 3,
    "unreviewed": [],
}

# Breadth is a fact from the code; depth and isolation are quality judgments.
t = A["tests"]
det_breadth = (met["totals"]["units"] - met["totals"]["units_with_no_tests"]) / met["totals"]["units"]
dims["test_coverage"] = {
    "plain": (0.5 * det_breadth
              + 0.5 * ((t["test_depth"]["score"] / 3 + t["test_isolation"]["score"] / 3) / 2)),
    "mean_confidence": (t["test_depth"]["confidence"] + t["test_isolation"]["confidence"]) / 2,
    "unreviewed": [],
    "detail": {"units_with_tests": met["totals"]["units"] - met["totals"]["units_with_no_tests"],
               "units_total": met["totals"]["units"],
               "tests_per_kloc": met["totals"]["tests_per_kloc"]},
}

# --- gates -----------------------------------------------------------------
# Separate from the weighted score on purpose: a compensating average can hide a
# single disqualifying defect, so these are hard conditions. They are findings
# verified by hand, not model output.
gates = [
    {
        "id": "SEC-1",
        "condition": "No request-controlled value reaches a shell",
        "status": "FAIL",
        "evidence": "api/sync.rs takes `paths` and `commit_message` from the request body "
                    "of any authenticated org member and passes them to "
                    "service/sync.rs::build_sync_script, which interpolates them into a "
                    "single-line shell script run via `sh -c` in the pod "
                    "(k8s.rs::exec_in_pod). `for SRC in {path_list}` is unquoted, and "
                    "`git commit -m '{commit_message}'` breaks out with a single quote. An "
                    "org member can therefore run arbitrary commands in the pod, and can "
                    "read the org's git credentials, which git_sync_org_agents decrypts "
                    "from the organization record and injects into that same container.",
    },
    {
        "id": "TEST-1",
        "condition": "Privileged paths have at least one test",
        "status": "FAIL",
        "evidence": f"{met['totals']['units_with_no_tests']} of {met['totals']['units']} "
                    f"modules have no test module at all, including every auth, k8s and API "
                    f"handler module: {', '.join(met['totals']['untested_units'])}.",
    },
    {
        "id": "DEP-1",
        "condition": "The build is pinned end to end",
        "status": "FAIL",
        "evidence": "Cargo.lock is committed and CI builds with --locked, but container "
                    "base images float (ubuntu:22.04 by tag, and Dockerfile-next1/2 depend "
                    "on desktop:20260608 / desktop:20260612 by tag) and the Rust toolchain "
                    "is `stable` in CI rather than pinned.",
    },
]

overall = sum(dims[d]["plain"] * WEIGHTS[d] for d in WEIGHTS)

card = {
    "overall": overall,
    "weights": WEIGHTS,
    "dimensions": dims,
    "unit_scores": unit_scores,
    "unit_confidence": {d: {u: round(conf(u, d), 2) for u in UNITS} for d in DIMS},
    "gates": gates,
    "corrections": CORRECTIONS,
    "nouls": {u: {n: A[u][n]["noul"] for n in
                  ("hardcoded_secret", "unvalidated_input_to_sink", "blocking_call_in_async")
                  if n in A[u]}
              for u in UNITS},
    "metrics": met["totals"],
    "token_usage": {
        "input": sum(v.get("usage", {}).get("input_tokens", 0) for v in R.values()),
        "output": sum(v.get("usage", {}).get("output_tokens", 0) for v in R.values()),
    },
}

# --- sensitivity: how much does the headline move if the weights change? ----
scen = {
    "equal weights": {k: 1 / len(WEIGHTS) for k in WEIGHTS},
    "security-heavy": {**WEIGHTS, "security": WEIGHTS["security"] + 0.20},
    "flip security/readability": {**WEIGHTS, "security": WEIGHTS["readability"],
                                  "readability": WEIGHTS["security"]},
}
card["sensitivity"] = {
    name: sum(w[k] / sum(w.values()) * dims[k]["plain"] for k in WEIGHTS)
    for name, w in scen.items()
}

(OUT / "scorecard.json").write_text(json.dumps(card, indent=2, ensure_ascii=False))

# --- print -----------------------------------------------------------------
print(f"{'DIMENSION':<18}{'SCORE':>7}{'/100':>7}{'LOCw':>7}{'conf':>7}")
print("-" * 46)
for d in sorted(WEIGHTS, key=lambda x: -WEIGHTS[x]):
    m = dims[d]
    lw = f"{m['loc_weighted']:.2f}" if "loc_weighted" in m else "  —"
    print(f"{d:<18}{m['plain']:>7.2f}{m['plain'] * 100:>7.0f}{lw:>7}"
          f"{m['mean_confidence']:>7.2f}")
print("-" * 46)
print(f"{'OVERALL':<18}{overall:>7.2f}{overall * 100:>7.0f}")
print()
print("GATES")
for g in gates:
    print(f"  [{g['status']}] {g['id']}: {g['condition']}")
print()
print("SENSITIVITY (headline under different weights)")
for name, v in card["sensitivity"].items():
    print(f"  {name:<28}{v * 100:.0f}")
print()
print("LOW-CONFIDENCE JUDGMENTS (confidence < 0.50, route to human review)")
for d in DIMS:
    if dims[d]["unreviewed"]:
        print(f"  {d:<16} {', '.join(dims[d]['unreviewed'])}")
print()
print(f"tokens: in={card['token_usage']['input']:,} out={card['token_usage']['output']:,}")
print(f"wrote {OUT / 'scorecard.json'}")
