import tempfile
import unittest
from pathlib import Path

from scripts.check_published_docs import validate


class PublishedDocumentationValidation(unittest.TestCase):
    def test_accepts_existing_local_file_and_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            site = Path(directory)
            guide = site / "guide"
            guide.mkdir()
            (site / "index.html").write_text(
                '<a href="guide/">Guide</a>', encoding="utf-8"
            )
            (guide / "index.html").write_text(
                '<h1 id="contract">Contract</h1>', encoding="utf-8"
            )
            (guide / "page.html").write_text(
                '<a href="index.html#contract">Contract</a>', encoding="utf-8"
            )

            self.assertEqual(validate(site), [])

    def test_rejects_missing_local_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            site = Path(directory)
            site.mkdir(exist_ok=True)
            (site / "index.html").write_text(
                '<a href="missing/">Missing</a>', encoding="utf-8"
            )

            self.assertEqual(validate(site), ["index.html: missing/"])

    def test_rejects_missing_guide_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            site = Path(directory)
            guide = site / "guide"
            guide.mkdir()
            (site / "index.html").write_text("", encoding="utf-8")
            (guide / "index.html").write_text(
                '<a href="page.html#absent">Missing</a>', encoding="utf-8"
            )
            (guide / "page.html").write_text(
                '<h1 id="present">Present</h1>', encoding="utf-8"
            )

            self.assertEqual(
                validate(site),
                ["guide/index.html: page.html#absent (missing fragment)"],
            )


if __name__ == "__main__":
    unittest.main()
