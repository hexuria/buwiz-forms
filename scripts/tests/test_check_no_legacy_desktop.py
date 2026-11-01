import tempfile
import unittest
from pathlib import Path

from scripts.check_no_legacy_desktop import find_references


class NoBirDesktopGuardTests(unittest.TestCase):
    def test_clean_files_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.rs"
            source.write_text("fn main() {}\n", encoding="utf-8")
            self.assertEqual(find_references([source], root), [])

    def test_new_reference_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.rs"
            source.write_text("use bir_desktop::app;\n", encoding="utf-8")
            self.assertEqual(
                find_references([source], root),
                [("source.rs", 1, "use bir_desktop::app;")],
            )

    def test_allowlisted_reference_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "scripts" / "check_no_legacy_desktop.py"
            source.parent.mkdir()
            source.write_text("pattern = 'bir-desktop'\n", encoding="utf-8")
            self.assertEqual(find_references([source], root), [])

    def test_news_fetcher_data_dir_literal_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/bir-core/src/news_fetcher.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                'ProjectDirs::from("com", "Goldcoders", "bir-desktop")\n',
                encoding="utf-8",
            )
            self.assertEqual(find_references([source], root), [])

    def test_other_news_fetcher_reference_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/bir-core/src/news_fetcher.rs"
            source.parent.mkdir(parents=True)
            source.write_text('let crate_name = "bir-desktop";\n', encoding="utf-8")
            self.assertEqual(
                find_references([source], root),
                [("crates/bir-core/src/news_fetcher.rs", 1, 'let crate_name = "bir-desktop";')],
            )
