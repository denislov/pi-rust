# 0.4.0 Runtime Baseline

This is the reproducible offline baseline required by `RIF-010` and `ADR-009`.
It measures deterministic correctness harnesses rather than optimized release
binary throughput. The harness keeps build artifacts under the repository
`target/` directory and writes the latest result to
`target/perf-baseline/latest.tsv`.

Run from the workspace root:

```bash
scripts/runtime-baseline.sh
```

The cases are:

| Case | Coverage | Provisional limit |
| --- | --- | ---: |
| `admission` | OperationScheduler admission contract | 5 s |
| `writer_pressure` | bounded writer queue saturation/rejection | 5 s |
| `session_commit_outbox` | terminal session writes and durable outbox | 5 s |
| `snapshot_reconnect` | retained-window eviction with committed context snapshot | 5 s |
| `recovery_scan` | compatibility/restart recovery matrix | 5 s |

The limits are guardrails for unexplained architectural regressions, not
release-performance budgets. They must be recalibrated on a materially
different CI host and retained with the host/toolchain context before being
promoted to a hard budget by the later 0.4.x hardening plan.
