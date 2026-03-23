# Contributing to Yantra

Thank you for your interest in contributing to Yantra.

## Development Workflow

1. Fork and clone the repository
2. Create a feature branch from `main`
3. Make your changes
4. Run `make check` to validate
5. Open a pull request

## Prerequisites

- Rust stable (MSRV 1.89)
- Components: `rustfmt`, `clippy`
- Optional: `cargo-audit`, `cargo-deny`, `cargo-tarpaulin`

## Makefile Targets

| Command | Description |
|---------|-------------|
| `make check` | fmt + clippy + test + audit |
| `make fmt` | Check formatting |
| `make clippy` | Lint with `-D warnings` |
| `make test` | Run test suite |
| `make audit` | Security audit |
| `make deny` | Supply chain checks |
| `make bench` | Run benchmarks |
| `make coverage` | Generate coverage report |
| `make doc` | Build documentation |

## Adding a Module

1. Create `src/module_name.rs` with module doc comment
2. Add `pub mod module_name;` to `src/lib.rs`
3. Re-export key types from `lib.rs`
4. Add tests in the module
5. Update README module table

If the module requires an external dependency, gate it behind a feature flag. Hardware I/O modules should be gated behind `#[cfg(target_os = "linux")]`.

## Code Style

- `cargo fmt` — mandatory
- `cargo clippy -- -D warnings` — zero warnings
- Doc comments on all public items
- `#[non_exhaustive]` on public enums
- No `println!` — use `tracing` for logging
- `unsafe` only for `libc` syscalls/ioctls — minimize scope, add `// SAFETY:` comments

## Testing

- Unit tests colocated in modules (`#[cfg(test)] mod tests`)
- Hardware-dependent tests marked `#[ignore]` with doc comments explaining requirements
- Feature-gated tests with `#[cfg(feature = "...")]`
- Parsing logic must be testable with mock data (e.g., `find_mount_in()` takes a string, not `/proc/mounts`)
- Target: 90%+ line coverage

## Benchmarks

- All optimizations must include corresponding benchmarks
- Run `./scripts/bench-history.sh` to record results
- Benchmark history is tracked in `bench-history.csv` with 3-point trend in `BENCHMARKS.md`

## Commits

- Use conventional-style messages
- One logical change per commit

## License

By contributing, you agree that your contributions will be licensed under GPL-3.0-only.
