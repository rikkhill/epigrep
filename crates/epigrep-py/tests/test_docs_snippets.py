"""Executable documentation snippets — a drift guard for the docs.

Every runnable ```python block in the docs and READMEs is executed against the
installed package, so a renamed function, a changed signature, or a stale import
in the documentation fails CI instead of shipping. Blocks are run per file in a
shared namespace (so a later snippet can use names a earlier one defined), with a
``from epigrep import *`` preamble so bare-expression snippets resolve.

A block is skipped if it is illustrative — it contains an ``...`` placeholder, or
is immediately preceded by an ``<!-- snippet: skip -->`` HTML comment (invisible
in the rendered docs). Blocks needing an optional dependency that is not installed
are skipped rather than failed.

This checks that the snippets *run*, not their printed output; that is enough to
catch the API drift that matters.
"""

import contextlib
import io
import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]

DOC_FILES = [
    REPO / "README.md",
    REPO / "crates" / "epigrep-py" / "README.md",
    REPO / "docs" / "index.md",
    REPO / "docs" / "getting-started.md",
    REPO / "docs" / "events-and-partitions.md",
    REPO / "docs" / "loading-data.md",
    REPO / "docs" / "patterns.md",
    REPO / "docs" / "explanations.md",
    REPO / "docs" / "logs-first-recipes.md",
    # agent-interface.md is intentionally illustrative (partial ASTs); its API is
    # exercised by test_agent.py, so it is not in the executable-snippet set.
]

_BLOCK = re.compile(r"```python\n(.*?)```", re.S)


def _runnable_blocks(text):
    """Yield (index, code) for each runnable python block."""
    for index, match in enumerate(_BLOCK.finditer(text)):
        code = match.group(1)
        preceding = text[: match.start()].rstrip()
        if preceding.endswith("<!-- snippet: skip -->"):
            continue
        if "..." in code:  # illustrative placeholder
            continue
        yield index, code


def _present_doc_files():
    return [path for path in DOC_FILES if path.exists()]


@pytest.mark.parametrize("path", _present_doc_files(), ids=lambda p: p.name)
def test_doc_snippets_execute(path):
    namespace: dict = {}
    exec("from epigrep import *", namespace)  # noqa: S102 - trusted doc content

    ran = 0
    for index, code in _runnable_blocks(path.read_text()):
        compiled = compile(code, f"{path.name}#block{index}", "exec")
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                exec(compiled, namespace)  # noqa: S102 - trusted doc content
        except ModuleNotFoundError as exc:
            # An optional dependency (e.g. polars) is not installed here.
            pytest.skip(f"{path.name} block {index} needs {exc.name}")
        except Exception as exc:  # pragma: no cover - failure path
            pytest.fail(
                f"{path.name} block {index} raised {type(exc).__name__}: {exc}\n"
                f"--- snippet ---\n{code}"
            )
        ran += 1

    # Each listed doc should contribute at least one executed snippet, so a file
    # silently losing its examples (or this harness silently skipping everything)
    # is caught.
    assert ran >= 1, f"{path.name}: no runnable python snippets executed"
