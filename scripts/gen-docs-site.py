#!/usr/bin/env python3
"""Generate a static local documentation site from the capability docs.

Scans `docs/capabilities/*/{en-US,zh-CN}.md` and writes a pure local,
never-published HTML site under `build/docs-site/`:

    build/docs-site/index.html            capability list grouped by category
    build/docs-site/<capability>/<locale>.html   one page per capability/locale

The markdown subset used by the capability docs is rendered with a tiny
built-in renderer (no external dependencies): ATX headings, fenced code
blocks, bullet lists, and inline bold/code. Output is deterministic (sorted
capabilities, fixed locale order, byte-stable rendering).

    python3 scripts/gen-docs-site.py

`--check` verifies that an existing `build/docs-site/` exactly matches what
would be generated (including absence of stale files) without writing
anything; it exits 1 on any drift.
"""

from __future__ import annotations

import argparse
import html
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs" / "capabilities"
OUTPUT = ROOT / "build" / "docs-site"

LOCALES = ("en-US", "zh-CN")
LOCALE_LABELS = {"en-US": "English", "zh-CN": "中文"}

FENCE_RE = re.compile(r"^```(.*)$")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
BULLET_RE = re.compile(r"^[-*]\s+(.*)$")
BOLD_RE = re.compile(r"\*\*([^*\n]+)\*\*")

CSS = """\
body { font-family: system-ui, -apple-system, "Segoe UI", "PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif; line-height: 1.6; color: #1a1a1a; background: #ffffff; margin: 0; }
nav, main, footer { max-width: 60rem; margin: 0 auto; padding: 0 1rem; }
nav { padding-top: 1rem; }
main { padding-bottom: 2rem; }
h1, h2 { border-bottom: 1px solid #e2e2e2; padding-bottom: 0.25em; }
h2 { margin-top: 2em; }
pre { background: #f5f5f5; padding: 0.75rem 1rem; border-radius: 6px; overflow-x: auto; }
code { background: #f0f0f0; padding: 0.1em 0.35em; border-radius: 4px; font-size: 0.9em; }
pre code { background: none; padding: 0; }
ul { padding-left: 1.5em; }
li { margin: 0.35em 0; }
footer { border-top: 1px solid #e2e2e2; padding-top: 0.75rem; padding-bottom: 1.5rem; font-size: 0.85em; color: #555555; }
.meta { font-size: 0.9em; color: #555555; }
"""


def find_documents() -> list[tuple[str, str, Path]]:
    """Return sorted (capability, locale, path) entries for every doc found."""
    entries: list[tuple[str, str, Path]] = []
    if not DOCS.is_dir():
        raise SystemExit(f"capability docs directory not found: {DOCS}")
    for capability_dir in sorted(DOCS.iterdir()):
        if not capability_dir.is_dir():
            continue
        capability = capability_dir.name
        for locale in LOCALES:
            document = capability_dir / f"{locale}.md"
            if document.is_file():
                entries.append((capability, locale, document))
    if not entries:
        raise SystemExit("no capability documents found")
    return entries


def document_title(document: Path, capability: str) -> str:
    """First ATX H1 of a doc, falling back to the capability id."""
    for line in document.read_text(encoding="utf-8").splitlines():
        match = HEADING_RE.match(line)
        if match and len(match.group(1)) == 1:
            return match.group(2)
    return capability


def render_inline(text: str) -> str:
    """Render inline markup (code spans, bold) after HTML-escaping."""
    text = html.escape(text, quote=False)
    parts = text.split("`")
    rendered: list[str] = []
    for index, part in enumerate(parts):
        if index % 2 == 1:
            rendered.append(f"<code>{part}</code>")
        else:
            rendered.append(BOLD_RE.sub(r"<strong>\1</strong>", part))
    return "".join(rendered)


def render_code_block(code_lines: list[str], info: str) -> str:
    language = info.strip().split()[0] if info.strip() else ""
    class_attr = f' class="language-{html.escape(language, quote=True)}"' if language else ""
    body = html.escape("\n".join(code_lines), quote=False)
    return f"<pre><code{class_attr}>{body}</code></pre>"


def render_markdown(text: str) -> str:
    """Render the markdown subset used by capability docs to HTML fragments."""
    lines = text.split("\n")
    out: list[str] = []
    paragraph: list[str] | None = None
    list_items: list[list[str]] | None = None
    index = 0

    def flush() -> None:
        nonlocal paragraph, list_items
        if paragraph:
            out.append(f"<p>{render_inline(' '.join(paragraph))}</p>")
            paragraph = None
        if list_items:
            items = "\n".join(
                f"  <li>{render_inline(' '.join(item_lines))}</li>"
                for item_lines in list_items
            )
            out.append(f"<ul>\n{items}\n</ul>")
            list_items = None

    while index < len(lines):
        line = lines[index]
        fence = FENCE_RE.match(line)
        if fence:
            flush()
            info = fence.group(1)
            code_lines: list[str] = []
            index += 1
            while index < len(lines) and not FENCE_RE.match(lines[index]):
                code_lines.append(lines[index])
                index += 1
            index += 1  # skip the closing fence
            out.append(render_code_block(code_lines, info))
            continue
        heading = HEADING_RE.match(line)
        if heading:
            flush()
            level = len(heading.group(1))
            out.append(f"<h{level}>{render_inline(heading.group(2))}</h{level}>")
            index += 1
            continue
        bullet = BULLET_RE.match(line)
        if bullet:
            if list_items is None:
                flush()
                list_items = []
            list_items.append([bullet.group(1)])
            index += 1
            continue
        if not line.strip():
            flush()
            index += 1
            continue
        if list_items is not None and (line.startswith(" ") or line.startswith("\t")):
            list_items[-1].append(line.strip())
            index += 1
            continue
        if list_items is not None:
            flush()
        paragraph = [line] if paragraph is None else paragraph + [line]
        index += 1
    flush()
    return "\n".join(out)


def render_page(capability: str, locale: str, document: Path) -> str:
    body = render_markdown(document.read_text(encoding="utf-8"))
    indented = "\n".join(f"    {row}" for row in body.split("\n"))
    title = f"{capability} — {locale}"
    return (
        "<!DOCTYPE html>\n"
        f'<html lang="{locale}">\n'
        "<head>\n"
        '  <meta charset="utf-8">\n'
        '  <meta name="viewport" content="width=device-width, initial-scale=1">\n'
        f"  <title>{html.escape(title, quote=True)}</title>\n"
        f"  <style>{CSS}</style>\n"
        "</head>\n"
        "<body>\n"
        '  <nav><a href="../index.html">← Capability documentation</a></nav>\n'
        "  <main>\n"
        f'    <p class="meta">capability <code>{html.escape(capability, quote=True)}</code> · locale {locale}</p>\n'
        f"{indented}\n"
        "  </main>\n"
        "  <footer>Static local site generated by scripts/gen-docs-site.py.</footer>\n"
        "</body>\n"
        "</html>\n"
    )


def render_index(entries: list[tuple[str, str, Path]]) -> str:
    titles: dict[str, str] = {}
    for capability, locale, document in entries:
        if capability not in titles or locale == "en-US":
            titles[capability] = document_title(document, capability)

    by_category: dict[str, list[str]] = {}
    for capability, _locale, _document in entries:
        category = capability.split(".", 1)[0]
        if capability not in by_category.setdefault(category, []):
            by_category[category].append(capability)

    sections: list[str] = []
    for category in sorted(by_category):
        rows: list[str] = []
        for capability in sorted(by_category[category]):
            links = [
                f'<a href="{capability}/{locale}.html" lang="{locale}">{LOCALE_LABELS[locale]}</a>'
                for locale in LOCALES
                if any(
                    entry[0] == capability and entry[1] == locale for entry in entries
                )
            ]
            rows.append(
                f'    <li><code>{html.escape(capability, quote=True)}</code>'
                f" — {html.escape(titles[capability], quote=False)}"
                f": {' · '.join(links)}</li>"
            )
        sections.append(
            f'  <h2 id="category-{html.escape(category, quote=True)}">{html.escape(category)}</h2>\n'
            f"  <ul>\n"
            f"{chr(10).join(rows)}\n"
            f"  </ul>"
        )

    categories = sorted(by_category)
    capabilities = sorted({entry[0] for entry in entries})
    return (
        "<!DOCTYPE html>\n"
        '<html lang="en">\n'
        "<head>\n"
        '  <meta charset="utf-8">\n'
        '  <meta name="viewport" content="width=device-width, initial-scale=1">\n'
        "  <title>Linxira Bio — Capability Documentation</title>\n"
        f"  <style>{CSS}</style>\n"
        "</head>\n"
        "<body>\n"
        "  <header>\n"
        "    <h1>Linxira Bio — Capability Documentation</h1>\n"
        f"    <p>{len(capabilities)} capabilities in {len(categories)} categories,"
        " generated from docs/capabilities. This site is a local build artifact"
        " and is never published.</p>\n"
        "  </header>\n"
        "  <main>\n"
        f"{chr(10).join(sections)}\n"
        "  </main>\n"
        "  <footer>Static local site generated by scripts/gen-docs-site.py.</footer>\n"
        "</body>\n"
        "</html>\n"
    )


def render_site(entries: list[tuple[str, str, Path]]) -> dict[str, str]:
    files: dict[str, str] = {"index.html": render_index(entries)}
    for capability, locale, document in entries:
        files[f"{capability}/{locale}.html"] = render_page(capability, locale, document)
    return files


def write_site(files: dict[str, str]) -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    # Remove stale files (and emptied directories) so the output tree exactly
    # mirrors the current docs; keeps --check consistent right after writing.
    expected = set(files)
    for path in list(OUTPUT.rglob("*")):
        if path.is_file() and path.relative_to(OUTPUT).as_posix() not in expected:
            path.unlink()
    for dirpath, dirnames, filenames in os.walk(OUTPUT, topdown=False):
        if not dirnames and not filenames:
            Path(dirpath).rmdir()
    for relative, content in sorted(files.items()):
        path = OUTPUT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content.encode("utf-8"))
    return 0


def check_site(files: dict[str, str]) -> int:
    actual: dict[str, bytes] = {}
    if OUTPUT.is_dir():
        for path in sorted(OUTPUT.rglob("*")):
            if path.is_file():
                actual[path.relative_to(OUTPUT).as_posix()] = path.read_bytes()

    expected_paths = set(files)
    actual_paths = set(actual)
    problems: list[str] = []
    for relative in sorted(expected_paths - actual_paths):
        problems.append(f"missing {relative}")
    for relative in sorted(actual_paths - expected_paths):
        problems.append(f"unexpected {relative}")
    for relative in sorted(expected_paths & actual_paths):
        if actual[relative] != files[relative].encode("utf-8"):
            problems.append(f"differs {relative}")

    if problems:
        for problem in problems:
            print(f"drift: {problem}", file=sys.stderr)
        print(
            f"docs site is out of date ({len(problems)} problem(s)); "
            "run scripts/gen-docs-site.py",
            file=sys.stderr,
        )
        return 1
    total_bytes = sum(len(content.encode("utf-8")) for content in files.values())
    print(
        f"docs site is current ({len(files)} files, {total_bytes} bytes)"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify build/docs-site/ matches the generated content without writing",
    )
    args = parser.parse_args()

    entries = find_documents()
    files = render_site(entries)
    if args.check:
        return check_site(files)

    write_site(files)
    total_bytes = sum(len(content.encode("utf-8")) for content in files.values())
    print(f"wrote {OUTPUT.relative_to(ROOT)} with {len(files)} files, {total_bytes} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
