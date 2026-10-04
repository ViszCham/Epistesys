"""Check links, bilingual ordering, and metric-token parity, not semantic truth."""

from collections import Counter
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[2]
DOCS = [
    "README.md", "docs/README.md", "docs/research-hypotheses.md",
    "docs/known-limitations.md", "benchmarks/gb-cc75/2026-08-18/README.md",
    *[f"docs/research/{name}.md" for name in (
        "gb-cc75-adversarial-audit", "gb-cc75-study", "gb-cc75-methods-and-provenance",
        "gb-cc75-statistical-analysis", "gb-cc75-failure-analysis",
        "epistesys-prospective-evaluation-plan",
    )],
    "validation/gb-cc75-documentation-verification-2026-10-03.md",
    "docs/system-architecture.md", "docs/execution-guide.md",
    "docs/dgcl-operating-profile-and-closure.md", "docs/migration-contract.md",
    "docs/implementation-status.md",
]
METRIC_TOKENS = (
    "74.38%", "94.10%", "19.71", "39.52%", "48.50%", "8.98",
    "12.20", "27.83", "79.17%", "94.03%", "14.86", "8.83", "21.63",
    "26.50%", "16.14", "22.11", "12.18", "16.86", "19.23%", "15.38%",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_document(relative):
    path = ROOT / relative
    text = path.read_text(encoding="utf-8")
    require(text.count("\n## 日本語\n") == 1, f"Japanese section missing/duplicated: {relative}")
    require(text.count("\n## English\n") == 1, f"English section missing/duplicated: {relative}")
    before, english = text.split("\n## English\n")
    _, japanese = before.split("\n## 日本語\n")
    for token in METRIC_TOKENS:
        require(japanese.count(token) == english.count(token), f"metric-token drift: {relative}: {token}")
    tables = lambda part: [Counter(re.findall(r"[0-9]+(?:\.[0-9]+)?", line))
                           for line in part.splitlines() if line.startswith("|")]
    require(tables(japanese) == tables(english), f"numeric table drift: {relative}")
    without_fences = re.sub(r"```.*?```", "", text, flags=re.S)
    links = re.findall(r"\[[^\]]+\]\(([^)]+)\)", without_fences)
    for target in links:
        target = target.strip("<>")
        split = urlsplit(target)
        if split.scheme or not split.path:
            continue
        local = (path.parent / unquote(split.path)).resolve()
        require(local.is_relative_to(ROOT), f"link escapes repository: {relative}")
        require(local.exists(), f"broken local link: {relative}: {target}")
    return len(links)


def main():
    try:
        links = sum(check_document(path) for path in DOCS)
    except (AssertionError, OSError, ValueError) as error:
        print(f"Research documentation check failed: {error}", file=sys.stderr)
        return 1
    print(f"Checked {len(DOCS)} bilingual documents and {links} links; "
          "semantic claim parity requires manual review, not this syntactic check.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
