import importlib.util
import json
from pathlib import Path
import statistics
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("export_benchmarks", Path(__file__).with_name("export-benchmarks.py"))
export = importlib.util.module_from_spec(spec)
spec.loader.exec_module(export)


class ReportExportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        original = export.ROOT / "docs/verification"
        for name in export.FILES:
            (self.path / name).write_bytes((original / name).read_bytes())

    def mutate(self, suffix, change):
        path = self.path / f"{export.PREFIX}-{suffix}.json"
        value = json.loads(path.read_text())
        change(value)
        path.write_text(json.dumps(value))

    def test_totals_and_signed_regression_are_derived_from_measured_samples(self):
        report = export.build_report(self.path)
        original = json.loads((self.path / export.FILES[0]).read_text())
        switch = next(row for row in report["comparison"]["operations"] if row["id"] == "switch")
        rows = original["operations"][0]["data"]["measurements"]
        # Exclude the second refresh after switching back to main.
        expected = sum(rows[i]["milliseconds"] for i in (0, 1, 2))
        self.assertEqual(switch["before"]["samplesMs"][0], expected)
        self.assertEqual(switch["before"]["medianMs"], statistics.median(switch["before"]["samplesMs"]))
        push = next(row for row in report["comparison"]["operations"] if row["id"] == "push")
        self.assertGreater(push["changePercent"], 0)
        self.assertEqual(report["soak"]["cycles"], 117)
        self.assertEqual(len(report["soak"]["intervals"]), 6)

    def test_missing_or_nonfinite_results_cannot_be_published(self):
        original = (self.path / export.FILES[0]).read_text()
        for value in (float("nan"), -1, True):
            (self.path / export.FILES[0]).write_text(original)
            self.mutate("optimization-timings", lambda j: j["operations"][0]["data"]["measurements"][0].update(milliseconds=value))
            with self.assertRaises(ValueError):
                export.build_report(self.path)
        (self.path / export.FILES[0]).write_text(original)
        self.mutate("optimization-timings", lambda j: j["operations"][0]["data"]["measurements"].pop(0))
        with self.assertRaises(ValueError):
            export.build_report(self.path)

    def test_shortened_run_is_not_claimed_complete(self):
        self.mutate("native-soak", lambda j: j.update(sampled_seconds=30))
        with self.assertRaises(ValueError):
            export.build_report(self.path)


if __name__ == "__main__":
    unittest.main()
