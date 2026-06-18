#!/usr/bin/env python3
"""Fail if the epigrep-py version is declared inconsistently.

The package version lives in two files that must agree:

  * ``crates/epigrep-py/pyproject.toml`` — PEP 440 (e.g. ``0.1.0rc2``)
  * ``crates/epigrep-py/Cargo.toml``     — semver    (e.g. ``0.1.0-rc2``)

maturin takes the wheel version from ``pyproject.toml``; the crate version is
the Cargo one. The two notations differ for pre-releases (PEP 440 ``0.1.0rc2``
vs semver ``0.1.0-rc2``), so we compare a normalised form rather than the raw
strings. Run with no arguments; exits non-zero on mismatch.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYPROJECT = ROOT / "crates" / "epigrep-py" / "pyproject.toml"
CARGO = ROOT / "crates" / "epigrep-py" / "Cargo.toml"

# The package/project version is the first ``version = "..."`` line in each
# file (under [project] / [package], before any dependency versions).
_VERSION = re.compile(r'^\s*version\s*=\s*"([^"]+)"', re.MULTILINE)


def read_version(path: Path) -> str:
    match = _VERSION.search(path.read_text(encoding="utf-8"))
    if match is None:
        sys.exit(f"error: no `version = \"...\"` line found in {path}")
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

    if normalise(py_version) != normalise(cargo_version):
        print(
            "version mismatch between package manifests:\n"
            f"  pyproject.toml (PEP 440): {py_version}\n"
            f"  Cargo.toml     (semver):  {cargo_version}\n"
            "Bump both together (semver uses a hyphen before the pre-release, "
            "e.g. 0.1.0-rc2; PEP 440 does not, e.g. 0.1.0rc2).",
            file=sys.stderr,
        )
        return 1

    print(f"version OK: pyproject {py_version} == Cargo {cargo_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
