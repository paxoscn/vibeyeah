# Quality scorecard

Scores this repository's code quality across eight dimensions using
[TypeSafe](https://docs.typesafe.ai) System One (Jev) judgments, and composes them
into a weighted scorecard with hard gates.

## Why it is split in two

The core design rule is **code computes facts, the model supplies judgment**:

| Handled by | What | Why |
|---|---|---|
| Code | LOC, panic sites, test counts, `unsafe`, CI config, file inventory | Anything countable should be counted, not guessed at. A model asked "how many `unwrap()` calls are here" will approximate. |
| Jev | Error handling, readability, modularity, security posture, testability | These need semantic understanding of *what the code is trying to do*. |

Both feed the same scorecard, and the deterministic half doubles as a check on the
model's half. When they disagree, that is the interesting case.

## Scoring model

Eight dimensions, each normalized to 0–1:

- **Five unit-level** (`error_handling`, `readability`, `modularity`, `security`,
  `testability`) are Jev `Score` questions asked once per module, so you get a
  per-module heatmap as well as a project number.
- **Three cross-cutting**: `documentation` (README/docs), `infrastructure`
  (Dockerfiles, CI, manifests), `test_coverage` (deterministic breadth × judged depth).

`scorecard.py` normalizes each to 0–1, takes the mean across modules, and combines
them with the weights in `WEIGHTS`. Units are unweighted by size by default so
boilerplate cannot dominate; a LOC-weighted figure is printed alongside as a
sensitivity check, along with the headline under three alternative weightings.

Separately from the weighted score, **gates** are hard conditions (`SEC-1`, `TEST-1`,
`DEP-1`). A weighted average is compensating — a strong README can hide a command
injection — so anything disqualifying is reported on its own and never averaged in.

## Usage

```bash
export TYPESAFE_API_KEY=...            # https://console.typesafe.ai/keys

# 1. Collect, score, persist raw judgments. Spends tokens (~122k in / 2k out for
#    the whole backend at the time of writing).
python3 tools/quality-scorecard/harness.py

# Inspect payload sizes and deterministic metrics without calling the API:
python3 tools/quality-scorecard/harness.py --dry-run

# 2. Compose the scorecard. Free, and repeatable.
python3 tools/quality-scorecard/scorecard.py
```

Artifacts land in `out/` (gitignored):

- `raw.json` — every judgment, as returned. The cache.
- `metrics.json` — deterministic metrics.
- `scorecard.json` — the composed result.

**Re-tuning weights does not re-run inference.** Change `WEIGHTS` in `scorecard.py`
and re-run it. Only re-run `harness.py` when the code or the questions change.

## Reading the output honestly

TypeSafe's own docs are explicit that *"typed output guarantees the interface, not
truth"*. Two failure modes showed up on the first run and are worth knowing:

- **Lexical false positives.** Jev flagged `hardcoded_secret` on `lark_app_secret`
  and `image_pull_secret` — database *column names*, not credentials. It reads shape,
  not provenance. `CORRECTIONS` in `scorecard.py` records each such judgment that was
  checked by hand and overridden, with the evidence, and `raw.json` keeps the original.
- **Low confidence means something.** `confidence` below 0.50 is printed as a
  human-review list; it usually means the levels overlapped or the module was too
  mixed to place cleanly.

Conversely, the model was *strong* where the question required tracing a value across
files — it caught the `sync.rs` shell injection at P(yes)=0.92, which a lexical check
would not have found. Treat the judgments as a well-calibrated triage signal to verify,
not as a verdict.

## Caveats

- **`CORRECTIONS` and `gates` are pinned to a revision.** They cite specific lines and
  findings. Re-verify them when the code moves, or they will quietly go stale.
- Unit boundaries and dimension weights are defaults, not a ground truth. The module
  split is in `UNITS`; adjust it if it stops matching how the code is organised.
- `data_layer` sends only a sample of migrations (near-duplicate boilerplate); the
  deterministic metrics still count all of them.
- `docker/desktop/configs/.hermes/` is excluded — it is vendored third-party config,
  not code this team wrote.

## Adding a dimension

1. Add a `Score` entry to `SCORE_DIMENSIONS` in `harness.py` (levels must be concrete
   situations, not degrees — the model sees each level alone, never its position).
2. Add it to `DIMS` and `WEIGHTS` in `scorecard.py`.
3. Re-run `harness.py` so `raw.json` contains the new answer.
