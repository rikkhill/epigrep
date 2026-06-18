# epigrep — release gate & PyPI readiness

This is the human-gated checklist for epigrep's 0.1 release. **Nothing in the
"gated" sections below should be run by an agent.** Each gated step requires
Rikk's explicit authorisation.

The current state was verified by a post-RC release-readiness pass on
**2026-06-17** against `main` at commit `1e7eacc`. epigrep is **published as a
release candidate** — `0.1.0rc2` is live on PyPI and TestPyPI — and the only
remaining gate is the decision to promote it to a final `0.1.0`. Re-run the
verification block before acting on the gated step.

---

## Settled decisions (no action needed)

- **Name** — `epigrep` is final and **reserved on PyPI**: distribution name,
  import package name, and GitHub repo (`git@github.com:rikkhill/epigrep.git`)
  all agree.
- **License** — MIT (`License-Expression: MIT`), `LICENSE` at repo root.
- **Posture** — **confirmed open source.** MIT core; the repo is **public**
  (`https://github.com/rikkhill/epigrep`).
- **Stable surfaces** — Python API (`match` / `explain` / `schema`, builder,
  `Event` / `Pattern` / `Match` / `NearMiss`) and the JSON AST. `parse_pattern`
  / the text DSL stay importable but **provisional**, outside the 0.1 guarantee.
- **Publishing** — **trusted publishing (OIDC)**, not long-lived tokens. The
  pending publishers on PyPI and TestPyPI are configured and have been exercised
  (rc1 + rc2 published through them). No tokens or secrets are stored anywhere.
- **Wheel matrix** — multi-platform build approved and implemented
  (`.github/workflows/release.yml`): Linux x86_64/aarch64, macOS **arm64**
  (Apple Silicon), Windows x64 (abi3, one wheel per platform covers Python
  ≥3.9), plus sdist. **Intel macOS (x86_64) is sdist-only**: the hosted
  `macos-13` runner is effectively unavailable (a 2026-06-17 experiment left the
  job queued 2h+ without ever being assigned a runner — and a queued job is not
  rescued by `timeout-minutes` / `continue-on-error`, so it silently blocks the
  publish job). Intel Macs build from the sdist (needs a Rust toolchain); a wheel
  would need cross-compilation, not a hosted Intel runner.
- **Versioning** — release-candidate track. Current published version is
  **`0.1.0rc2`** (`crates/epigrep-py/pyproject.toml`). `pip` does not install a
  pre-release by default, so `--pre` is required to get it.

---

## Verification block (safe to run anytime, no publication)

Run from the repo root. None of this leaves the machine.

```sh
# 1. Repo clean and at the intended commit
git status --porcelain        # expect empty
git rev-parse HEAD            # record this as the verified commit

# 2. Quality gates (mirror CI)
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

# 5. LICENSE present in both artifacts
#    wheel -> epigrep-<v>.dist-info/licenses/LICENSE
#    sdist -> epigrep-<v>/LICENSE   (at the sdist root, via PEP 639 license-files)
python - <<'PY'
import tarfile, zipfile
from pathlib import Path
w = next(Path("dist").glob("epigrep-*.whl"))
s = next(Path("dist").glob("epigrep-*.tar.gz"))
with zipfile.ZipFile(w) as a:
    assert any(n.endswith(".dist-info/licenses/LICENSE") for n in a.namelist())
with tarfile.open(s) as a:
    assert any(n.endswith("/LICENSE") for n in a.getnames())
print("LICENSE present in wheel + sdist")
PY

# 6. Fresh-venv offline install smoke + run an example
TMP=$(mktemp -d); python3 -m venv "$TMP/v"
"$TMP/v/bin/pip" install --no-index --find-links dist epigrep
"$TMP/v/bin/python" -c "import epigrep; print('import ok')"
"$TMP/v/bin/python" examples/logs-first/run.py
rm -rf "$TMP"
```

### Published-state check (read-only)

epigrep is already published, so these now return the live RC rather than a
`404`. This is a read against the official JSON API and uploads nothing.

```sh
curl -s https://pypi.org/pypi/epigrep/json      | python3 -c "import sys,json;print(json.load(sys.stdin)['info']['version'])"   # 0.1.0rc2
curl -s https://test.pypi.org/pypi/epigrep/json | python3 -c "import sys,json;print(json.load(sys.stdin)['info']['version'])"   # 0.1.0rc2
```

A clean-environment install of the published RC (the public install path):

```sh
python -m pip install --pre epigrep        # resolves 0.1.0rc2 from PyPI
```

#### State as of 2026-06-17 (`main` @ `1e7eacc`)

| Check | Result |
|---|---|
| `git status` clean | ✅ |
| `twine check` wheel + sdist | ✅ PASSED |
| LICENSE in wheel + sdist | ✅ |
| Fresh-venv offline install + example run | ✅ |
| `pip install --pre epigrep` from PyPI + example run | ✅ |
| Metadata name = `epigrep`, License = MIT, Requires-Python ≥3.9 | ✅ |
| PyPI `epigrep` | published — latest `0.1.0rc2` (rc1, rc2) |
| TestPyPI `epigrep` | published — latest `0.1.0rc2` (rc1, rc2) |

CI (GitHub Actions) runs fmt, clippy (core + bindings), tests, bench compile,
wheel + sdist build, artifact LICENSE inspection, wheel install, and pytest on
Python 3.9 + 3.12. The Release workflow builds the wheel matrix + sdist and
publishes via trusted publishing on a `v*` tag push (PyPI) or manual dispatch
(TestPyPI by default).

---

## Done

- **G1 — repo public.** `https://github.com/rikkhill/epigrep` is public.
- **Wheel matrix + release workflow.** `.github/workflows/release.yml` builds
  Linux x86_64/aarch64, macOS arm64, Windows x64 wheels (abi3) + sdist (Intel
  macOS is sdist-only), and publishes via trusted publishing.
- **G2 — trusted publishers configured** on PyPI and TestPyPI (environments
  `pypi` / `testpypi`); exercised by the rc1 and rc2 publishes.
- **G3 — TestPyPI rehearsal** run and verified.
- **G4 — RC published.** `0.1.0rc1` then `0.1.0rc2` are live on PyPI (name
  reserved) and TestPyPI; `pip install --pre epigrep` resolves rc2.

## GATED — remaining step

### G5. Promote rc2 → final 0.1.0 (Rikk — irreversible, outward-facing)

The release candidate is functionally and packaging-sound and is sufficient to
promote (see `projects:epigrep:release-readiness-2026-06-17` in OB). Promotion is
the one remaining human-gated step. The recommended sequence:

```sh
# 1. Bump versions and sweep RC / --pre prose to plain install instructions,
#    then tag — this is what /publish-pypi 0.1.0 automates:
#      pyproject 0.1.0rc2 -> 0.1.0 ; Cargo 0.1.0-rc2 -> 0.1.0
#      README, PyPI README, docs/{index,getting-started,limitations}.md:
#        "release candidate" / "pip install --pre epigrep" -> "pip install epigrep"
#      twine check ; TestPyPI rehearsal ; confirm
git tag -a v0.1.0 -m "epigrep 0.1.0"
git push origin v0.1.0          # tag push triggers build + PyPI publish (OIDC)
```

Then cut a **GitHub Release** for `v0.1.0` with notes (the Releases page is
currently empty).

### Rollback / yank notes

- **PyPI/TestPyPI**: a version cannot be deleted and reused. You can **yank** it
  (project web UI or API); `pip` then skips it for new installs unless pinned.
  Fix forward with a new version rather than deleting.
- **Git tag**: `git push --delete origin v0.1.0` (and `git tag -d v0.1.0`)
  removes a tag pushed in error before it has triggered a publish.
- **Repo visibility**: public; assume anything pushed is cloned/cached.

---

## Remaining human gates (summary)

1. **G5** — decide to promote `0.1.0rc2` → final `0.1.0`, then tag `v0.1.0` to
   publish and cut the GitHub Release.
