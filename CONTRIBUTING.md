# Contributing to Yukti

Thanks for taking the time to dig in.

## Prerequisites

- Cyrius toolchain 5.4.6+ (`cyrius` on `$PATH`) —
  <https://github.com/MacCracken/cyrius>
- A Linux host for udev/mount behaviour to actually do anything useful

## Development Workflow

1. Fork and clone
2. `cyrius deps` — pull stdlib + sakshi + patra into `lib/`
3. Create a feature branch from `main`
4. Make your changes
5. Build + test + fuzz (see below)
6. Open a pull request

## Build / Test / Bench / Fuzz

```sh
cyrius build src/main.cyr build/yukti
cyrius test  tests/tcyr/yukti.tcyr
cyrius bench tests/bcyr/yukti.bcyr
for f in fuzz/*.fcyr; do
  n=$(basename "$f" .fcyr); cyrius build "$f" "build/$n" && "./build/$n"
done
cyrius distlib             # rebuild dist/yukti.cyr (full userland)
cyrius distlib core        # rebuild dist/yukti-core.cyr (kernel-safe)
cyrius build programs/core_smoke.cyr build/core_smoke && ./build/core_smoke
cyrius deps --verify       # supply-chain gate
```

There is no Makefile — the `cyrius` tool is the whole build system.
Never shell out to `cc5` directly; always go through `cyrius <subcommand>`.

## Adding a Module

1. Create `src/your_module.cyr` — zero transitive includes (stdlib
   includes live only in `src/lib.cyr`)
2. Add `include "src/your_module.cyr"` to `src/lib.cyr` in dependency order
3. Add it to `[lib] modules = [...]` in `cyrius.cyml`
4. If the module is kernel-safe (zero alloc, zero syscalls, zero stdlib
   calls) also add it to `[lib.core] modules` and extend
   `programs/core_smoke.cyr` with an assertion per exported symbol
5. Write tests in `tests/tcyr/yukti.tcyr` (assertion target: +30 per module)
6. Add benchmarks in `tests/bcyr/yukti.bcyr` for any hot path
7. Update `docs/architecture/overview.md` module table

## Code Style

- Enums for constants — zero gvar cost
- Manual struct layout — `alloc` + `store64/load64` with named offsets
- Accessor functions — `fn type_field(ptr) { return load64(ptr + F); }`
- `str_builder` for formatting, not temp allocations
- Direct syscalls only — no libc, no external deps
- `sakshi_*` for logging; no raw `println` in library code

## Testing

- 531 assertions is the current floor — do not regress
- Hardware-dependent logic must be reachable from mock data (see
  `find_mount_in()` taking a string, not `/proc/mounts`)
- Parsers get a fuzz target (`fuzz/*.fcyr`) before merge

## Benchmarks

- Every optimization PR needs a benchmark showing the delta
- `./scripts/bench-history.sh` appends to `docs/benchmarks/history.csv`
  and regenerates `docs/benchmarks/results.md` with a 3-run trend

## Commits

- Conventional-ish messages, one logical change per commit
- The user (maintainer) handles all git push / tag / release — do NOT
  push tags from a PR branch

## License

By contributing, you agree that your contributions will be licensed under
GPL-3.0-only.
