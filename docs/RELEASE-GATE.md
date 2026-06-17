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
- **Posture** — **confirmed open source.** MIT core; the repo is now **public**
  (`https://github.com/rikkhill/epigrep`).
- **Stable surfaces** — Python API (`match` / `explain` / `schema`, builder,
  `Event` / `Pattern` / `Match` / `NearMiss`) and the JSON AST. `parse_pattern`
  / the text DSL stay importable but **provisional**, outside the 0.1 guarantee.

### Release decisions confirmed (2026-06-17, by Rikk)

- **Name reservation** — authorised. `epigrep` is reserved on first trusted
  publish (no separate placeholder upload).
- **Publishing** — **trusted publishing (OIDC)**, not long-lived tokens.
- **Wheel matrix** — multi-platform build approved and implemented
  (`.github/workflows/release.yml`): Linux x86_64/aarch64, macOS x86_64/arm64,
  Windows x64 (abi3, one wheel per platform covers Python ≥3.9), plus sdist.
- **Versioning** — release-candidate track. Version is now **`0.1.0rc1`**
  (`crates/epigrep-py/pyproject.toml`).

The only remaining step before a publish can succeed is the **PyPI-side trusted
publisher configuration**, which requires Rikk's PyPI/TestPyPI account — see
"PyPI trusted publisher setup" below.

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

## Done

- **G1 — repo public.** `https://github.com/rikkhill/epigrep` is public.
- **Wheel matrix + release workflow.** `.github/workflows/release.yml` builds
  Linux x86_64/aarch64, macOS x86_64/arm64, Windows x64 wheels (abi3) + sdist,
  and publishes via trusted publishing.
- **RC versioning.** `0.1.0rc1`.

## GATED — the remaining steps to publish

### G2. PyPI trusted publisher setup (Rikk — needs PyPI account)

This is the one step the repo cannot do for itself. On each index, add a
**pending publisher** (Account → Publishing on PyPI; the project need not exist
yet — the name is reserved on first publish):

| Field | Value |
|---|---|
| PyPI Project Name | `epigrep` |
| Owner | `rikkhill` |
| Repository name | `epigrep` |
| Workflow name | `release.yml` |
| Environment name | `pypi` (on pypi.org) / `testpypi` (on test.pypi.org) |

- TestPyPI publisher: <https://test.pypi.org/manage/account/publishing/>
- PyPI publisher: <https://pypi.org/manage/account/publishing/>

The workflow's `environment:` names (`pypi`, `testpypi`) must match exactly. No
tokens or secrets are stored anywhere — OIDC handles auth at publish time.

### G3. TestPyPI rehearsal (recommended first)

Once the TestPyPI publisher exists, run the workflow manually against TestPyPI:

```sh
gh workflow run release.yml -f index=testpypi
```

Then confirm a clean install resolves (TestPyPI is a separate registry; the name
is reserved there only):

```sh
python -m pip install --index-url https://test.pypi.org/simple/ \
  --extra-index-url https://pypi.org/simple/ epigrep==0.1.0rc1
```

### G4. PyPI publish (reserves the name, ships the RC)

Once the PyPI publisher exists and the rehearsal looks right, publish by tagging
the RC commit — the tag push triggers the build + PyPI publish:

```sh
git tag -a v0.1.0rc1 -m "epigrep 0.1.0rc1"
git push origin v0.1.0rc1
```

(Or `gh workflow run release.yml -f index=pypi` for a tagless dispatch.) `pip`
will not install a pre-release by default, so `0.1.0rc1` is a safe first publish.

### G5. Rollback / yank notes

- **PyPI/TestPyPI**: a version cannot be deleted and reused. You can **yank** it
  (via the project web UI or API); `pip` then skips it for new installs unless
  pinned. Fix forward with a new version (`0.1.0rc2`, then `0.1.0`) rather than
  deleting.
- **Git tag**: `git push --delete origin v0.1.0rc1` (and `git tag -d v0.1.0rc1`)
  removes a tag pushed in error before it has triggered a publish.
- **Repo visibility**: already public; assume anything pushed is cloned/cached.

---

## Remaining human gates (summary)

1. **G2** — add the PyPI + TestPyPI trusted publishers (needs Rikk's accounts).
2. **G3** — run the TestPyPI rehearsal and check the install.
3. **G4** — tag `v0.1.0rc1` to publish to PyPI (reserves the name, ships the RC).
