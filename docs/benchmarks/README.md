# Benchmarks — what is in this directory

| File | What it is |
|------|------------|
| `results.md` | Latest 3 runs, auto-generated. **Do not edit by hand.** |
| `history.csv` | The regression baseline. Cyrius builds only. |
| `history-rust-preport.csv` | Frozen archive. Pre-port Rust/criterion runs. |
| `rust-v-cyrius.md` | One-off port-day comparison, April 2026. |

## Why the history is split

`history.csv` originally held 382 rows recorded 2026-03-22/23 against the
**Rust** implementation, measured with criterion. That code was deleted in the
April 2026 port to Cyrius. Every row therefore measured software that no longer
exists, while `CLAUDE.md` pointed at the file as the current regression
baseline — so the "Numbers don't lie" gate had nothing real behind it, and no
Cyrius run had ever been recorded.

Those rows are preserved verbatim in `history-rust-preport.csv` and
`history.csv` was restarted from the Cyrius build at 2.3.1. The two are not
comparable: different implementation, different language, different measurement
harness. Do not chart them on one axis.

## Regenerating

```bash
./scripts/bench-history.sh
```

Appends one row per benchmark to `history.csv` and rewrites `results.md` from
the latest 3 runs. Run it from the repo root.

## A note on the parser

Before 2.3.1 the script's matcher only accepted whole-nanosecond values, so it
silently dropped every benchmark of 1µs or more — the 11 slowest, which are
exactly the ones worth watching. Two benchmarks sitting near the 1µs boundary
crossed it run-to-run and so appeared and vanished from the history depending on
noise. The parser now normalises ns/µs/ms/s to nanoseconds and captures all 46.

If a future run records fewer than 46 rows, suspect the parser before believing
a benchmark disappeared.
