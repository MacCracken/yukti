# cycc 6.6.0 clobbers a function parameter's stack slot on aggregate assign

**Status:** OPEN — blocks 3 tcyr assertions + 1 fuzz harness in yukti 2.3.9
**Affects:** cycc 6.5.57 through 6.6.0 (bisected below)
**Severity:** P0 — silent stack-slot corruption in a very common stdlib shape
**Filed against:** cyrius (this repo cannot fix it; the fix is in `src/`, upstream)

## Symptom in yukti

Under cycc 6.6.0 these SIGSEGV (signal 11):

- `tests/tcyr/yukti.tcyr::test_parse_uevent_usb_add`
- `tests/tcyr/yukti.tcyr::test_parse_uevent_remove`
- `tests/tcyr/yukti.tcyr::test_parse_uevent_no_header`
- `fuzz/fuzz_parse_uevent.fcyr`

All four funnel into `parse_uevent` (`src/udev.cyr:479`). None of them touch
`Result`, so this is not the 6.6.0 value-form migration.

## It is the compiler, not the migration

The crash reproduces on the **unmigrated 2.3.8 tree**:

```
$ git archive HEAD | tar -x -C /tmp/baseline    # yukti 2.3.8, no migration
$ cd /tmp/baseline && cyrius deps               # vendors the 6.5.29 stdlib
$ cat tests/tcyr/yukti.tcyr | ~/.cyrius/versions/6.5.29/bin/cycc > t && ./t
797 passed, 0 failed (797 total)                # rc=0

$ cat tests/tcyr/yukti.tcyr | ~/.cyrius/versions/6.6.0/bin/cycc > t && ./t
=== udev ===                                    # rc=139 (SIGSEGV)
```

Same bytes in, same stdlib, two compilers, opposite outcomes.

## Mechanism

`parse_uevent` splits a NUL-separated uevent buffer with

```
for (var i = 0; i <= len; i = i + 1) { ... }
```

`len` is the second **parameter**. Tracing `i` and `len` every iteration on a
118-byte buffer:

```
i=0 len=118
...
i=41 len=118
i=42 len=140219676361280      <-- parameter overwritten with a pointer
```

`len` becomes a `Str` data pointer, the bound never trips, and the loop walks
off the end of the buffer until it faults. The flip happens while processing
the `ACTION=add` part — i.e. on the `action = val;` arm of the `elif` chain.

`Str` is `struct Str { data; len; }`, a two-word aggregate. cycc 6.5.57 added
`_try_aggregate_copy_assign`, a word-by-word copy for `dst = src` on aggregates.
`action` is declared `var action = str_from("");` — the DECLARATION path sizes
it from a call return (one slot), but the ASSIGNMENT path now writes two words,
and the second lands in the neighbouring slot, which here holds `len`.

## Minimal repro (stdlib only, ~20 lines)

```cyrius
include "lib/syscalls.cyr"
include "lib/string.cyr"
include "lib/alloc.cyr"
include "lib/str.cyr"
include "lib/fmt.cyr"

fn work(buf, len) {
    var action = str_from("");
    for (var i = 0; i <= len; i = i + 1) {
        sys_write(1, "i=", 2); fmt_int(i); sys_write(1, " len=", 5); fmt_int(len); sys_write(1, "\n", 1);
        if (i > 30) { sys_write(1, "RUNAWAY\n", 8); sys_exit(9); }
        if (i == 5) {
            var part = str_from_buf(buf, 10);
            var val = str_substr(part, 7, 10);
            action = val;
        }
    }
    return action;
}

fn main() {
    alloc_init();
    var buf[64];
    memcpy(&buf, "ACTION=add", 10);
    store8(&buf + 10, 0);
    work(&buf, 11);
    sys_write(1, "done\n", 5);
    return 0;
}
```

Run from a directory whose `lib/` is the cyrius stdlib:

```
$ cat m15.cyr | ~/.cyrius/versions/6.5.56/bin/cycc > m && chmod +x m && ./m | tail -1
done
$ cat m15.cyr | ~/.cyrius/versions/6.6.0/bin/cycc > m && chmod +x m && ./m | tail -1
RUNAWAY
```

Both compile cleanly — no diagnostic either way.

## Bisect

| cycc   | result  |
|--------|---------|
| 6.5.29 | done    |
| 6.5.55 | done    |
| 6.5.56 | done    |
| 6.5.57 | RUNAWAY |
| 6.5.58 | RUNAWAY |
| 6.5.59 | RUNAWAY |
| 6.5.60 | RUNAWAY |
| 6.5.64 | RUNAWAY |
| 6.5.66 | RUNAWAY |
| 6.5.68 | RUNAWAY |
| 6.5.70 | RUNAWAY |
| 6.5.71 | RUNAWAY |
| 6.5.72 | RUNAWAY |
| 6.5.73 | RUNAWAY |
| 6.6.0  | RUNAWAY |

First bad release: **6.5.57**, whose CHANGELOG entry is
"Copying an aggregate copied only its first 8 bytes" — the change that
introduced `_try_aggregate_copy_assign` and touched the regalloc picker's
rbp-disp safety scan.

## Not avoidable from the consumer side

None of these change the outcome:

```
CYRIUS_IR=0  CYRIUS_IR=1  CYRIUS_IR=2  CYRIUS_RELOADELIM=0
CYRIUS_FRAMETRIM=0  CYRIUS_MONOMORPH=0  CYRIUS_STACK_ARRAYS=0
```

Per the cyrius working agreement "DON'T encode codegen bugs as language rules —
when the compiler can't compile valid cyrius, FIX THE COMPILER", `parse_uevent`
is NOT being restructured around this. The four tests stay as written and go
green when cycc is fixed.

## Narrowing notes (for whoever fixes it)

Variants that DO reproduce keep all three of: a local initialized from a
`Str`-returning call, a `str_substr` result assigned to it, and the assignment
inside a loop whose bound is a parameter.

- `var action = str_from(""); ... var val = str_substr(part, 7, 10); action = val;` → **repro**
- same, but `var action = 0;` instead → no repro
- same, but `var val = str_from_buf(buf, 10);` instead of `str_substr` → no repro
- adding the `str_eq_cstr` guard / `elif` chain / `map_set` / `continue` changes
  nothing either way — they are not part of the trigger
