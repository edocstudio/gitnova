#!/usr/bin/env python3
"""Export measured evidence as a portable, versioned website report. No benchmarks run here."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import statistics

ROOT = Path(__file__).resolve().parents[3]
PREFIX = "2026-10-28"
FILES = [f"{PREFIX}-{name}" for name in (
    "optimization-timings.json", "native-soak.json", "native-soak-memory.csv",
    "optimization-and-soak.md", "benchmark-environment.json",
)]
FLOWS = {
    "switch": ["switch_feature_preview", "switch_feature_apply", "switch_refresh"],
    "push": ["push_preview", "push_apply", "push_refresh"],
    "pull": ["pull_fetch", "pull_preview", "pull_apply", "pull_refresh"],
    "merge": ["merge_preview", "merge_begin", "merge_finish_preview", "merge_finish", "merge_refresh"],
}


def number(value, name, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"Invalid finite number: {name}")
    if value < 0 or (positive and value == 0):
        raise ValueError(f"Invalid measurement range: {name}")
    return value


def measurement(rows, scenario):
    # The fixture switches to feature and back to main, with two refresh rows.
    # The switch total intentionally uses the first refresh, after feature.
    row = next((v for v in rows if v["scenario"] == scenario), None)
    if row is None:
        raise ValueError(f"Missing measured scenario: {scenario}")
    number(row["milliseconds"], scenario)
    return row


def comparison(before, after):
    for values in (before, after):
        if not values:
            raise ValueError("Empty measurement series")
        for value in values:
            number(value, "latency", positive=True)
    a, b = statistics.median(before), statistics.median(after)
    return {
        "before": {"medianMs": a, "samplesMs": before, "sampleCount": len(before)},
        "after": {"medianMs": b, "samplesMs": after, "sampleCount": len(after)},
        "changePercent": (b / a - 1) * 100,  # Negative means shorter elapsed time.
    }


def build_report(directory):
    def read(suffix):
        return json.loads((directory / f"{PREFIX}-{suffix}.json").read_text())
    timings = read("optimization-timings")
    soak = read("native-soak")
    environment = read("benchmark-environment")
    operations = []
    for flow, scenarios in FLOWS.items():
        series, calls = {}, {}
        for side in ("before", "after"):
            runs = [v["data"]["measurements"] for v in timings["operations"] if v["side"] == side]
            if len(runs) < 3:
                raise ValueError("Publication requires at least three operation runs per side")
            series[side] = [sum(measurement(rows, k)["milliseconds"] for k in scenarios) for rows in runs]
            calls[side] = [sum(number(measurement(rows, k)["git"]["calls"], "Git calls") for k in scenarios) for rows in runs]
        row = {"id": flow, **comparison(series["before"], series["after"])}
        for side in series:
            row[side]["gitCallsSamples"] = calls[side]
        operations.append(row)
    search_series = {"cold": {}, "warm": {}}
    for side in ("before", "after"):
        runs = [v["data"]["measurements"] for v in timings["reads"] if v["side"] == side]
        if len(runs) != 1:
            raise ValueError("Expected one declared read benchmark per side")
        rows = runs[0]
        search_series["cold"][side] = [measurement(rows, "workspace_search_cold")["milliseconds"]]
        search_series["warm"][side] = [number(v["milliseconds"], "warm search") for v in rows if v["scenario"].startswith("workspace_search_warm_")]
    samples = soak["samples"]
    if not samples:
        raise ValueError("No resource samples")
    for row in samples:
        for key in ("elapsed_seconds", "native_rss_kib", "webkit_rss_kib", "sampled_git_rss_kib", "sampled_total_rss_kib"):
            number(row[key], key)
    duration = number(soak["sampled_seconds"], "duration", positive=True)
    if soak["passed"] and duration < soak["requested_seconds"]:
        raise ValueError("Completed soak shorter than requested duration")
    intervals = []
    for start in range(0, int(soak["requested_seconds"]), 300):
        rows = [v for v in samples if start <= v["elapsed_seconds"] < start + 300]
        if rows:
            intervals.append({"startSeconds": start, "endSeconds": start + 300, **{
                key: statistics.median(v[source] / 1024 for v in rows)
                for key, source in [("nativeMiB", "native_rss_kib"), ("webKitMiB", "webkit_rss_kib"), ("sampledSumMiB", "sampled_total_rss_kib")]
            }})
    large = timings["large_operations"]
    return {
        "schemaVersion": 1,
        "reportId": environment["reportId"],
        "measuredAt": environment["measuredAt"],
        "environment": environment["host"],
        "provenance": {"kind": "measured-local-development-build", "baselineRevision": None, "candidateRevision": None,
                       "revisionNote": "Uncommitted local development builds; revision attribution was not captured in the original run.",
                       "beforeProfile": timings["before"], "afterProfile": timings["after"]},
        "comparison": {"operationFixture": {"trackedFiles": 500, "transport": "local-bare", "scope": "preview-apply-refresh", "includesHumanReviewTime": False},
                       "operations": operations,
                       "search": {"fixtureCommits": 20100, "cold": comparison(**search_series["cold"]), "warm": comparison(**search_series["warm"])},
                       "largeOperations": {"historyCommits": large["history_commits"], "trackedFiles": large["tracked_files"], "sampleCount": 1,
                                           "elapsedMs": {name: sum(measurement(large["measurements"], k)["milliseconds"] for k in keys) for name, keys in FLOWS.items()}}},
        "soak": {"passed": soak["passed"], "failure": soak["failure"], "requestedSeconds": soak["requested_seconds"],
                 "sampledSeconds": duration, "cycles": soak["cycles"], "sampleCount": len(samples), "fixture": soak["fixture"],
                 "visibility": soak["observed_webkit_visibility"], "profile": soak["profile"], "attribution": soak["attribution"],
                 "memory": soak["memory"], "intervals": intervals, "latencies": soak["latencies"],
                 "maxRenderedCommitRows": soak["max_visible_rows"], "maxDomNodes": soak["max_dom_nodes"]},
        "limitations": ["single-machine", "development-build", "code-and-profile-effects-combined", "local-transport-only",
                        "background-webview-not-foreground-rendering", "sampled-rss-not-js-heap-or-gpu", "shared-page-double-counting-possible",
                        "no-full-workday-or-leak-proof", "no-language-or-cross-product-comparison", "cold-and-large-workspace-search-remain-slow",
                        "search-coverage-20000-commits-per-worktree"],
        "artifacts": [{"path": f"artifacts/{name}", "bytes": (directory / name).stat().st_size,
                       "sha256": hashlib.sha256((directory / name).read_bytes()).hexdigest()} for name in FILES],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verification-dir", type=Path, default=ROOT / "docs/verification")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "docs/benchmarks")
    args = parser.parse_args()
    report = build_report(args.verification_dir)
    encoded = json.dumps(report, indent=2, allow_nan=False) + "\n"
    folder = args.output_dir / "artifacts"
    folder.mkdir(parents=True, exist_ok=True)
    for name in FILES:
        shutil.copyfile(args.verification_dir / name, folder / name)
    (args.output_dir / "latest.json").write_text(encoded)
    print(f"Exported {report['reportId']}: {len(report['comparison']['operations'])} flows, {report['soak']['cycles']} soak cycles; no benchmarks rerun.")


if __name__ == "__main__":
    main()
