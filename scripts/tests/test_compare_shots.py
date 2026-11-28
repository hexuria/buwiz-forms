"""Unit tests for scripts/e2e/compare_shots.py on tiny generated images.

Skipped whole-file when Pillow is absent: the scripts lane runs on machines without it
(`python -m unittest discover -s scripts/tests` must still exit 0 there)."""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "e2e"))

try:
    from PIL import Image
    import compare_shots
    HAVE_PIL = True
except ImportError:
    HAVE_PIL = False


def make_image(path, size=(8, 6), color=(10, 20, 30), changed=None):
    img = Image.new("RGB", size, color)
    if changed:
        img.putpixel(changed[0], changed[1])
    img.save(path)


@unittest.skipUnless(HAVE_PIL, "Pillow not installed")
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
        self.assertTrue(result["rows"][0]["identical"])
        self.assertEqual(result["rows"][0]["diff_pixels"], 0)

    def test_one_pixel_differs_with_box(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((3, 2), (99, 0, 0)))
        row = compare_shots.compare_runs(self.a, self.b)["rows"][0]
        self.assertFalse(row["identical"])
        self.assertEqual(row["diff_pixels"], 1)
        self.assertEqual(row["bbox"], [3, 2, 4, 3])

    def test_single_channel_small_change_counts(self):
        # luminance of (10,20,30) vs (10,20,34) rounds to equal on some weights: any-band must still count it
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((3, 2), (10, 20, 34)))
        row = compare_shots.compare_runs(self.a, self.b)["rows"][0]
        self.assertFalse(row["identical"])
        self.assertEqual(row["diff_pixels"], 1)

    def test_size_difference(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"), size=(8, 6))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), size=(9, 6))
        self.assertFalse(compare_shots.compare_runs(self.a, self.b)["rows"][0]["same_size"])

    def test_mask_top_ignores_top_rows(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((0, 0), (99, 0, 0)))
        self.assertTrue(compare_shots.compare_runs(self.a, self.b, mask_top=2)["rows"][0]["identical"])
        self.assertFalse(compare_shots.compare_runs(self.a, self.b)["rows"][0]["identical"])

    def test_mask_bottom_ignores_bottom_rows(self):
        size = (8, 6)
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"), size=size)
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), size=size, changed=((0, 5), (99, 0, 0)))
        self.assertTrue(compare_shots.compare_runs(self.a, self.b, mask_bottom=2)["rows"][0]["identical"])
        self.assertFalse(compare_shots.compare_runs(self.a, self.b)["rows"][0]["identical"])

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
        self.assertEqual(Image.open(written).size, (8 * 2 + 8, 6))
        self.assertTrue(os.path.isfile(os.path.join(out, "compare.json")))

    def test_slice_subfolders_do_not_collide_and_failure_png_skipped(self):
        for run in (self.a, self.b):
            for slice_dir in ("slice-1-shots", "slice-2-shots"):
                os.makedirs(os.path.join(run, "01-recipe", slice_dir))
                make_image(os.path.join(run, "01-recipe", slice_dir, "001-s1.png"))
            make_image(os.path.join(run, "01-recipe", "failure.png"), changed=((0, 0), (200, 0, 0)))
        result = compare_shots.compare_runs(self.a, self.b)
        keys = [row["pair"] for row in result["rows"]]
        self.assertEqual(len(keys), 2)
        self.assertTrue(all("failure" not in key for key in keys))

    def test_main_exit_codes_and_out_refusal(self):
        make_image(os.path.join(self.a, "01-recipe", "001-s1.png"))
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"), changed=((1, 1), (99, 0, 0)))
        self.assertEqual(compare_shots.main([self.a, self.b]), 1)
        self.assertEqual(compare_shots.main([self.a, self.b, "--allow-diff"]), 0)
        make_image(os.path.join(self.b, "01-recipe", "001-s1.png"))
        self.assertEqual(compare_shots.main([self.a, self.b]), 0)
        with self.assertRaises(SystemExit) as raised:
            compare_shots.main([self.a, self.b, "--out", os.path.join(self.a, "out")])
        self.assertNotEqual(raised.exception.code, 0)


if __name__ == "__main__":
    unittest.main()
