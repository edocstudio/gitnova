# M73 — Reproducible persistent-core resource profiling

A standalone opt-in Rust example creates a synthetic real Git repository and
linked worktrees, then repeatedly exercises graph/detail, working snapshot,
metadata search, literal file/code investigation and branch comparison in one
long-lived Application instance. It never builds, launches a normal GitNova app,
reads credentials or operates on a developer's project repository.

The serial Python runner samples only that process and descendants, retains
bounded latency tails, records a cold observation separately, exports five-minute
memory medians and stops on resource/deadline failure. Fixture preparation is
labelled separately from workload samples. Default protocol is 30 minutes,
50,000 commits, 10,001 tracked files and three worktree locations sharing one
object database. Native Git fixtures and transient catalogs are temporary.

This measures Rust core and sampled child-process RSS. It excludes WebKit/native
UI, network/provider behavior and writes/recovery workloads. It does not prove
absence of leaks or establish full-workday/production readiness. No language or
cross-product speed claim is inferred. Previous native/background-WebKit evidence
remains independently available in M57; its protocol is not relabelled as this
new experiment.

The source revision, uncommitted-state flag and exact benchmark executable hash
identify each run. A failed preparation/probe is not a successful measurement.
Do not publish a result before inspecting completion, resource guards, sample
coverage and the report's limitations.

The probe's final fixture check verifies unchanged HEAD and no staged, unstaged
or untracked changes in each checkout. It does not compare every index byte,
configuration file or secondary ref byte-for-byte. Functional Git-write tests
cover stronger preservation assertions separately; do not infer them from the
soak completion flag alone.

Actual results: [30-minute core soak](verification/2026-10-30-core-soak.md).
The machine-readable supplemental report and integrity manifest live under
`docs/benchmarks`; the historical M57 `latest.json` remains unchanged.
