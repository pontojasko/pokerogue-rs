"""Comparator contract tests: altered pixels, alpha, geometry, artifacts and failure gates."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import visual_parity as visual


class ComparatorTests(unittest.TestCase):
    def test_channels_tolerance_and_bounding_box(self):
        black = (2, 2, bytes([0, 0, 0, 255]*4))
        altered = (2, 2, bytes([0, 0, 0, 255]*3 + [5, 0, 0, 250]))
        result = visual.metrics(black, altered)
        self.assertEqual(result['changed_pixels'], 1)
        self.assertEqual(result['changed_ratio'], .25)
        self.assertEqual(result['changed_bbox'], [1, 1, 2, 2])
        self.assertEqual(result['mean_absolute_error'], 10/16)
        self.assertEqual(result['maximum_channel_error'], 5)
        self.assertEqual(visual.metrics(black, altered, 5)['changed_pixels'], 0)
        self.assertIsNone(visual.metrics(black, black)['changed_bbox'])

    def test_rejects_geometry_mismatch(self):
        with self.assertRaisesRegex(ValueError, 'dimensions differ'):
            visual.metrics((2, 1, bytes(8)), (1, 2, bytes(8)))

    def test_real_png_artifacts_and_regression_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            for name, color in [('original.png', 'black'), ('rust.png', 'red'), ('baseline.png', 'black')]:
                visual.run(['magick', '-size', '2x2', f'xc:{color}', output/name])
            result = subprocess.run(['python3',str(visual.ROOT/'scripts/visual_parity.py'),
                                     '--compare-only','--output',directory,'--baseline',str(output/'baseline.png')],
                                    capture_output=True)
            self.assertEqual(result.returncode, 1, result.stderr.decode())
            report = json.loads((output/'report.json').read_text())
            self.assertEqual(report['status'], 'gate-failed')
            self.assertEqual(report['parity']['changed_ratio'], 1)
            self.assertFalse(report['regression']['passed'])
            self.assertTrue((output/'diff.png').exists())
            self.assertEqual(visual.pixels(output/'side-by-side.png')[:2], (4, 2))
            self.assertIn('gate-failed', (output/'report.html').read_text())

    def test_missing_capture_cannot_report_success(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(['python3',str(visual.ROOT/'scripts/visual_parity.py'),
                                     '--compare-only','--output',directory],capture_output=True)
            self.assertEqual(result.returncode, 2)
            report = json.loads((Path(directory)/'report.json').read_text())
            self.assertEqual(report['status'], 'error')
            self.assertFalse(report['complete_visual_parity'])


    def test_baseline_cannot_alias_capture_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory)
            result=subprocess.run(['python3',str(visual.ROOT/'scripts/visual_parity.py'),'--output',directory,'--baseline',str(output/'rust.png')],capture_output=True)
            self.assertEqual(result.returncode,2)
            self.assertIn(b'separate reviewed file',result.stderr)

    def test_suite_rejects_overlapping_baseline_and_output(self):
        with tempfile.TemporaryDirectory() as directory:
            result=subprocess.run(['python3',str(visual.ROOT/'scripts/visual_suite.py'),'--output',directory,'--baseline-dir',directory,'--record-baselines'],capture_output=True)
            self.assertEqual(result.returncode,2)
            self.assertIn(b'must not overlap',result.stderr)


if __name__ == '__main__':
    unittest.main()
