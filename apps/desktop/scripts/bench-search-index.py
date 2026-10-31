#!/usr/bin/env python3
"""Compare two already-built perf_audit probes serially on disposable fixtures.

No build, application launch, credentials or writes to existing repositories.
"""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--commits', type=int, default=10000)
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not 100 <= args.commits <= 20000 or not 2 <= args.rounds <= 5:
        parser.error('Use 100–20000 commits and 2–5 rounds.')
    binaries = {key: getattr(args, key).resolve() for key in ('baseline', 'candidate')}
    if any(not path.is_file() for path in binaries.values()):
        parser.error('Both perf_audit executables must already exist.')
    hashes = {key: hashlib.sha256(path.read_bytes()).hexdigest() for key, path in binaries.items()}
    if hashes['baseline'] == hashes['candidate']:
        parser.error('Baseline and candidate are the same executable.')
    env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull, LC_ALL='C')
    runs = []
    for round_index in range(args.rounds):
        order = ('baseline', 'candidate') if round_index % 2 == 0 else ('candidate', 'baseline')
        for key in order:
            result = subprocess.run([str(binaries[key]), str(args.commits), '2'], env=env,
                                    capture_output=True, text=True, timeout=300, check=True)
            data = json.loads(result.stdout)
            if data['commits'] != args.commits + 100 or data['locations'] != 2:
                raise ValueError('Unexpected fixture size')
            for row in data['measurements']:
                value = row['milliseconds']
                if not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
                    raise ValueError('Invalid timing')
            runs.append({'round': round_index + 1, 'variant': key, **data})
            print(f'Finished round {round_index + 1}: {key}', flush=True)
    scenarios = ['workspace_search_cold', 'workspace_search_persisted_restart']
    comparison = {}
    for scenario in scenarios:
        samples = {key: [row['milliseconds'] for run in runs if run['variant'] == key
                         for row in run['measurements'] if row['scenario'] == scenario]
                   for key in binaries}
        if any(len(values) != args.rounds for values in samples.values()):
            raise ValueError('Missing scenario observations')
        medians = {key: statistics.median(values) for key, values in samples.items()}
        comparison[scenario] = {'samplesMs': samples, 'medianMs': medians,
                                'changePercent': (medians['candidate'] / medians['baseline'] - 1) * 100}
    report = {'schemaVersion': 1, 'kind': 'gitnova-search-index-comparison',
              'measuredAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'host': {'os': platform.system(), 'architecture': platform.machine()},
              'binarySha256': hashes, 'commits': args.commits + 100, 'repositories': 2,
              'rounds': args.rounds, 'comparison': comparison, 'runs': runs,
              'limitations': [
                  'Alternating serial runs on separate deterministic synthetic histories; system load and filesystem caches affect latency.',
                  'Cold means first search in an Application instance, not cold OS caches.',
                  'Debug Rust core only; excludes UI/WebKit, network, real credentials and Git write operations.',
                  'RSS observations occur after operations and are not peaks or a leak/stability guarantee.',
                  'No cross-product, language or whole-app speed comparison.']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(comparison, indent=2), flush=True)


if __name__ == '__main__':
    main()
