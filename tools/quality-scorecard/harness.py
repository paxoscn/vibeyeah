#!/usr/bin/env python3
"""
TypeSafe-powered code-quality scorecard for vibeyeah.

Design (per the TypeSafe skill's "keep code in control" guidance):
  * code  -> deterministic facts (LOC, panic sites, test counts, unsafe, ...)
  * Jev   -> semantic judgments that need understanding, as Score + Noul questions
  * code  -> composes the weighted scorecard from the saved judgments

Inference runs once and is persisted to out/raw.json, so weights and thresholds
can be re-tuned later without paying for inference again.

Usage:
    export TYPESAFE_API_KEY=...
    python3 tools/quality-scorecard/harness.py            # collect + score (spends tokens)
    python3 tools/quality-scorecard/harness.py --dry-run  # collect + metrics only, no API calls
    python3 tools/quality-scorecard/scorecard.py          # compose from the saved judgments
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# Repo root defaults to two levels up from this file; override for a checkout
# kept somewhere unusual. Output lands next to the script unless SCORECARD_OUT
# says otherwise.
HERE = Path(__file__).resolve().parent
REPO = Path(os.environ.get("VIBEYEAH_REPO", HERE.parents[1])).resolve()
OUT = Path(os.environ.get("SCORECARD_OUT", HERE / "out")).resolve()

MODEL = os.environ.get("TYPESAFE_MODEL", "jev-latest")
ENDPOINT = "https://api.typesafe.ai/v1/systemone"

PROJECT_CONTEXT = (
    "vibeyeah is a Rust backend (axum + sea-orm + kube-rs + openlark) that provisions "
    "per-user containerized AI agent desktops on Kubernetes. Users authenticate, get a pod "
    "with a remote desktop stream, and interact with a Lark (Feishu) / WeChat bot that drives "
    "the agent. Single team-owned codebase, Chinese-language comments, CI runs "
    "cargo fmt + clippy + build + test."
)

# --------------------------------------------------------------------------
# Units of the codebase that get scored
# --------------------------------------------------------------------------
# name, role, [relative paths], kind
UNITS = [
    ("service/lark_bot", "Lark (Feishu) bot: event handling, message routing, agent replies",
     ["service/lark_bot.rs"], "code"),
    ("service/k8s", "Kubernetes resource management: pods, deployments, kubeconfig, secrets",
     ["service/k8s.rs"], "code"),
    ("service/user_home", "User home-directory provisioning and filesystem bootstrap",
     ["service/user_home.rs"], "code"),
    ("service/callback", "Lark callback endpoints: event verification and dispatch",
     ["service/callback.rs"], "code"),
    ("service/lark_qr", "Lark QR-code login flow",
     ["service/lark_qr.rs"], "code"),
    ("service/agent", "Agent lifecycle: create, start, stop, status",
     ["service/agent.rs"], "code"),
    ("service/auth", "Authentication and session/credential handling",
     ["service/auth.rs"], "code"),
    ("service/sync", "Data synchronisation with upstream directory sources",
     ["service/sync.rs"], "code"),
    ("service/organization", "Organisation management and ownership",
     ["service/organization.rs"], "code"),
    ("service/wechat_qr", "WeChat QR-code login flow",
     ["service/wechat_qr.rs"], "code"),
    ("api", "HTTP API handlers: routing, request parsing, response shaping",
     ["api/agent.rs", "api/auth.rs", "api/callback.rs", "api/mod.rs",
      "api/organization.rs", "api/sync.rs", "api/user.rs", "api/wechat.rs"], "code"),
    # Migrations are near-duplicate boilerplate; sample the ends rather than
    # sending all 20, which would spend tokens without changing the judgment.
    ("data_layer", "Database entities and schema migrations (migrations sampled)",
     ["entity/*.rs", "migration/*.rs@3"], "code"),
    ("config_middleware", "Configuration loading and HTTP middleware (auth layer)",
     ["config.rs", "middleware/auth.rs", "middleware/mod.rs"], "code"),
    ("app_entry", "Application entrypoint, router wiring, background schedulers",
     ["main.rs", "lib.rs"], "code"),
]

# --------------------------------------------------------------------------
# Questions
# --------------------------------------------------------------------------
# Five universal Score dimensions, one per unit. Level descriptions are concrete
# situations rather than degrees, so each level stands on its own.

SCORE_DIMENSIONS = {
    "error_handling": {
        "instructions": (
            "How well does this code handle failure and error conditions? Consider how "
            "fallible operations (I/O, network, database, external APIs) are treated and "
            "what happens to the caller when they fail."
        ),
        "criteria": [
            "Failure paths are unhandled: unchecked unwrap/expect, panics, or silently "
            "discarded errors on operations that can fail in normal operation",
            "Errors are surfaced but inconsistently: some paths return errors while others "
            "panic or swallow them, with no clear rule for which",
            "Most fallible paths return or propagate errors; a few unchecked spots remain, "
            "mostly in startup or genuinely-impossible branches",
            "Errors are consistently propagated with context, callers can distinguish "
            "failure modes, and no reachable panic remains on ordinary error paths",
        ],
    },
    "readability": {
        "instructions": (
            "How readable and well-organised is this code for a maintainer who is new to "
            "this file? Consider naming, function size and structure, nesting depth, and "
            "whether comments explain intent."
        ),
        "criteria": [
            "Hard to follow: unclear or misleading names, deep nesting, long unstructured "
            "blocks where the flow cannot be traced",
            "Understandable but uneven: naming and structure vary within the file, and a "
            "reader must re-read sections to follow the control flow",
            "Clear names and structure throughout; a new maintainer can follow most flows "
            "on a first read without external help",
            "Consistently clear: intention-revealing names, well-factored flows, and "
            "comments that explain why the code is this way rather than restating what it does",
        ],
    },
    "modularity": {
        "instructions": (
            "How well does this code separate concerns and limit coupling? Consider whether "
            "unrelated responsibilities sit together, whether callers reach into internals, "
            "and how far a change ripples."
        ),
        "criteria": [
            "Concerns are entangled: unrelated responsibilities such as transport, business "
            "rules, persistence and I/O are mixed together in the same functions",
            "Some separation exists, but responsibilities leak across boundaries and callers "
            "depend on implementation details rather than a defined interface",
            "Clear responsibilities with defined interfaces; a change to one concern stays "
            "mostly local to this unit",
            "Well-bounded units with narrow interfaces; dependencies point in one direction "
            "and units can be understood, replaced or tested independently",
        ],
    },
    "security": {
        "instructions": (
            "How well does this code guard against security and authorisation failures? "
            "Consider trust in external input, authorisation at privileged boundaries, "
            "credential handling, and whether secrets can leak into logs or errors."
        ),
        "criteria": [
            "Trusts input or callers without checks, or handles credentials unsafely; "
            "privileged operations are reachable without an authorisation decision",
            "Partial validation or authentication, but with real gaps: some privileged "
            "paths lack an authorisation check, or unsafe defaults are used",
            "Input is validated and authorisation enforced on the main paths; only minor "
            "hardening gaps remain",
            "Consistent input validation, an explicit authorisation decision at every "
            "privileged boundary, and secrets kept out of code, logs and error messages",
        ],
    },
    "testability": {
        "instructions": (
            "How easily could this code be verified by automated tests? Consider whether "
            "logic is separated from side effects, whether state is passed in or hidden in "
            "globals, and whether there are seams to substitute collaborators."
        ),
        "criteria": [
            "Untestable as written: business logic is welded to I/O, globals and hard-coded "
            "externals with no seam to substitute",
            "Testable only with heavy setup: logic and side effects are entangled, so "
            "exercising one path requires standing up most of the surrounding system",
            "Core logic can be exercised with modest setup; some I/O still needs stubbing "
            "or a real dependency to be present",
            "Logic is separated from effects behind clear seams, so behaviour can be "
            "exercised directly and deterministically",
        ],
    },
}

# Concrete red flags. Noul returns P(yes); code gates on a threshold, and unlike a
# Score these have no separate confidence, so we ask them one label at a time.
NOUL_FLAGS = {
    "hardcoded_secret": (
        "This code contains a hard-coded credential, API key, token or password that is "
        "written into the source rather than read from configuration or the environment."
    ),
    "unvalidated_input_to_sink": (
        "User-controlled or externally-supplied input reaches a dangerous sink (a shell "
        "command, a filesystem path, a raw query, or an outbound request URL) without "
        "validation or escaping."
    ),
    "blocking_call_in_async": (
        "This code performs a blocking, synchronous operation (blocking filesystem, "
        "blocking network, thread sleep or CPU-heavy work) inside an async function "
        "without offloading it to a blocking thread pool."
    ),
}

DOC_QUESTIONS = {
    "doc_completeness": {
        "type": "score",
        "instructions": (
            "How completely does this documentation equip a new engineer to understand, run, "
            "operate and change this system? Consider whether the key concepts, setup steps, "
            "architecture and operational procedures are all covered."
        ),
        "criteria": [
            "Sparse or missing: a reader cannot get the system running or understand its "
            "shape from these documents alone",
            "Covers the basics such as setup and purpose, but architecture, operational "
            "procedures or failure modes are absent or hand-waved",
            "Covers purpose, setup, architecture and day-to-day operation; a new engineer "
            "could become productive with only minor gaps",
            "Comprehensive and maintained: concepts, architecture, setup, operations and "
            "failure handling are all documented with enough specificity to act on",
        ],
    },
    "doc_specificity": {
        "type": "score",
        "instructions": (
            "How specific and verifiable is this documentation? Consider whether it names "
            "concrete commands, endpoints, configuration keys and expected outcomes, rather "
            "than describing things in general terms that a reader must guess at."
        ),
        "criteria": [
            "Generic prose with no concrete commands, keys or endpoints a reader can act on",
            "Some concrete details, but key steps are vague or outdated relative to the "
            "system described elsewhere in these documents",
            "Concrete and mostly accurate: real commands, configuration keys and endpoints, "
            "with only occasional gaps",
            "Precise and verifiable throughout: exact commands, configuration keys, endpoint "
            "paths and expected outcomes a reader can check against",
        ],
    },
}

INFRA_QUESTIONS = {
    "build_reproducibility": {
        "type": "score",
        "instructions": (
            "How reproducible and deterministic is the build and deployment path? Consider "
            "pinned dependencies, pinned base images and toolchain versions, lockfiles, and "
            "whether the same input yields the same artifact."
        ),
        "criteria": [
            "Unpinned and non-deterministic: floating base images or dependency ranges mean "
            "two builds can differ with no record of why",
            "Partially pinned: some inputs are pinned while others float, or the toolchain "
            "version is not fixed",
            "Pinned dependencies, base images and toolchain; a rebuild reproduces the same "
            "artifact",
            "Pinned and verifiable: lockfiles enforced in CI, images referenced by digest or "
            "immutable tag, and the build is hermetic",
        ],
    },
    "deployment_safety": {
        "type": "score",
        "instructions": (
            "How safely can this deployment configuration be operated? Consider health "
            "checks, resource limits, rollout and rollback behaviour, secret injection, and "
            "what happens on failure."
        ),
        "criteria": [
            "Unsafe by omission: no health checks, no resource limits, secrets or privileged "
            "settings embedded, and a failure leaves the system in an unknown state",
            "Basic deployment works, but important safety features such as probes, resource "
            "limits or safe secret handling are missing",
            "Health checks, resource limits and secret injection are in place; rollout "
            "behaviour is predictable",
            "Failures are contained and recoverable: probes, limits, scoped secrets, and a "
            "rollout strategy with a clear rollback path",
        ],
    },
    "secret_hygiene": {
        "type": "score",
        "instructions": (
            "How well do these build and deployment files keep credentials out of the "
            "repository and out of build artifacts? Consider use of build arguments, copied "
            "configuration files, and files that would bake secrets into an image layer."
        ),
        "criteria": [
            "Secrets are written into committed files, or copied into an image in a way that "
            "bakes them permanently into a layer",
            "Secrets mostly come from outside, but some are passed through mechanisms that "
            "leave them in image layers, history or logs",
            "Secrets are injected at runtime rather than build time, with only minor leakage "
            "risk from build arguments or copied files",
            "Secrets are consistently injected at runtime from a secret store, never written "
            "into the repository, image layers, or build logs",
        ],
    },
}

TEST_QUESTIONS = {
    "test_depth": {
        "type": "score",
        "instructions": (
            "How much real behaviour do these tests pin down? Consider whether they assert on "
            "meaningful outcomes such as error paths and edge cases, or merely confirm that "
            "code runs."
        ),
        "criteria": [
            "Tests are absent, or assert only that a call does not crash without checking "
            "any outcome",
            "Happy paths are asserted, but error paths, boundaries and edge cases are not "
            "covered",
            "Main behaviours including some error paths and boundaries are asserted with "
            "meaningful expectations",
            "Behaviour is pinned down thoroughly: error paths, boundaries and edge cases are "
            "asserted, so a regression in behaviour fails a test",
        ],
    },
    "test_breadth": {
        "type": "score",
        "instructions": (
            "How much of this system's surface do these tests reach? Consider how many of the "
            "system's main modules and workflows are exercised at all."
        ),
        "criteria": [
            "No meaningful surface is exercised; the bulk of the system has no test that "
            "touches it",
            "A small fraction of the system is exercised, concentrated in one or two areas "
            "while the rest is untested",
            "A substantial portion of the core workflows is exercised, with some modules or "
            "cross-module paths still untested",
            "Broad coverage across the system's main modules and their integration paths",
        ],
    },
    "test_isolation": {
        "type": "score",
        "instructions": (
            "How isolated and reliable are these tests? Consider dependence on external "
            "services, shared mutable state, ordering assumptions, and whether a failure "
            "here points at a real defect."
        ),
        "criteria": [
            "Tests depend on external services or shared state, so results are flaky or "
            "depend on run order",
            "Mostly isolated, but some tests need a live dependency or shared fixture to "
            "pass",
            "Well isolated and deterministic, with dependencies replaced or scoped per test",
            "Fully hermetic: each test owns its state, no external dependency, and a failure "
            "reliably indicates a real defect",
        ],
    },
}


# --------------------------------------------------------------------------
# State collection
# --------------------------------------------------------------------------
def read_files(patterns, max_lines_per_file=900):
    """Read the given repo-relative paths, expanding globs. Returns (text, meta).

    Patterns without a `!` prefix resolve under backend/src; `!`-prefixed ones
    resolve from the repo root.

    A pattern may end in `@N` to sample the first N and last N matches, used for
    long repetitive families (migrations) where sending all of them would spend
    tokens on near-duplicate text without changing the judgment.
    """
    chunks, meta = [], []
    for pat in patterns:
        sample = None
        if "@" in pat:
            pat, _, n = pat.partition("@")
            sample = int(n)
        matches = sorted(REPO.glob(pat[1:])) if pat.startswith("!") \
            else sorted((REPO / "backend/src").glob(pat))
        if sample and len(matches) > 2 * sample:
            matches = matches[:sample] + matches[-sample:]
        for p in matches:
            if not p.is_file():
                continue
            rel = p.relative_to(REPO)
            try:
                body = p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            lines = body.splitlines()
            truncated = len(lines) > max_lines_per_file
            if truncated:
                body = "\n".join(lines[:max_lines_per_file]) + \
                    f"\n... [truncated: {len(lines) - max_lines_per_file} further lines omitted]"
            chunks.append(f"===== {rel} =====\n{body}")
            meta.append({"path": str(rel), "lines": len(lines), "truncated": truncated})
    return "\n\n".join(chunks), meta


def collect_units():
    payloads = []
    for name, role, patterns, kind in UNITS:
        source, meta = read_files(patterns)
        if not source:
            print(f"  ! {name}: no files matched {patterns}", file=sys.stderr)
            continue
        state = {
            "project": PROJECT_CONTEXT,
            "unit_name": name,
            "unit_role": role,
            "language": "Rust",
            "files": meta,
            "source": source,
        }
        questions = {}
        for qid, spec in SCORE_DIMENSIONS.items():
            questions[qid] = {"type": "score", **spec}
        for qid, text in NOUL_FLAGS.items():
            questions[qid] = {"type": "noul", "instructions": text}
        payloads.append({"id": name, "role": role, "state": state,
                         "questions": questions, "files": meta})
    return payloads


def collect_cross():
    payloads = []

    # SECURITY.md and CODE_OF_CONDUCT.md are stock GitHub scaffold templates
    # (Contributor Covenant, default policy table); including them would inflate
    # the completeness judgment with prose the team did not write.
    docs, doc_meta = read_files([
        "!README.md", "!README.zh-CN.md", "!CHANGELOG.md", "!CONTRIBUTING.md",
        "!docs/architecture.md", "!docs/deployment.md",
    ])
    tree = []
    for p in sorted((REPO / "backend/src").rglob("*.rs")):
        tree.append(f"{p.relative_to(REPO)} ({len(p.read_text(errors='replace').splitlines())} lines)")
    payloads.append({
        "id": "documentation", "role": "Project documentation",
        "state": {
            "project": PROJECT_CONTEXT,
            "documents": docs,
            "source_tree": "\n".join(tree),
            "dependency_manifest": (REPO / "backend/Cargo.toml").read_text(),
            "ci_config": (REPO / ".github/workflows/backend.yml").read_text(),
        },
        "questions": DOC_QUESTIONS,
        "files": doc_meta,
    })

    infra, infra_meta = read_files([
        "!docker/desktop/Dockerfile", "!docker/desktop/Dockerfile-next1",
        "!docker/desktop/Dockerfile-next2", "!docker/sidecar/Dockerfile",
        "!docker/desktop/entrypoint.sh", "!docker/sidecar/entrypoint.sh",
        "!docker/sidecar/stream-ctrl.py", "!docker/sidecar/mediamtx.yml",
        "!docker/desktop/install.sh", "!docker/desktop/run-local.sh",
        "!.github/workflows/backend.yml",
        "!deploy/test/deployment.yaml", "!deploy/test/service.yaml",
        "!deploy/test/init.sql",
    ])
    payloads.append({
        "id": "infrastructure", "role": "Build, container and deployment configuration",
        "state": {"project": PROJECT_CONTEXT, "configuration": infra},
        "questions": INFRA_QUESTIONS,
        "files": infra_meta,
    })

    # Existing tests: everything from the first `#[cfg(test)]` onward, per file.
    test_chunks, test_meta = [], []
    for p in sorted((REPO / "backend/src").rglob("*.rs")):
        body = p.read_text(errors="replace")
        m = re.search(r"^#\[cfg\(test\)\]", body, re.M)
        if not m:
            continue
        rel = p.relative_to(REPO)
        chunk = body[m.start():]
        test_chunks.append(f"===== {rel} =====\n{chunk}")
        test_meta.append({"path": str(rel), "lines": len(chunk.splitlines())})
    payloads.append({
        "id": "tests", "role": "Existing automated tests",
        "state": {
            "project": PROJECT_CONTEXT,
            "note": "These are all the test modules in the repository. The rest of the "
                    "codebase has no tests at all.",
            "tests": "\n\n".join(test_chunks) or "(no tests exist)",
        },
        "questions": TEST_QUESTIONS,
        "files": test_meta,
    })
    return payloads


# --------------------------------------------------------------------------
# Deterministic metrics
# --------------------------------------------------------------------------
PANIC = re.compile(r"\.unwrap\(\)|\.expect\(|panic!|unreachable!|todo!|unimplemented!")
OBS = re.compile(r"tracing::(info|warn|error|debug|trace)!|\blog::")


def metrics():
    src = REPO / "backend/src"
    per_unit, totals = {}, {"loc": 0, "files": 0, "panics": 0, "tests": 0, "unsafe": 0}
    for name, role, patterns, kind in UNITS:
        loc = panics = tests = unsafe = obs = 0
        n_files = 0
        for pat in patterns:
            # metrics always cover every file; the `@N` sampling only limits what
            # gets sent to the model, never what gets counted.
            for p in sorted(src.glob(pat.partition("@")[0])):
                body = p.read_text(errors="replace")
                n_files += 1
                prod = body.split("#[cfg(test)]")[0]
                loc += len(body.splitlines())
                panics += len(PANIC.findall(prod))
                tests += len(re.findall(r"#\[(?:tokio::)?test\]|#\[cfg\(test\)\]", body))
                unsafe += len(re.findall(r"\bunsafe\b", prod))
                obs += len(OBS.findall(prod))
        per_unit[name] = {"loc": loc, "files": n_files, "panics": panics,
                          "tests": tests, "unsafe": unsafe, "observability_calls": obs,
                          "role": role}
        for k in ("loc", "files", "panics", "tests", "unsafe"):
            totals[k] += per_unit[name][k]

    untested = [n for n, m in per_unit.items() if m["tests"] == 0]
    return {
        "per_unit": per_unit,
        "totals": {**totals,
                   "units": len(UNITS),
                   "units_with_no_tests": len(untested),
                   "untested_units": untested,
                   "panic_sites_per_kloc": round(totals["panics"] / max(totals["loc"], 1) * 1000, 1),
                   "tests_per_kloc": round(totals["tests"] / max(totals["loc"], 1) * 1000, 1),
                   "ci": "cargo fmt --check, cargo clippy (advisory, not -D warnings), "
                         "cargo build --locked, cargo test --locked"},
    }


# --------------------------------------------------------------------------
# API
# --------------------------------------------------------------------------
def ask(payload, attempts=4):
    body = json.dumps({
        "state": payload["state"] if isinstance(payload["state"], str)
        else json.dumps(payload["state"], ensure_ascii=False),
        "model": MODEL,
        "questions": payload["questions"],
    }).encode()
    req = urllib.request.Request(
        ENDPOINT, data=body,
        headers={"Authorization": f"Bearer {os.environ['TYPESAFE_API_KEY']}",
                 "Content-Type": "application/json"},
    )
    delay = 2.0
    for i in range(attempts):
        try:
            with urllib.request.urlopen(req, timeout=180) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            detail = e.read().decode()[:400]
            if e.code in (429, 500, 502, 503, 504) and i < attempts - 1:
                time.sleep(delay)
                delay *= 2
                continue
            return {"error": f"HTTP {e.code}: {detail}"}
        except Exception as e:  # noqa: BLE001
            if i < attempts - 1:
                time.sleep(delay)
                delay *= 2
                continue
            return {"error": f"{type(e).__name__}: {e}"}
    return {"error": "exhausted retries"}


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dry-run", action="store_true",
                    help="collect units and metrics, write metrics.json, make no API calls")
    ap.add_argument("--jobs", type=int, default=4, help="concurrent API requests (default 4)")
    args = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    print(f"repo: {REPO}\nout:  {OUT}")

    payloads = collect_units() + collect_cross()
    m = metrics()
    (OUT / "metrics.json").write_text(json.dumps(m, indent=2, ensure_ascii=False))
    print(f"  {len(payloads)} units, {m['totals']['loc']} LOC, "
          f"{m['totals']['panics']} panic sites, "
          f"{m['totals']['units_with_no_tests']}/{m['totals']['units']} units untested")
    print(f"  wrote {OUT / 'metrics.json'}")

    if args.dry_run:
        est = sum(len(json.dumps(p["state"], ensure_ascii=False)) for p in payloads) // 4
        print(f"  dry run: would spend roughly {est:,} input tokens; no API calls made")
        return

    if "TYPESAFE_API_KEY" not in os.environ:
        sys.exit("TYPESAFE_API_KEY is not set (get a key at https://console.typesafe.ai/keys)")

    print(f"calling TypeSafe ({MODEL})...")
    results = {}

    def run(p):
        started = time.time()
        resp = ask(p)
        dt = time.time() - started
        status = "ERR " + resp["error"][:80] if "error" in resp else "ok"
        print(f"  {p['id']:<16} {dt:5.1f}s  {status}")
        return p["id"], resp

    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        for uid, resp in ex.map(run, payloads):
            results[uid] = resp

    (OUT / "raw.json").write_text(json.dumps({
        "model": MODEL,
        "units": {p["id"]: {"role": p["role"], "files": p["files"],
                            "questions": p["questions"]} for p in payloads},
        "responses": results,
    }, indent=2, ensure_ascii=False))

    tin = sum(r.get("usage", {}).get("input_tokens", 0) for r in results.values())
    tout = sum(r.get("usage", {}).get("output_tokens", 0) for r in results.values())
    errs = [k for k, v in results.items() if "error" in v]
    print(f"tokens: in={tin} out={tout}  errors={errs or 'none'}")
    print(f"wrote {OUT / 'raw.json'} — now run scorecard.py")


if __name__ == "__main__":
    main()
