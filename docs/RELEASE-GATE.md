# epigrep — release gate & PyPI readiness

This is the human-gated checklist for taking epigrep from a clean local 0.1
release candidate to a published package. **Nothing in the "gated" sections
below should be run by an agent.** Each gated step requires Rikk's explicit
authorisation.

The state below was verified by a release-readiness pass on **2026-06-17**
against commit `a0bdb00` (`Add logs-first RC examples and package smoke`).
Re-run the verification block before acting on any gated step, and re-pin the
commit hash if it has moved.

---

## Settled decisions (no action needed)

- **Name** — `epigrep` is final: distribution name, import package name, and
  GitHub repo (`git@github.com:rikkhill/epigrep.git`) all agree.
- **License** — MIT (`License-Expression: MIT`), `LICENSE` at repo root.
- **Posture (planning)** — MIT open-source core; repo stays **private** until
  the 0.1 RC is clean, then flip public. The flip and any upload are gated.
- **Stable surfaces** — Python API (`match` / `explain` / `schema`, builder,
  `Event` / `Pattern` / `Match` / `NearMiss`) and the JSON AST. `parse_pattern`
  / the text DSL stay importable but **provisional**, outside the 0.1 guarantee.

---

## Verification block (safe to run anytime, no publication)

Run from the repo root. None of this leaves the machine.

```sh
# 1. Repo clean and at the intended RC commit
git status --porcelain        # expect empty
git rev-parse HEAD            # record this as the RC commit

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
#    sdist -> epigrep-<v>/crates/epigrep-py/LICENSE
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

### Name availability (read-only, no upload)

A `404` means the name is unregistered/available. This is a read against the
official JSON API and does **not** reserve or publish anything.

```sh
curl -s -o /dev/null -w "%{http_code}\n" https://pypi.org/pypi/epigrep/json       # 404 = free
curl -s -o /dev/null -w "%{http_code}\n" https://test.pypi.org/pypi/epigrep/json  # 404 = free
```

> Caveat: a `404` proves the name is **not currently registered**; it cannot
> reserve it. The name is only secured by the first successful upload, which is
> gated below. Availability cannot be guaranteed between this check and the
> upload without actually uploading.

#### State as of 2026-06-17 (commit `a0bdb00`)

| Check | Result |
|---|---|
| `git status` clean, at `a0bdb00` | ✅ |
| `twine check` wheel + sdist | ✅ PASSED |
| LICENSE in wheel + sdist | ✅ |
| Fresh-venv offline install + example run | ✅ |
| Metadata name = `epigrep`, License = MIT, Requires-Python ≥3.9 | ✅ |
| PyPI `epigrep` | 404 (available) |
| TestPyPI `epigrep` | 404 (available) |

CI (GitHub Actions) already runs fmt, clippy (core + bindings), tests, bench
compile, wheel + sdist build, artifact LICENSE inspection, wheel install, and
pytest on Python 3.9 + 3.12.

---

## GATED steps — require Rikk's explicit authorisation

### G1. Flip repo to public

Do this only once the RC is clean and Rikk confirms open-source posture. An
sdist publishes source, so the repo and the package expose the same code.

```sh
gh repo edit rikkhill/epigrep --visibility public --accept-visibility-change-consequences
```

Before flipping: re-scan history for anything that should not be public
(secrets, tokens, local-only paths, private data). `git log -p` / `gh secret
list` / a `gitleaks`-style scan are reasonable pre-flip checks.

### G2. Version & tag

`version` lives in `crates/epigrep-py/pyproject.toml` (currently `0.1.0`). A
PyPI version is **permanent** — it can be yanked but never reused. Prefer an RC
on TestPyPI first (e.g. `0.1.0rc1`).

```sh
# bump pyproject version if needed, commit, then tag the RC commit
git tag -a v0.1.0 -m "epigrep 0.1.0"
git push origin v0.1.0
```

### G3. TestPyPI rehearsal (do before real PyPI)

Manual-token path (token entered interactively or via env, never committed):

```sh
# build is already in dist/ from the verification block
twine upload --repository testpypi dist/*
# then verify a clean install resolves from TestPyPI:
python -m pip install --index-url https://test.pypi.org/simple/ epigrep
```

> TestPyPI is a separate registry with separate accounts/tokens. A successful
> upload here reserves the name **on TestPyPI only**, not on real PyPI.

### G4. PyPI publication

Preferred: **trusted publishing (OIDC)** via GitHub Actions — no long-lived
tokens. Configure a PyPI "pending publisher" for project `epigrep`, repo
`rikkhill/epigrep`, workflow filename, and environment, then add a release
job, e.g.:

```yaml
# .github/workflows/release.yml (sketch — add when authorised)
on:
  release:
    types: [published]
jobs:
  publish:
    runs-on: ubuntu-latest
    environment: pypi
    permissions:
      id-token: write          # OIDC for trusted publishing
    steps:
      - uses: actions/checkout@v4
      # build wheels (cibuildwheel / maturin-action across platforms) + sdist into dist/
      - uses: pypa/gh-action-pypi-publish@release/v1
```

Manual fallback (only if trusted publishing is not set up):

```sh
twine upload dist/*          # uses a PyPI API token; never commit the token
```

> Note: the wheel built locally is macOS/arm64 only. A real release needs the
> multi-platform wheel matrix (Linux manylinux, macOS x86_64+arm64, Windows)
> built in CI via `maturin-action` / `cibuildwheel`, plus the sdist. Add that
> matrix as part of G4.

### G5. Rollback / yank notes

- **PyPI/TestPyPI**: you cannot delete-and-reuse a version. You can **yank**
  (`pip` won't pick a yanked version for new installs unless pinned):
  via the project's web UI, or `twine`/API. Fix forward with a new version
  (`0.1.1`) rather than deleting.
- **Git tag**: `git push --delete origin v0.1.0` (and `git tag -d v0.1.0`)
  removes a tag pushed in error, *before* it is referenced by a published
  release.
- **Repo visibility**: a public flip can be reverted (`gh repo edit
  --visibility private ...`), but assume anything published was cloned/cached;
  treat the flip as irreversible for secret-exposure purposes.

---

## Remaining human gates (summary)

1. Confirm open-source posture → **G1** repo flip.
2. Authorise name reservation via first upload → **G3** TestPyPI, then **G4**.
3. Decide trusted-publishing vs manual-token for **G4**.
4. Approve the multi-platform wheel matrix addition for a real release.
