#!/usr/bin/env python3
"""Fail if the epigrep-py version is declared inconsistently.

The package version lives in three places that must agree:

  * ``crates/epigrep-py/pyproject.toml`` — PEP 440 (e.g. ``0.1.0rc2``)
  * ``crates/epigrep-py/Cargo.toml``     — semver    (e.g. ``0.1.0-rc2``)
  * ``Cargo.lock``                       — semver, the resolved ``epigrep-py``
    entry (what an offline/locked build actually compiles)

maturin takes the wheel version from ``pyproject.toml``; the crate version is
the Cargo one; ``Cargo.lock`` records the resolved version and drifts if it is
not regenerated after a ``Cargo.toml`` bump. The two notations differ for
pre-releases (PEP 440 ``0.1.0rc2`` vs semver ``0.1.0-rc2``), so we compare a
normalised form rather than the raw strings. Run with no arguments; exits
non-zero on mismatch.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYPROJECT = ROOT / "crates" / "epigrep-py" / "pyproject.toml"
CARGO = ROOT / "crates" / "epigrep-py" / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"

# The package/project version is the first ``version = "..."`` line in each
# manifest (under [project] / [package], before any dependency versions).
_VERSION = re.compile(r'^\s*version\s*=\s*"([^"]+)"', re.MULTILINE)

# The ``epigrep-py`` entry in Cargo.lock: a [[package]] block whose ``name`` is
# ``epigrep-py``, with a ``version = "..."`` line in the same block.
_LOCK_ENTRY = re.compile(
    r'\[\[package\]\]\s*\nname = "epigrep-py"\s*\nversion = "([^"]+)"'
)


def read_version(path: Path) -> str:
    match = _VERSION.search(path.read_text(encoding="utf-8"))
    if match is None:
        sys.exit(f"error: no `version = \"...\"` line found in {path}")
    return match.group(1)


def read_lock_version(path: Path) -> str:
    match = _LOCK_ENTRY.search(path.read_text(encoding="utf-8"))
    if match is None:
        sys.exit(f"error: no `epigrep-py` [[package]] entry found in {path}")
    return match.group(1)


def normalise(version: str) -> str:
    """Collapse PEP 440 / semver pre-release spelling to one form.

    ``0.1.0rc2`` and ``0.1.0-rc2`` both normalise to ``0.1.0rc2``; a plain
    ``0.1.0`` is unchanged.
    """
    return version.replace("-", "").lower()


def main() -> int:
    py_version = read_version(PYPROJECT)
    cargo_version = read_version(CARGO)
    lock_version = read_lock_version(CARGO_LOCK)

    normalised = {
        normalise(py_version),
        normalise(cargo_version),
        normalise(lock_version),
    }
    if len(normalised) != 1:
        print(
            "version mismatch across package metadata:\n"
            f"  pyproject.toml (PEP 440): {py_version}\n"
            f"  Cargo.toml     (semver):  {cargo_version}\n"
            f"  Cargo.lock     (semver):  {lock_version}\n"
            "Bump pyproject.toml and Cargo.toml together (semver uses a hyphen "
            "before the pre-release, e.g. 0.1.0-rc2; PEP 440 does not, e.g. "
            "0.1.0rc2), then regenerate Cargo.lock (`cargo update -p epigrep-py "
            "--precise <ver>` or any `cargo` build).",
            file=sys.stderr,
        )
        return 1

    print(
        f"version OK: pyproject {py_version} == Cargo {cargo_version} "
        f"== Cargo.lock {lock_version}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
