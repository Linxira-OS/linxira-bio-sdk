"""Pre-publication markers must not reach a rendered page as visible text.

Capability pages open with an HTML comment recording their review state. The
docs-site generator renders a small markdown subset, and it originally treated that
line as a paragraph and HTML-escaped it, so the marker appeared on the generated page
(and in the GUI's offline documentation pane, which renders the same files). The same
applies to block comments. These tests pin the skip in the generator.
"""

import importlib.util
import pathlib
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GENERATOR = REPO_ROOT / "scripts" / "gen-docs-site.py"


def load_generator():
    spec = importlib.util.spec_from_file_location("gen_docs_site", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class DocsSiteMarkerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.generator = load_generator()

    def test_single_line_marker_is_dropped(self) -> None:
        html = self.generator.render_markdown(
            "<!-- 发布前检查 / pre-publication check: pending -->\n\nTitle\n\nBody text.\n"
        )
        self.assertNotIn("发布前检查", html)
        self.assertNotIn("pending", html)
        self.assertIn("Title", html)
        self.assertIn("Body text.", html)

    def test_block_marker_is_dropped(self) -> None:
        html = self.generator.render_markdown(
            "<!--\n     未核项: the English mirror\n     second line\n-->\n\nVisible.\n"
        )
        self.assertNotIn("未核项", html)
        self.assertNotIn("second line", html)
        self.assertIn("Visible.", html)

    def test_a_real_capability_page_carries_no_marker_text(self) -> None:
        page = REPO_ROOT / "docs" / "capabilities" / "plot.render.v1" / "en-US.md"
        text = page.read_text(encoding="utf-8")
        self.assertIn("pre-publication check", text, "the marker is missing from the page")
        html = self.generator.render_markdown(text)
        self.assertNotIn("pre-publication", html)
        self.assertNotIn("发布前检查", html)


if __name__ == "__main__":
    unittest.main()
