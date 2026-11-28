"""Unit tests for scripts/e2e/compare_shots.py on tiny generated images."""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "e2e"))

from PIL import Image

import compare_shots


def make_image(path, size=(8, 6), color=(10, 20, 30), changed=None):
    img = Image.new("RGB", size, color)
    if changed:
        img.putpixel(changed[0], changed[1])
    img.save(path)


class CompareShotsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.a = os.path.join(self.tmp, "run-a")
        self.b = os.path.join(self.tmp, "run-b")
        for run in (self.a, self.b):
            os.makedirs(os.path.join(run, "01-recipe"))

    def test_identical_pair(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"))
        result = compare_shots.compare_runs(self.a, self.b)
        row = result["rows"][0]
        self.assertTrue(row["identical"])
        self.assertEqual(row["diff_pixels"], 0)

    def test_one_pixel_differs_with_box(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((3, 2), (99, 0, 0)))
        result = compare_shots.compare_runs(self.a, self.b)
        row = result["rows"][0]
        self.assertFalse(row["identical"])
        self.assertEqual(row["diff_pixels"], 1)
        self.assertEqual(row["bbox"], [3, 2, 4, 3])

    def test_size_difference(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"), size=(8, 6))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), size=(9, 6))
        result = compare_shots.compare_runs(self.a, self.b)
        self.assertFalse(result["rows"][0]["same_size"])

    def test_mask_top_ignores_top_rows(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((0, 0), (99, 0, 0)))
        masked = compare_shots.compare_runs(self.a, self.b, mask_top=2)
        self.assertTrue(masked["rows"][0]["identical"])
        unmasked = compare_shots.compare_runs(self.a, self.b, mask_top=0)
        self.assertFalse(unmasked["rows"][0]["identical"])

    def test_only_in_one_run(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        os.makedirs(os.path.join(self.b, "02-other"))
        make_image(os.path.join(self.b, "02-other", "001-s1.png"))
        result = compare_shots.compare_runs(self.a, self.b)
        self.assertEqual(len(result["only_before"]), 1)
        self.assertEqual(len(result["only_after"]), 1)

    def test_side_by_side_written_for_differing_pair(self):
        out = os.path.join(self.tmp, "out")
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((1, 1), (99, 0, 0)))
        compare_shots.compare_runs(self.a, self.b, out)
        written = os.path.join(out, "01-recipe_001-s1.png")
        self.assertTrue(os.path.isfile(written))
        img = Image.open(written)
        self.assertEqual(img.size, (8 * 2 + 8, 6))
        self.assertTrue(os.path.isfile(os.path.join(out, "compare.json")))

    def test_slice_subfolder_flattens(self):
        deep = os.path.join(self.a, "01-recipe", "slice-2-shots")
        os.makedirs(deep)
        make_image(os.path.join(deep, "017-s19.png"))
        other = os.path.join(self.b, "01-recipe")
        make_image(os.path.join(other, "017-s19.png"))
        result = compare_shots.compare_runs(self.a, self.b)
        self.assertEqual(len(result["rows"]), 1)
        self.assertTrue(result["rows"][0]["identical"])


if __name__ == "__main__":
    unittest.main()
