# epigrep — release runbook & gate

This is the durable runbook for cutting an epigrep release. **Nothing in the
"gated" section below should be run by an agent.** Publication and tagging each
require Rikk's explicit authorisation.

`0.1.0` shipped on **2026-06-18** and is live on PyPI: the public install path is
plain `pip install epigrep`. This document now describes how to cut *the next*
release from that baseline; the historical pre-1.0 gate notes have been folded
into "Settled decisions".

---

## Settled decisions (no action needed)

- **Name** — `epigrep` is final and reserved on PyPI: distribution name, import
  package name, and GitHub repo (`git@github.com:rikkhill/epigrep.git`) all
  agree.
- **License** — MIT (`License-Expression: MIT`), `LICENSE` at repo root; a
  byte-identical copy at `crates/epigrep-py/LICENSE` is what makes the sdist
  pass PyPI's PEP 639 check. CI guards the two against drift.
- **Posture** — confirmed open source. MIT core; the repo is public
  (`https://github.com/rikkhill/epigrep`).
- **Stable surfaces** — the Python API (`match` / `explain` / `schema`, the
  builder reached via `Pattern.event(...)`, and `Event` / `Pattern` / `Match` /
  `NearMiss`) and the JSON AST (`pattern_from_json` / `Pattern.to_json`).
  `parse_pattern` / the text DSL stay importable but **provisional**, outside the
  stability guarantee. The package ships `py.typed` and a `_core.pyi` stub.
- **Publishing** — trusted publishing (OIDC), not long-lived tokens. Pending
  publishers on PyPI and TestPyPI are configured and exercised. No tokens or
  secrets are stored anywhere.
- **Wheel matrix** — `.github/workflows/release.yml` builds Linux x86_64/aarch64,
  macOS arm64 (Apple Silicon), Windows x64 wheels (abi3, one wheel per platform
  covers Python ≥3.9), plus sdist. **Intel macOS (x86_64) is sdist-only**: the
  hosted `macos-13` runner proved effectively unavailable (a queued job is not
  rescued by `timeout-minutes` / `continue-on-error` and silently blocks the
  publish job). Intel Macs build from the sdist (needs a Rust toolchain).

---

## Cutting the next release

### 1. Decide the version

Pick the next version per semver. Pre-releases use a hyphen in semver
(`0.2.0-rc1`) and no hyphen in PEP 440 (`0.2.0rc1`); the two notations must
normalise to the same thing — `scripts/check_version_sync.py` enforces this.

### 2. Bump version in lockstep (agent-safe)

Update **all** of:

- `crates/epigrep-py/pyproject.toml` — `version` (PEP 440)
- `crates/epigrep-py/Cargo.toml` — `version` (semver)
- `Cargo.lock` — regenerate so the resolved `epigrep-py` entry matches
  (`cargo update -p epigrep-py --precise <ver>`, or any `cargo build`)

Then confirm they agree:

```sh
python3 scripts/check_version_sync.py   # checks pyproject, Cargo.toml, Cargo.lock
```

### 3. Update the changelog (agent-safe)

Move the accumulated entries under `## [Unreleased]` in `CHANGELOG.md` into a new
`## [x.y.z] — YYYY-MM-DD` section, and refresh the compare/link footnotes at the
bottom. The GitHub Release notes can be drawn from this section.

### 4. Verification block (safe to run anytime, no publication)

Run from the repo root. None of this leaves the machine.

```sh
# 1. Repo clean and at the intended commit
git status --porcelain        # expect empty
git rev-parse HEAD            # record this as the verified commit

# 2. Quality gates (mirror CI)
python3 scripts/check_version_sync.py
diff LICENSE crates/epigrep-py/LICENSE
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy -p epigrep-py --features extension-module -- -D warnings
cargo test
cargo bench --no-run

# 3. Build artifacts
rm -rf dist
maturin build --release --manifest-path crates/epigrep-py/Cargo.toml --out dist
maturin sdist            --manifest-path crates/epigrep-py/Cargo.toml --out dist

# 4. Metadata validation (official tool)
python -m pip install --quiet twine
twine check dist/*       # expect PASSED for wheel + sdist

# 5. LICENSE + typing markers present in the wheel
python - <<'PY'
import tarfile, zipfile
from pathlib import Path
w = next(Path("dist").glob("epigrep-*.whl"))
s = next(Path("dist").glob("epigrep-*.tar.gz"))
with zipfile.ZipFile(w) as a:
    names = a.namelist()
    assert any(n.endswith(".dist-info/licenses/LICENSE") for n in names), names
    assert any(n.endswith("epigrep/py.typed") for n in names), names
    assert any(n.endswith("epigrep/_core.pyi") for n in names), names
with tarfile.open(s) as a:
    assert any(n.endswith("/LICENSE") for n in a.getnames())
print("LICENSE + typing markers present")
PY

# 6. Fresh-venv offline install smoke + run an example
TMP=$(mktemp -d); python3 -m venv "$TMP/v"
"$TMP/v/bin/pip" install --no-index --find-links dist epigrep
"$TMP/v/bin/python" -c "import epigrep; print('import ok')"
"$TMP/v/bin/python" examples/logs-first/run.py
rm -rf "$TMP"
```

### Published-state check (read-only)

```sh
curl -s https://pypi.org/pypi/epigrep/json | python3 -c "import sys,json;print(json.load(sys.stdin)['info']['version'])"
```

CI (GitHub Actions) runs version-sync + LICENSE checks, fmt, clippy (core +
bindings), tests, bench compile, wheel + sdist build, artifact LICENSE
inspection, wheel install, and pytest on Python 3.9 + 3.12. The Release workflow
builds the wheel matrix + sdist and publishes via trusted publishing on a `v*`
tag push (PyPI) or manual dispatch (TestPyPI by default).

---

## GATED — publishing (Rikk only; irreversible, outward-facing)

After the bump + changelog commit is pushed and CI is green:

```sh
# Optional TestPyPI rehearsal first:
gh workflow run release.yml -f index=testpypi

# Publish: the tag push triggers the wheel-matrix build + PyPI publish (OIDC).
git tag -a vX.Y.Z -m "epigrep X.Y.Z"
git push origin vX.Y.Z
```

Then cut a **GitHub Release** for `vX.Y.Z` with notes drawn from `CHANGELOG.md`.

### Rollback / yank notes

- **PyPI/TestPyPI**: a version cannot be deleted and reused. You can **yank** it
  (web UI or API); `pip` then skips it for new installs unless pinned. Fix
  forward with a new version rather than deleting.
- **Git tag**: `git push --delete origin vX.Y.Z` (and `git tag -d vX.Y.Z`)
  removes a tag pushed in error before it has triggered a publish.
- **Repo visibility**: public; assume anything pushed is cloned/cached.
