# Search indexing and core memory follow-up — 2026-10-31

Source baseline: `921e979b73ae33a5e4c2bbe00b0fdac660d49b53`. The candidate
contains uncommitted search changes; exact measured executable SHA-256 values
are in the JSON evidence. This is macOS arm64 debug-core evidence, not native
Windows/Linux acceptance or a whole-application performance certificate.

## Change and bounds

Read up to 512 object headers in each `cat-file --batch-check`, instead of 64.
Read eligible commit bodies in separate groups of at most 64, retaining the
5 MiB output limit and 64 KiB per-commit eligibility limit. This reduces process
startup overhead during first indexing without enlarging the body-read buffer.
Header reads remain capped at 64 KiB. The existing scan, metadata, resident-cache,
pagination and ref-revalidation bounds are unchanged. No dependency was added.

## Alternating comparison

Three serial rounds alternate baseline/candidate order. Each executable creates
separate deterministic disposable histories: 10,000 commits plus a second
100-commit repository. Each search asserts the expected exact total and warm
result identity. First search means a new Application instance, not cold OS
caches; fixture creation itself warms the filesystem. No build/test ran while
measuring. The baseline was copied before rebuilding the candidate.

| Scenario | Baseline median | Candidate median | Change |
| --- | ---: | ---: | ---: |
| First workspace search | 4625.33 ms | 2601.38 ms | -43.76% |
| Search after persisted-cache restart | 444.55 ms | 447.97 ms | +0.77% |

The restart difference is small and not evidence of an improvement. There are
only three samples per variant; other applications and filesystem caching can
affect them. This does not measure branch switching, Pull/Push/Merge, WebView,
network latency, a language comparison or another product.

[Raw comparison](../../benchmarks/search-index-2026-10-31.json) includes all
samples, post-operation RSS observations and executable hashes. Post-operation
RSS is not a high-water measurement. Reproduce with already-built probes:

```sh
python3 apps/desktop/scripts/bench-search-index.py \
  --baseline /absolute/path/to/baseline-perf_audit \
  --candidate target/debug/examples/perf_audit \
  --commits 10000 --rounds 3 --output /tmp/search-comparison.json
```

## Ten-minute core regression soak

The already-built candidate completed 612.75 workload seconds, 50 cycles and
300 measured operations over a synthetic 50,000-commit / 10,001-file repository
with three linked checkout locations. The existing runner's final HEAD and
clean-working-state assertions passed for all locations. This is not a
byte-for-byte audit of every ref or a write/recovery test.

- Peak sampled core RSS: **58.83 MiB**.
- Peak sampled core plus descendant RSS: **92.48 MiB**.
- Workload core RSS medians in the first and second five-minute windows:
  **45.94 MiB** and **46.66 MiB**. The trailing window has only five samples.
- The 1 GiB owned-process guard and macOS free-memory guard did not fire.
- No native app was launched, and no user repository or real credential was used.

| Warm core scenario | Observations excluding first | Median | p95 |
| --- | ---: | ---: | ---: |
| Graph plus selected detail | 49 | 1051.24 ms | 1221.14 ms |
| Working snapshot | 49 | 703.30 ms | 797.35 ms |
| Commit search | 49 | 921.00 ms | 1242.65 ms |
| File history | 49 | 2578.17 ms | 2912.43 ms |
| Code history | 49 | 2633.14 ms | 2891.64 ms |
| Branch comparison | 49 | 2147.91 ms | 2329.21 ms |

Commit search, file history and code history report partial coverage in every
cycle; the full 50,000-commit history is not indexed/scanned. In particular,
commit search retains its 20,000-commit-per-worktree bound. File/code history
remain measured bottlenecks. Do not turn this into an instant-search claim.

[Raw soak](../../benchmarks/core-soak-2026-10-31.json) contains the five-second
memory samples, phase labels, all retained latencies and completion assertions.
Sampling can miss short-lived Git peaks. These numbers exclude WebKit/JS/GPU,
network/providers and write/recovery operations. A ten-minute core regression
sample does not replace the older thirty-minute evidence or establish full-day
or leak-free stability. Foreground native UI measurements remain open.

## Verification and remaining gates

- Core search integration: 14 tests passed, including a new 130-commit fixture
  whose eligible bodies total more than 5 MiB; both pages and persisted-cache
  reload preserve the exact results.
- Search/core all-targets Clippy with `-D warnings` passed on macOS.
- Search unit tests: 7 passed. Formatter, whitespace, Python syntax, JSON
  parsing and local report links passed.
- The comparison runner completed six serial runs, and the core soak passed.
- Native Windows/Linux CI still needs a run of the preceding corrections;
  the most recent observed hosted run is still the older failed baseline.

These supplemental JSON kinds can be consumed independently by a website.
The historical `latest.json` contract and the sibling website were not changed.
No commit, push, app installation or release was performed.
