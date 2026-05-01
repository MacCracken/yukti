# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.2.0] — 2026-04-30

Audio device discovery — unblocks vani 0.3.x. New `src/audio.cyr`
enumerates ALSA PCM devices (playback + capture) and surfaces the
typed descriptor adapter API that vani's `vani_open_yukti(desc,
direction)` consumes. After this lands, vani's roadmap items #8
(typed yukti audio descriptor adapter) and #9 (multi-device
routing — onboard / USB / HDMI) can fill in and ship 1.0.0.

Domain split: yukti reports presence + identity; vani opens the
kernel handle and asks via `SNDRV_PCM_IOCTL_HW_REFINE` what the
device actually supports. Capability queries deliberately stay
out of yukti — see vani's `audio_query_caps` (v0.2.0).

### Added

- **`src/audio.cyr`** — new ~430-line module mirroring
  `src/gpu.cyr`'s shape (sysfs/procfs enumeration, named offset
  constants, named struct accessors). Surfaces:
  - **Direction constants**: `YUKTI_AUDIO_PLAYBACK = 0`,
    `YUKTI_AUDIO_CAPTURE = 1` — bit-for-bit match with vani's
    `VaniDirection` so the adapter is a copy, not a translation.
  - **`AudioDeviceInfo` struct** (72 bytes): `card`, `device`,
    `subdevice`, `direction`, `name`, `driver`, `hw_id`,
    `dev_path`, `sys_path`. All fields i64 or Str — matches
    yukti's flat-pointer convention.
  - **Top-level enumerator**: `yukti_audio_devices()` walks
    `/dev/snd/pcmC{N}D{M}{p|c}`, cross-references
    `/proc/asound/cards` for driver name and
    `/proc/asound/card{N}/pcm{D}{p|c}/info` for friendly name.
  - **Filters**: `yukti_audio_devices_for_direction(dir)`,
    `yukti_audio_devices_for_card(card)`,
    `yukti_audio_count()`.
  - **Vani descriptor adapter API** — typed accessors that
    vani 0.3.x reads against an `AudioDeviceInfo*`:
    `yukti_audio_card(d)`, `_device`, `_subdevice`, `_direction`,
    `_name`, `_driver`, `_hw_id`, `_dev_path`, `_sys_path`.

  **`hw_id` construction** (anchors NEVER on the ALSA card index
  — adding a PCIe sound card reorders cards and would break
  device_db history):
  - **PCI** (built-in HDA, discrete sound, GPU HDMI): PCI BDF
    from card{N}/device/uevent's `PCI_SLOT_NAME` —
    `pci:0000:04:00.1:dev3:p`. Stable across reboots and ALSA
    reorderings.
  - **USB** (USB DAC, USB headset): `usb:VID:PID:SERIAL:dev{M}:{p|c}`
    using card{N}/device/uevent's PRODUCT triple plus
    card{N}/device/../serial when available.
  - **Fallback**: `card{N}_dev{M}_{p|c}` for loopback /
    bluetooth / virtual sources where neither PCI nor USB
    metadata exposes a stable identifier. Documented as
    "transient" in the module header — consumers should treat
    these as non-persistent.

- **`DC_AUDIO = 9`** appended to `DeviceClass` enum
  (`src/core.cyr:25`) for the udev classifier path. Placed
  AFTER `DC_UNKNOWN = 8` to preserve the AGNOS-pinned numeric
  value of the unknown sentinel — new variants always go at
  the tail; never inserted in the middle.

- **45 new test assertions** in `tests/tcyr/yukti.tcyr`
  covering `_audio_parse_pcm_name` (valid / multi-digit /
  capture / 7 invalid-input cases), bit-pack/unpack roundtrip,
  and AudioDeviceInfo accessor contracts. Total: 594 → 639.

### Fixed

- **`_parse_uevent_key` in `src/gpu.cyr`** silently returned
  the entire uevent text from the matched key onward instead
  of just the value. The inner line-end loop set
  `line_end = text_len` as a "break" sentinel and the
  post-loop fixup never fired (`line_end == text_len`, not
  `>`), so `vlen = line_end - vstart` captured everything
  through end-of-buffer. `gpu_info_driver` returning
  `"amdgpu\nPCI_CLASS=...\nPCI_ID=...\nPCI_SLOT_NAME=..."`
  instead of `"amdgpu"` was the symptom. Bug pre-dated
  2.1.x — surfaced when 2.2.0's audio module re-used the
  helper and got the same garbage. Replaced the negative-index
  break sentinel with the standard `done`-flag pattern
  (CLAUDE.md: "`break` in `var`-heavy loops unreliable —
  use flag + `continue`"). gpu.cyr's `gpu_info_driver` /
  `gpu_info_pci_slot` now return clean values
  (`amdgpu` / `0000:04:00.0`).

### Verified — live hardware (this build host)

- **3 ALSA cards × 8 PCM endpoints** detected end-to-end:
  card 0 HDMI 0/1/2/3 (playback × 4), card 1 ALC897 Analog
  (playback + 2 × capture), card 2 acp (capture × 1). All
  PCI hw_ids resolved correctly to BDF identifiers; acp falls
  back to `card2_dev0_c` because its uevent doesn't expose
  PCI_SLOT_NAME (expected — it's an AMD audio coprocessor
  with a different probe path).
- All gates green: build clean, lint 0 warnings, fmt 0 drift,
  639/639 tests, 3/3 fuzz, bench, core_smoke, dist regen
  (yukti.cyr 5925 lines, yukti-core.cyr 458 lines), kernel-safe
  invariant holds, aarch64 cross-build emits valid ARM ELFs
  for all 5 CI targets.

### Held — for 2.2.1+

The roadmap's audio-domain section listed two stretch items
that aren't blocking the vani consumer contract — vani 0.3.x
calls `yukti_audio_devices()` on demand, so it doesn't need
either before shipping:

- **udev hotplug subscription for `SUBSYSTEM=sound`** with a
  `pcmC*D*[pc]` DEVPATH filter (the control / sequencer /
  timer event noise has to be filtered out). Will land in
  2.2.1 alongside the file-manager / aethersafha consumer
  flow.
- **`device_db` persistence keyed by `hw_id`** — first-seen /
  last-seen / friendly-name table mirroring the existing
  `devices` table shape. Same 2.2.1 timing.

Both threads tracked in `docs/development/roadmap.md`.

## [2.1.4] — 2026-04-30

aarch64 runtime correctness — closes the second held-aarch64
thread (raw-number arch-divergent syscalls) flagged in 2.1.3.
2.1.3 made cross-builds *compile* clean; 2.1.4 makes them
*run* the right syscalls. The remaining held aarch64 work is
the original 5.4.6 SIGILL retest on real Cortex-A72 — that's
hardware-bound, not a code change. No yukti API changes. No
behavioural shift on x86_64 (every migration is pass-through:
either to a stdlib wrapper that resolves to the same syscall
on x86, or to an arch-conditional `SYS_*` constant whose
x86 value matches the hardcoded number it replaces).

### Changed

- **Raw-number `syscall(N, …)` calls migrated — 33 sites
  across 9 modules.** Pre-2.1.4 yukti src/ had hardcoded
  x86_64 syscall numbers throughout — the aarch64 binary
  built and linked but would invoke the *wrong* syscalls at
  runtime. Two migration paths depending on stdlib coverage:

  - **To stdlib wrappers** (which arch-translate internally):
    - `syscall(0, …)` → `sys_read(…)` — partition.cyr × 2,
      storage.cyr.
    - `syscall(1, …)` → `sys_write(…)` — main.cyr × 6,
      udev_rules.cyr × 2, storage.cyr.
    - `syscall(2, …)` → `sys_open(…)` — storage.cyr (the
      `/proc/mounts` reader; was missed in 2.1.3 because it
      used the literal `2` rather than the `SYS_OPEN`
      symbol).
    - `syscall(4, …)` → `sys_stat(…)` — device.cyr.
      Stdlib only ships `sys_stat` for aarch64; x86 gap
      filled with a yukti-local shim in `src/syscalls.cyr`
      (`fn sys_stat(path, buf) { return syscall(SYS_STAT,
      path, buf); }` under `#ifdef CYRIUS_ARCH_X86`). On
      aarch64 the stdlib's `sys_stat` (which routes through
      `SYS_NEWFSTATAT(AT_FDCWD, …)`) takes over.
    - `syscall(60, code)` → `sys_exit(code)` — main.cyr × 2.
    - `syscall(83, …)` → `sys_mkdir(…)` — network.cyr,
      storage.cyr (uses `SYS_MKDIRAT(AT_FDCWD, …)` on
      aarch64).
    - `syscall(84, …)` → `sys_rmdir(…)` — storage.cyr (uses
      `SYS_UNLINKAT(AT_FDCWD, AT_REMOVEDIR)` on aarch64).
    - `syscall(165, …)` → `sys_mount(…)` — network.cyr,
      storage.cyr × 2.
    - `syscall(166, …)` → `sys_umount2(…)` — storage.cyr.

  - **To stdlib `SYS_*` constants** (arch-correct values
    resolve at compile time):
    - `syscall(8, …)` → `syscall(SYS_LSEEK, …)` —
      partition.cyr × 2 (LSEEK has no wrapper in stdlib;
      arch-divergent number — x86=8, aarch64=62 — but the
      stdlib's enum resolves correctly per arch).

- **Arch-conditional `SYS_*` constants for stdlib-uncovered
  syscalls** — new file `src/syscalls.cyr` (added to
  `[lib] modules` and `src/lib.cyr` include chain just before
  `src/error.cyr`). Defines a single `enum YkSyscalls` block
  per arch under `#ifdef CYRIUS_ARCH_{X86,AARCH64}`,
  centralising what's currently a stdlib gap:
  - `SYS_SOCKET` (41 / 198), `SYS_CONNECT` (42 / 203),
    `SYS_BIND` (49 / 200), `SYS_RECVFROM` (45 / 207),
    `SYS_SETSOCKOPT` (54 / 208) — for udev netlink monitor +
    network probe.
  - `SYS_PPOLL` (271 / 73 — aarch64 has it in stdlib;
    x86 doesn't, so we add it x86-side only).
  - `SYS_STATFS` (137 / 43), `SYS_NEWFSTATAT` (262 / 79 —
    aarch64 has it in stdlib; x86 only), `SYS_CLOCK_GETTIME`
    (228 / 113) — for filesystem usage, mount-symlink TOCTOU
    guard, and event timestamping.
  - Plus the x86-side `sys_stat` shim noted above.

  Centralisation prevents the 2.1.3-era pattern where each
  src/ file had its own local `enum` shadow with hardcoded
  x86 numbers (silently wrong on aarch64). Will fold into
  the upstream cyrius stdlib whenever those constants get
  promoted; tracked alongside the held-aarch64 issue.

- **udev netlink monitor: `poll(2)` → `ppoll(2)`** in
  `udev_monitor_poll` (`src/udev.cyr:647-660`). Linux
  aarch64's syscall table has no `SYS_POLL` — the kernel
  exposes only `ppoll`. ppoll takes a `timespec*` (not int
  ms) and an extra `(sigmask, sigsetsize)` pair, so the call
  site now constructs a stack-allocated 16-byte timespec
  from `timeout_ms` (`tv_sec = ms / 1000; tv_nsec = (ms %
  1000) * 1_000_000`) and passes `sigmask = NULL,
  sigsetsize = 8` for "no signal mask change". x86_64 also
  has `ppoll` (271), so we drive both arches uniformly
  through it — no arch-conditional dispatch in the call site.

- **Local arch-divergent `enum NlConst` cleanup**
  (`src/udev.cyr:589-601`) — dropped `SYS_SOCKET = 41`,
  `SYS_BIND = 49`, `SYS_SETSOCKOPT = 54`, `SYS_POLL = 7`,
  `SYS_RECVFROM = 45` from the netlink constants enum. The
  socket-family numbers now resolve through
  `src/syscalls.cyr`; `SYS_POLL` is gone entirely (replaced
  by `SYS_PPOLL`). The enum keeps the truly-arch-stable
  socket constants (`AF_NETLINK`, `SOCK_DGRAM`,
  `SOCK_CLOEXEC`, `NETLINK_KOBJECT_UEVENT`, `SOL_SOCKET`,
  `SO_RCVBUF`, `RECV_BUF_SIZE`).

### Verified

- Build clean on 5.7.48: `OK`, only the expected `dead:
  sakshi_error` import-unused report.
- `cyrius lint` 0 warnings, `cyrfmt --check` 0 drift across
  every `src/*.cyr`, `programs/*.cyr`, `tests/tcyr/*.tcyr`,
  `tests/bcyr/*.bcyr`, `fuzz/*.fcyr`.
- 594/594 unit tests, 3/3 fuzz harnesses (uevent, mount
  table, partition table), bench suite running cleanly,
  `core_smoke` PASS, kernel-safe invariant holds (zero
  `alloc`/`sys_*`/`syscall` references in
  `dist/yukti-core.cyr`).
- aarch64 cross-build: all 5 CI targets emit valid ARM ELFs
  (`yukti`, `core_smoke`, three fuzz harnesses). Remaining
  `syscall arity mismatch` warnings are entirely inside
  stdlib `lib/syscalls_*.cyr` wrappers and patra dep —
  yukti src/ is now clean of direct-syscall arity issues.
- Both dist profiles regenerate clean (`yukti.cyr` 5417
  lines, `yukti-core.cyr` 451 lines, both v2.1.4). Lockfile
  unchanged (sakshi 2.0.0, patra 1.9.2).

### Held

- **Runtime SIGILL retest on real Cortex-A72** (the original
  5.4.6 codegen bug, still untested against any of
  5.5.x → 5.7.x's aarch64 backend fixes). Unchanged from
  2.1.3 — hardware-bound, not a code change. Tracked in
  `docs/development/issues/2026-04-19-cc5-aarch64-repro.md`.

## [2.1.3] — 2026-04-30

Modernization sweep + aarch64 portability + roadmap docs.
Toolchain pin steps from Cyrius 5.5.11 to 5.7.48 with the
project's standard closeout-pass discipline. Replaces the
intended-but-skipped 2.1.2 (CI gated on aarch64 cross-build,
which surfaced 30 raw `SYS_OPEN`/`SYS_CLOSE`/`SYS_UNLINK`
callers in yukti src/ + the patra 1.1.1 dep that hadn't done
the same migration; addressing both was scoped beyond a
docs-only patch, so the in-flight 2.1.2 work rolls forward to
2.1.3 with the aarch64 fix included). Adds the 2.2.0
audio-domain roadmap so the requirements blocking vani 0.3.x
live here rather than only in a sibling repo's notes. No
yukti public-API changes. No behavioural shift on x86_64.
Downstream consumers (jalwa / file manager / aethersafha /
argonaut / AGNOS kernel) build and link unchanged.

### Changed

- **Toolchain pin bumped 5.5.11 → 5.7.48** (`cyrius.cyml`). The
  5.5.x → 5.7.x arc is mostly stdlib expansion (json
  pretty-print/streaming/pointer in v5.7.40-5.7.42, sandhi
  HTTP/TLS folded into the toolchain at v5.7.0, regex engine
  v5.7.18, JSON tagged-tree engine v5.7.20, Landlock +
  getrandom syscall wrappers in v5.7.35) and aarch64 backend
  hardening (f64 basic ops v5.7.30, EB() codebuf cap raised
  v5.7.34). v5.7.48 is the closeout-backstop release for the
  longest minor in cyrius history (49 patches across 35 days).
  Two latent language gotchas surface during the bump — both
  audited, neither requires a yukti code change:
  - `var buf[N]` inside a function is **static data**, not
    stack — consecutive calls share backing memory, so any
    `Str` or pointer aliasing into `buf` dangles on the next
    call. Yukti has six such sites (`device.cyr:153`,
    `partition.cyr:358`, `storage.cyr:125`, `udev.cyr:666`,
    `udev_rules.cyr:246`, `udev_rules.cyr:293`); all six are
    safe today because the parsing-bound sites pass through
    `str_from_buf` (`alloc + memcpy` at `lib/str.cyr:283-284`)
    before any `Str` escapes, and the syscall-buffer sites
    only do scalar i64 loads. Build warning to watch for:
    "large static data (N bytes)".
  - 5.x stdlib lookup helpers (`toml_get`, `args_get`, etc.)
    take cstr keys, not `Str`; passing `str_from("…")` silently
    returns 0 because `str_eq_cstr` calls `strlen` on a NUL-less
    Str. Yukti uses `map_*` exclusively (cstr-keyed via
    `map_new()`), with bare-cstr literals or `str_cstr(s)` at
    every call site (`linux.cyr:80,95`, `udev.cyr:67`,
    `udev.cyr:298,301,304,309,316,322,327,330,493,515`). No
    consumers of the affected helpers in yukti src/.

  Sandhi (HTTP/TLS service-boundary stdlib) is now available
  via `lib/sandhi.cyr`; not pulled into yukti's `[deps] stdlib`
  because yukti has no HTTP surface.

  Full gate verified on 5.7.48: build 0 warnings in yukti code,
  `cyrius lint` 0 warnings across every `src/*.cyr`,
  `programs/*.cyr`, `tests/tcyr/*.tcyr`, `tests/bcyr/*.bcyr`,
  `fuzz/*.fcyr`, `cyrfmt --check` 0 drift across the same
  surface, `cyrius vet` clean (2 deps, 0 untrusted, 0 missing),
  594/594 tests pass (+2 from the patra 1.9.2 bump),
  3/3 fuzz targets pass (uevent / mount table / partition table),
  `core_smoke` PASS, kernel-safe invariant holds (zero
  `alloc`/`sys_*`/`syscall` references in `dist/yukti-core.cyr`),
  both dist profiles regenerate clean. Binary 341 KB → 384 KB
  (~13% growth from the json/freelist/chrono stdlib expansion
  net of the json drop and patra dep refresh).

  Notable additions yukti doesn't currently exercise but worth
  flagging for follow-on work:
  - `cyrius smoke` / `cyrius soak` subcommands (v5.7.38) —
    natural fit for `programs/core_smoke.cyr` to replace the
    current manual build+run dance.
  - `cyrius api-surface` (v5.7.33) — public-API diff gate; could
    formalize yukti's stable surface for jalwa / argonaut /
    aethersafha / AGNOS kernel.
  - `lib/security.cyr` Landlock + `lib/random.cyr` getrandom
    (v5.7.35) — useful for path-traversal hardening on mount
    points (currently defense-in-depth at MED-2 in the
    2026-04-19 audit).
  - `lib/test.cyr` v1 with `test_each` table-driven dispatch
    (v5.7.43) — could compress the 594-assertion suite.

- **`[deps.patra]` bumped 1.1.1 → 1.9.2** (`cyrius.cyml`). Eight
  intervening minor releases since yukti's previous pin
  (`1.5.x` slab + perf, `1.6.x` `COL_BYTES`, `1.7.x` `INSERT
  OR IGNORE` + STR-keyed B+ tree, `1.8.x` group commit +
  prepared statements + page-slab allocator, `1.9.x`
  `json_build → patra_json_build` rename + the matching
  aarch64 syscall-wrapper migration that 2.1.3 needs from
  yukti's side). Yukti consumes patra exclusively through
  `src/device_db.cyr`, which calls only the stable
  `patra_open` / `patra_exec` / `patra_query` /
  `patra_result_*` / `patra_close` surface — none of those
  changed signatures across the 1.x line, so no yukti
  call-site edits required. Lockfile refreshed (sakshi 2.0.0
  unchanged).

- **Syscall wrapper migration — 30 sites across 6 modules**
  (the aarch64 unblock; pairs with patra 1.9.2's matching
  migration on its side). aarch64's syscall table omits
  `SYS_OPEN` (legacy x86 number 2) and `SYS_UNLINK` (87) —
  the kernel exposes only the AT-variants on arm64
  (`SYS_OPENAT = 56`, `SYS_UNLINKAT = 35`). Yukti's previous
  raw `syscall(SYS_OPEN, …)` / `syscall(SYS_UNLINK, …)` /
  `syscall(87, …)` callers were undefined-symbol or wrong-
  number on aarch64. Migrated:
  - `src/optical.cyr` — 1 `sys_open` + 8 `sys_close`
    (eject / close / status / TOC read / audio rip).
  - `src/partition.cyr` — 1 `sys_open` + 9 `sys_close`
    (MBR + GPT readers).
  - `src/udev_rules.cyr` — 1 `sys_open` + 1 `sys_close` +
    1 `sys_unlink` (the latter was a raw `syscall(87, …)`
    with no symbol — most fragile of the lot).
  - `src/storage.cyr` — 2 `sys_open` + 2 `sys_close` (eject
    + delete-partition paths).
  - `src/udev.cyr` — 2 `sys_close` (sysfs walk).
  - `src/network.cyr` — 1 `sys_close` (was raw
    `syscall(3, fd)` after a connect probe).

  Plus dropped the local `enum EjectConst` shadowing of
  `SYS_OPEN = 2` / `SYS_CLOSE = 3` / `SYS_IOCTL = 16` in
  `src/storage.cyr:670` — the stdlib provides arch-correct
  values for all three (notably `SYS_IOCTL = 16` on x86_64
  vs `29` on aarch64 — the local hardcoded 16 was silently
  wrong on aarch64). Verified: `cyrius build --aarch64
  src/main.cyr` produces `ELF 64-bit LSB executable, ARM
  aarch64, version 1 (SYSV), statically linked`, as do
  `programs/core_smoke.cyr` and all three fuzz harnesses.

  **Held for follow-on**: yukti still has direct raw-number
  `syscall(N, …)` calls for syscalls that don't have stdlib
  wrappers — `clock_gettime` (228 x86 / 113 aarch64),
  `mount` (165 / 40), `socket` (41 / 198), `connect`
  (42 / 203), `write` (1 / 64), `exit_group` (60 / 94),
  `stat` (4 / no-aarch64), `mkdir` (83 / no-aarch64). These
  use arch-divergent numbers hardcoded for x86_64; the
  aarch64 binary builds and links but would call the
  *wrong* syscall at runtime. Tracked under the existing
  held-aarch64 issue
  (`docs/development/issues/2026-04-19-cc5-aarch64-repro.md`)
  alongside the original 5.4.6 SIGILL retest — both block
  the same downstream goal (real-hardware aarch64 yukti).
  Not 2.1.3 scope.

- **`"json"` dropped from `[deps] stdlib`** (`cyrius.cyml`)
  + matching `include "lib/json.cyr"` removed from
  `src/lib.cyr`. Cleanup deferred from the 2.1.2 work
  (where it was deferred because patra 1.1.1 still vendored
  its own `json_build` and a no-stdlib-json bundle would
  have changed the dist resolution order). With patra 1.9.2
  now using `patra_json_build` and shipping the stdlib
  helpers it actually needs through its own deps, yukti
  cleanly drops the unused stdlib include.
  `device_info_to_json` continues to use `str_builder`
  directly — no stdlib `json_*` helper consumers in yukti
  src/.

- **CI workflows hardened on aarch64**
  (`.github/workflows/{ci,release}.yml`). The pre-2.1.3
  skip-on-missing-cc5_aarch64 logic is replaced with a
  hard requirement: cyrius 5.7.43+ ships `cc5_aarch64` in
  the x86_64 release bundle, and yukti+patra are now
  cross-build-clean, so the gate enforces real verification
  on every push instead of silently passing.

- `docs/development/roadmap.md` gains a new "Next minor — 2.2.0"
  section detailing the audio device discovery work needed to
  unblock vani v0.3.0 #8 (typed yukti audio descriptor adapter)
  and #9 (multi-device routing). Spelled out as a concrete
  punch list:
  - `src/audio.cyr` enumerator over `/dev/snd/` + `/proc/asound/`
  - `AudioDeviceInfo` struct (card, device, subdevice,
    direction, name, driver, hw_id)
  - `yukti_audio_devices()` + direction / card filters
  - udev hotplug subscription for `SUBSYSTEM=sound`
  - `device_db` persistence scoped by hw_id
  - Vani descriptor adapter API: `yukti_audio_card(d)` /
    `_device` / `_subdevice` / `_direction` / `_name` / `_hw_id`
  Capability queries explicitly stay on the vani side
  (`audio_query_caps` via `SNDRV_PCM_IOCTL_HW_REFINE`); yukti
  reports presence + identity only.
- `vani — audio device discovery → descriptor → open` added to
  the "Ecosystem Integration" list, gated on the new audio
  module.
- `src/device.cyr` re-formatted (lines 222-263, `device_info_to_json`
  body): pre-existing 8-space indentation dedented to the canonical
  4-space. The 5.5.11 `cyrfmt` was tolerant; 5.7.x catches it. Pure
  whitespace, no semantic change.
- aarch64 retest note (CHANGELOG 2.1.0 "Investigated, held",
  `docs/development/cyrius-usage.md`, `roadmap.md`) rolls
  forward from "pending retest on 5.5.11" to "pending retest
  on 5.7.48" — the 5.7.30 f64 fixes, 5.7.34 codebuf cap raise,
  and 2.1.3's syscall-wrapper migration are the meaningful
  aarch64 deltas since the original 5.4.6 SIGILL repro. Cross-
  builds now succeed on every CI run; the runtime SIGILL
  retest on real Cortex-A72 is the remaining held work, gated
  also on the raw-number-syscall portability follow-on
  flagged above.

## [2.1.1] — 2026-04-20

Housekeeping patch. Toolchain pin bumped from Cyrius 5.4.8 to
5.5.11 with the project's standard closeout-pass discipline —
dead-code audit, stale-comment sweep, version-consistency check,
security re-scan, full clean rebuild, all gates re-run. No new
features, no API changes, no behavioural shift for downstream
consumers (jalwa / file manager / aethersafha / argonaut / AGNOS
kernel). Ship as the last patch of the 2.1.x minor before any
further roadmap work lands.

### Changed

- **Toolchain pin bumped 5.4.8 → 5.5.11** (`cyrius.cyml`). The
  intervening Cyrius 5.4.9–5.5.11 arc is entirely Windows PE /
  Apple Silicon Mach-O / aarch64 backend work — no language-level
  breaking changes for Linux x86_64. Clean bump, zero source edits
  required for the upgrade itself. Full gate verified on the
  5.5.11-pinned toolchain: build 0 warnings in yukti code,
  `cyrius lint` 0 warnings across every `src/*.cyr`, `cyrius vet`
  clean (1 dep, 0 untrusted, 0 missing), 592/592 tests pass,
  3/3 fuzz targets pass, `core_smoke` PASS, both dist profiles
  (`yukti.cyr` / `yukti-core.cyr`) regenerate clean, benchmark
  numbers match the 5.4.8 baseline within noise. Lockfile
  (`cyrius.lock`) unchanged — sakshi 2.0.0 and patra 1.1.1
  tags didn't move. Notable upstream additions yukti doesn't
  currently exercise: `--strict` CLI flag (v5.4.19) escalates
  undef-fn warnings to hard errors; `#ifplat PLAT` / `#endplat`
  preprocessor directives (v5.4.19) as a cleaner alternative to
  `#ifdef CYRIUS_ARCH_*`; fncall arity ceiling raised 6→8
  (v5.4.13). One upstream stdlib warning
  (`lib/syscalls_x86_64_linux.cyr:358: syscall arity mismatch`)
  is flagged by 5.5.11 — known benign, documented in Cyrius
  v5.4.20 CHANGELOG as "standalone include emits warning;
  low-priority cleanup" — will be picked up automatically when
  upstream resolves it.

### Removed

- **`_is_dangerous_action` in `src/udev_rules.cyr`** — private
  helper that recognised `RUN` / `PROGRAM` / `IMPORT` as
  privileged udev-rule action keys. Defined but never wired into
  `validate_rule` (which still returns `Ok(0)` silently when
  those actions appear). Dead code today; if the threat model
  later calls for flagging these keys, the policy re-lands with
  explicit tests rather than an orphaned predicate.
- **`_map_ioctl_error` in `src/optical.cyr`** — private helper
  whose comment read "Read errno from last syscall failure —
  we pass it from caller" but whose body ignored the `op`
  argument and unconditionally returned `err_tray_failed`. Never
  called. Removed rather than completed, since no production path
  needed it and each ioctl call site already composes its own
  specific error.

### Fixed

- **Stale "pending Cyrius 5.2.3 distlib profiles" comment** in
  `programs/core_smoke.cyr` header. Distlib profiles shipped in
  Cyrius 5.4.6 and `dist/yukti-core.cyr` has been produced via
  `cyrius distlib core` ever since. Comment rewritten to describe
  current reality (AGNOS kernel consumes `dist/yukti-core.cyr`
  directly; this smoke binary is just the invariant check).
- **Stale assertion / line-count numbers** across `CLAUDE.md` and
  `docs/development/cyrius-usage.md` — `531 assertions` updated to
  `592`, source line count bumped from `~5270` to `~5490`,
  binary size reference from `~362 KB` to `~341 KB` (matches
  current `cyrius build` output).
- **Stale toolchain version refs** — `README.md` now says
  "Cyrius 5.5.11 or newer" (was `5.2.x`); `CLAUDE.md`, roadmap,
  and threat-model all reference 5.5.11 consistently; the
  held-aarch64 note now flags that the 5.4.6 Cortex-A72 repro
  needs a retest on the 5.5.11 cc5_aarch64 (v5.4.19 added an
  aarch64 `EW` alignment assert; v5.5.11 shipped an Apple
  Silicon Mach-O probe — both touched aarch64 codegen and the
  Linux repro has not yet been re-run).

### Investigated, held

- **Native aarch64**. Cross-build succeeded with Cyrius 5.4.6's
  `cc5_aarch64`, but the produced binaries crashed with `SIGILL`
  on real Cortex-A72 hardware (Raspberry Pi 4, Ubuntu 24.04).
  Faulting PC landed on word `0x800000d6` — an unallocated
  opcode in the ARMv8-A top-level encoding space. Affected every
  yukti target, including the minimal `core_smoke` (no stdlib,
  no syscalls). Diagnosed as a Cyrius compiler codegen bug, not
  a yukti issue. Pending retest on the 5.5.11 toolchain (Cyrius
  v5.4.19 added an `EW` aarch64 alignment assert; v5.5.11 shipped
  an Apple Silicon Mach-O probe — both touch aarch64 codegen,
  and the Cortex-A72 Linux repro has not yet been re-run).
  Reproducer filed at
  `docs/development/issues/2026-04-19-cc5-aarch64-repro.md`;
  one-command retest script at `scripts/retest-aarch64.sh`. The
  CI and release workflow hooks are in place and gated on
  `cc5_aarch64` existing — they stay dormant today and pick up
  automatically once the toolchain ships a fixed compiler.

## [2.1.0] — 2026-04-19

Follow-up release to close out the LOW findings from the 2.0.0
security audit (`docs/audit/2026-04-19-audit.md`) and ship the
near-term roadmap items. CHANGELOG is now the source of truth for
historical work — the roadmap has been trimmed to forward-looking
items only.

### Added
- **Dual-layer / dual-sided optical disc types**:
  `DT_DVD_WRITABLE_DL`, `DT_DVD_ROM_DL`, `DT_DVD_DUAL_SIDED`,
  `DT_BLURAY_DL`, `DT_BLURAY_XL`. Detection covers
  `dvd-r-dl`, `dvd+r-dl`, `dvd-r_dl`, `dvd-rom-dl`, `dvd-ds`,
  `dvd-dual-sided`, `bd-dl`, `bd-r-dl`, `bdxl`, `bd-xl` (case-insensitive).
  New `disc_type_nominal_sectors(dt)` returns the expected sector
  count per family (CD, DVD SL/DL/DS, BD SL/DL/XL) for display
  until the drive reports actual geometry.
- **Audio CD ripping** — `read_audio_sectors(dev, lba, nframes, buf, buflen)`
  wraps the `CDROMREADAUDIO` ioctl (2352-byte CD-DA frames, capped
  at 75 frames per call to bound latency). Higher-level
  `read_audio_track(dev, toc_entry, buf, buflen)` loops the per-
  call cap to rip a whole audio track.
- **Freelist plumbing** for DeviceEvent + UdevEvent — matching
  `device_event_free()` / `udev_event_free()`. Lets transient
  events from the hotplug dispatch loop be reclaimed. Investigation
  on `DeviceInfo` showed freelist overhead regresses the current
  long-lived call pattern by ~30%; kept on bump allocator with a
  no-op `device_info_free()` for API symmetry.
- **New test coverage** (+30 assertions, 562 → 592):
  `test_read_audio_sectors_rejects_bad_input`,
  `test_read_audio_track_rejects_data_track`,
  `test_detect_disc_type_layered`,
  `test_disc_type_nominal_sectors`,
  `test_disc_type_layered_predicates`,
  `test_validate_mount_point_trailing_slash`.
- **`docs/development/threat-model.md`** — full rewrite for the
  Cyrius era (was Rust leftover: `unsafe`, `cargo-deny`,
  `Option<String>`, `bitflags`). Covers trust boundaries, the
  17-row attack-surface matrix, privilege model, supply-chain
  stance, audit cadence, known gaps.

### Security (LOW-severity audit findings from 2.0.0)

- **[LOW-1] sysfs eject input allowlist**. `storage_eject` now
  validates the extracted device basename as `[a-zA-Z0-9_-]{1,32}`
  before composing `/sys/block/<name>/device/delete`. Defense-in-depth
  against path-traversal via crafted `dev_path` (sysfs legitimately
  uses symlinks under `/sys/block/`, so `O_NOFOLLOW` on the full
  path isn't applicable — we gate the untrusted component instead).
- **[LOW-2] TOC integer clamp**. `read_toc` clamps both track
  `length` and `leadout_lba` at 128 M sectors before any
  multiplication. A crafted disc can no longer produce nonsense
  duration / size values via adversarial TOC entries.
- **[LOW-3] Trailing-slash mount blacklist**. Already closed by
  the 2.0.0 MED-1 prefix-matching fix (`_starts_with_dir` treats
  trailing `/` as the component boundary). Regression test added.
- **[LOW-4] Mount-path label cap**. `default_mount_point`
  truncates the sanitized label at 64 chars to bound pathological
  USB labels and leave room under `PATH_MAX` for downstream file
  operations.
- **[LOW-5] Observability on malformed uevents**. `parse_uevent`
  emits `sakshi_warn` on empty `ACTION` / `DEVPATH` instead of
  silently returning 0 — a burst of these during an incident is
  a signal worth chasing.
- **[LOW-6] Threat model rewrite**. See Added section.

### Changed

- `disc_type_is_writable` now also returns true for
  `DT_DVD_WRITABLE_DL` so callers that gate burn UI behave
  correctly for dual-layer writable media.
- `disc_type_has_data` now covers every layered variant
  (DVD DL/DS, BD DL/XL) — previously missed the new variants.
- Roadmap stripped of completed items; CHANGELOG is the authoritative
  history. Remaining roadmap tracks Medium Term + Long Term only.

### Performance

Investigated the "targeted freelist" and "DeviceInfo pool for
enumerate" roadmap items. Findings:

- DeviceEvent (56 B) and UdevEvent (48 B) switched to `fl_alloc`
  with matching `_free` helpers. No bench regression in
  microsecond-resolution event paths; unlocks pool reuse for
  future consumers that track event lifecycle.
- DeviceInfo (168 B) kept on the bump allocator: matched bench
  showed a ~30% regression from fl_alloc overhead because the
  current call pattern (enumerate → cache → listener dispatch)
  has no `_free` call site to amortize against. Pool pays only
  under a churn workload that doesn't exist yet.
- `enumerate_devices` pooling deferred: same constraint —
  returned objects are consumed by long-lived callers. Will
  revisit when jalwa / argonaut expose a bulk-release API.

### Metrics

- **Tests**: 592 assertions (was 559, +33)
- **Source lines**: ~5270 (was ~5180)
- **Binary size**: ~350 KB static ELF (unchanged)
- **dist/yukti.cyr**: 5228 lines
- **dist/yukti-core.cyr**: 451 lines (unchanged — kernel-safe
  subset preserved through 2.x)
- **Fuzz targets**: 3 (unchanged from 2.0.0)

## [2.0.0] — 2026-04-19

First major version bump. 1.3.0 formalized the kernel-safe split
(`core.cyr` + `pci.cyr`) and multi-profile dist bundles; 2.0.0
follows up with a full P(-1) security audit pass, fixing every
HIGH and MEDIUM finding from `docs/audit/2026-04-19-audit.md`.

### Breaking

- **Stricter mount-path validation**. `validate_mount_point()` now
  rejects any path containing `..` or `//`, and the forbidden-root
  list matches both the root itself and everything under it
  (`/etc` + `/etc/foo`), plus new roots: `/var`, `/root`, `/home`,
  `/lib`, `/lib64`, `/srv`, `/opt`. Callers that previously relied
  on being able to mount under `/usr/local` or with un-canonical
  paths must pre-resolve.
- **GPT entry_size must be exactly 128 bytes**. `read_partition_table`
  now rejects GPT headers whose `entry_size` is not 128 (the
  single-size value produced by every real-world GPT writer). Disks
  with vendor extensions using larger entries will fail to parse —
  intentional; see HIGH-2.
- **`trigger_device` / `query_device` reject non-`/sys/` paths**.
  Previously the function accepted arbitrary paths and silently
  failed when udevadm rejected them; now returns `err_udev` on
  non-sysfs input before spawning any subprocess.
- **Udevadm wrappers now use absolute `/usr/bin/udevadm`**. Prior
  releases used a bare `"udevadm"` which `sys_execve` cannot resolve
  (no PATH lookup) — those code paths were effectively dead. Systems
  with udevadm only at `/sbin/udevadm` need to arrange for the
  `/usr/bin` symlink (modern distros already do).

### Security

All fixes below map 1:1 to findings in
`docs/audit/2026-04-19-audit.md`.

- **[HIGH-1] SQL injection via malicious USB descriptor fields** —
  `device_db.cyr` built every `patra_exec` / `patra_query` statement
  by string concatenation, allowing a USB stick advertising a
  crafted `ID_SERIAL` to tamper with the device-history DB.
  Introduced `_sql_escape_str` (doubles single quotes, drops NUL and
  newline bytes) and routed every user-influenced field (`key`,
  `vendor`, `model`, `dev_path`, `mount_point`, `fs_type`, `serial`)
  through it.
- **[HIGH-2] Stack buffer overflow via crafted GPT entry_size** —
  `_parse_gpt_entries` read `entry_size` bytes into a 128-byte stack
  buffer; a malicious disk setting `entry_size > 128` in the GPT
  header triggered stack corruption during partition scan. Parser
  now rejects any `entry_size != 128`.
- **[MED-1] Incomplete mount-point blacklist** — exact-match check
  missed `/var`, `/root`, `/home`, `/lib`, `/lib64`, `/srv`, `/opt`,
  and did not prefix-match (so `/etc/foo` was allowed). Replaced
  with `_starts_with_dir` prefix check over an extended list, plus
  a new `_path_has_traversal` gate that rejects `..` and `//`.
- **[MED-2] Mount TOCTOU (CVE-2026-27456 class)** — between
  `validate_mount_point` and `mount(2)` an attacker with write
  access to the mount parent could symlink the target. `storage_mount`
  now `newfstatat`s the final component with `AT_SYMLINK_NOFOLLOW`
  after `mkdir` and refuses to proceed if the target is a symlink
  or not a directory.
- **[MED-3] `/proc/mounts` truncation at 8 KB** — on container /
  btrfs / snap hosts, `/proc/mounts` exceeds 8 KB and
  `find_mount_point` silently returned false negatives.
  Reader now loops 4 KB chunks until EOF into a `str_builder`
  (capped at 1 MB as a DoS bound).
- **[MED-4] Netlink uevent spoofing (CVE-2009-1185 class)** —
  `udev_monitor_poll` called `recvfrom` with NULL `src_addr`, so
  kernel-origin was never verified. Now passes `sockaddr_nl`,
  checks `nl_pid == 0`, and drops messages with non-zero pid
  (defense-in-depth on pre-hardening kernels).
- **[MED-5] Broken `run("udevadm", ...)` pattern** — existing
  wrappers passed multi-token arg strings into a single argv slot
  and used a relative command name that `sys_execve` cannot
  resolve. Every udevadm caller now builds an argv vec with
  `/usr/bin/udevadm` as absolute cmd and one token per element,
  via `exec_vec` / `exec_capture`. Closes a latent command-injection
  surface that would have opened if `run()` ever grew shell support.

### Added

- `docs/audit/2026-04-19-audit.md` — full P(-1) audit report:
  methodology, 13 findings with file/line references, CVE sweep
  of 10 adjacent kernel/util-linux/udev classes, remediation plan.
- `fuzz/fuzz_partition_table.fcyr` — closes the audit-flagged
  coverage gap: MBR + GPT parser fuzzing via temp-file fixture,
  500 mutation rounds + truncation pass, explicit HIGH-2
  regression check (malicious entry_size must be rejected).
- 28 new test assertions for the security fixes:
  `test_validate_mount_point_blacklist_extended`,
  `test_validate_mount_point_rejects_traversal`,
  `test_sql_escape`, `test_udevadm_sysfs_path_gate`.

### Changed

- CI release and main workflows rewritten for 5.4.6 toolchain +
  multi-dist + kernel-safe tripwire (see 1.3.0 entry).
- `docs/development/roadmap.md` reorganized — LOW audit findings
  scheduled for 2.1.0; `Future` section retained verbatim.

### Metrics

- **Tests**: 559 assertions (was 531, +28 security regressions)
- **Fuzz targets**: 3 (was 2; added `fuzz_partition_table`)
- **Binary size**: ~348 KB static ELF (unchanged)
- **Source lines**: ~5180 (was 5067)
- **dist/yukti.cyr**: 5147 lines
- **dist/yukti-core.cyr**: 451 lines (unchanged — kernel-safe subset
  untouched by security fixes, which is by design)

### Non-findings (verified clean during audit)

- No memory-unsafe primitives in user code (Cyrius has no raw
  pointer arithmetic).
- No libc / FFI.
- No `sys_system()` in `src/`.
- No raw `execve(59)` / `fork(57)` in `src/` (only in audited
  `lib/process.cyr` stdlib).
- No writes to `/etc`, `/bin`, `/sbin`.
- Kernel-safe invariant verified — `dist/yukti-core.cyr` contains
  zero `alloc` / `syscall` / `sys_*` references.

## [1.3.0] — 2026-04-19

### Added
- **`core.cyr`** — kernel-safe core types extracted from `device.cyr`:
  `DeviceClass`, `DeviceState`, `DeviceCapabilities`, struct layouts,
  pure accessors/predicates. Zero alloc, zero syscalls, zero stdlib —
  safe for bare-metal consumption by the AGNOS kernel for PCI device
  identification.
- **`pci.cyr`** — kernel-safe PCI class/subclass/vendor/device tables
  and pure predicates (`pci_class_to_device_type`, `pci_is_storage`,
  `pci_is_nvme`, `pci_is_gpu`, `pci_is_network`, etc.). Same
  kernel-safe discipline as `core.cyr`.
- **`programs/core_smoke.cyr`** — invariant check for the kernel-safe
  subset. Links only `core.cyr` + `pci.cyr` (no `src/lib.cyr`,
  no stdlib) and asserts every exported predicate. Tripwire for
  accidental alloc/syscall additions to the kernel-safe modules.
- **Multi-dist profiles** (requires Cyrius 5.4.6+):
  - `cyrius distlib` → `dist/yukti.cyr` (full userland, 4929 lines)
  - `cyrius distlib core` → `dist/yukti-core.cyr` (kernel-safe, 451 lines)
  - `[lib.core]` section in `cyrius.cyml` declares the profile
- **`docs/development/cyrius-usage.md`** — single source of truth for
  toolchain commands (build, test, bench, fuzz, distlib, deps, release),
  multi-profile dist bundles, quality gates, and Yukti-relevant Cyrius
  conventions. Referenced from `CLAUDE.md`.
- PCI class/vendor lookup tests added to `tests/tcyr/yukti.tcyr`

### Changed
- **Toolchain pin**: `cyrius.cyml` now requires Cyrius 5.4.6+
  (was 5.2.1). Needed for multi-dist profile support (`[lib.PROFILE]`).
- **`CLAUDE.md` restructured** to match the agnosticos first-party
  application template (`docs/development/applications/example_claude.md`
  in the agnosticos repo). Sections now align across AGNOS projects:
  Project Identity, Goal, Current State, Consumers, Dependencies,
  Quick Start, Architecture, Key Constraints, Development Process
  (P(-1) + Work Loop + Security Hardening + Closeout), Key Principles,
  CI/Release, Key References, DO NOT.
- Toolchain-specific commands moved out of `CLAUDE.md` into
  `docs/development/cyrius-usage.md`; `CLAUDE.md` now links there
  instead of duplicating.

### Fixed
- `cyrius fmt --check` now diff-clean across `src/`, `programs/`,
  `tests/`, `fuzz/` (3 files re-formatted: `core_smoke.cyr`,
  `tests/tcyr/yukti.tcyr`, `tests/bcyr/yukti.bcyr`).
- `cyrius lint` now reports 0 warnings across the whole project.
  Previously silent byte-length overflows (Unicode box-drawing chars
  in bench section headers counted as 3 bytes each), duplicate blank
  lines in 7 domain modules, and long one-liner bench declarations.
- `cyrius vet src/main.cyr` clean (1 dep, 0 untrusted, 0 missing).

### Metrics
- **Modules**: 16 (was 14 — added `core.cyr`, `pci.cyr`)
- **Source lines**: 5067 (was 4573)
- **Tests**: 531 assertions (was 485)
- **Binary size**: ~348 KB static ELF
- **Full dist bundle**: 4929 lines (`dist/yukti.cyr`)
- **Kernel-safe dist bundle**: 451 lines (`dist/yukti-core.cyr`)

### Consumers
- AGNOS kernel now consumes `dist/yukti-core.cyr` for PCI device
  identification — same tables userland uses, zero runtime cost.

## [1.2.0] — 2026-04-11

### Added
- **`gpu.cyr`** — GPU device discovery via `/sys/class/drm/`. Enumerates GPU
  devices, reads PCI vendor/device IDs, driver name, and PCI slot from sysfs.
  Known vendors: AMD (0x1002), Intel (0x8086), NVIDIA (0x10DE), VirtIO (0x1AF4).
  API: `enumerate_gpus()`, `gpu_count()`, `gpu_info_report(g)`, plus accessors
  for card name, dev path, sys path, vendor/device ID, driver, PCI slot, and
  render node. Unblocks mabda GPU library port (pre-flight GPU detection).
- **`DC_GPU`** device class added to `DeviceClass` enum (value 7, `DC_UNKNOWN` → 8)

## [1.1.2] — 2026-04-11

### Fixed
- All source files pass `cyrfmt` (indentation, line wrapping)
- All source files pass `cyrlint` (0 warnings — no double blank lines, no lines >100 chars)
- Bundle script now strips consecutive blank lines automatically
- SQL string literals split to stay under 100-char line limit

## [1.1.1] — 2026-04-11

### Fixed
- All private helper functions consistently prefixed with `_` (storage, optical, udev, partition, device_db, network)
- Removed duplicate `str_to_hex()` and `str_to_int()` from udev.cyr (now provided by lib/str.cyr)
- Added inline doc comments to all accessor functions (partition, network, device_db)
- Zero compiler warnings on clean build

## [1.1.0] — 2026-04-11

### Added
- **`partition.cyr`** — MBR and GPT partition table reading
  - MBR: 4 primary entries, 15 known type IDs, boot flag (0x80)
  - GPT: header validation ("EFI PART"), 128-byte entries, mixed-endian GUID formatting
  - 4 known GPT type GUIDs: EFI System, Linux filesystem, Linux swap, Microsoft Basic Data
  - `read_partition_table(dev)`, `find_efi_partition()`, `find_bootable_partitions()`
  - `partition_count()`, `has_efi_partition()`, `read_partition_table_by_name()`
- **`device_db.cyr`** — device database persistence via patra
  - 3 tables: `devices` (known devices), `mount_history` (event log), `preferences` (per-device config)
  - `device_db_record_seen()`, `device_db_record_mount()`, `device_db_is_known()`
  - `device_db_set_preference()` / `device_db_get_preference()` — per-device mount config
  - `device_db_mount_count()`, `device_db_device_count()`
- **`network.cyr`** — network filesystem mount helpers
  - SMB/CIFS and NFS/NFS4 mount via direct syscall with credential and port support
  - `NetworkShare` struct — host, path, fs_type, port, username, password
  - `network_mount()`, `network_unmount()`, `network_list_mounted()`
  - `network_probe_smb()` / `network_probe_nfs()` — TCP connect probe on ports 445/2049
  - `network_mount_source()` — builds `//host/path` (SMB) or `host:/path` (NFS)
- **sakshi_full structured logging** — upgraded from minimal sakshi
  - Span-based instrumentation on mount/unmount/eject, tray control, TOC reading, enumeration, udev monitor
  - `sakshi_span_enter()` / `sakshi_span_exit()` with automatic duration tracking

### Changed
- Include chain (`lib.cyr`) now uses `sakshi_full.cyr` instead of `sakshi.cyr`
- Added `lib/patra.cyr` and `lib/freelist.cyr` as stdlib dependencies
- Bundle script updated to include partition, device_db, network modules
- CI/release workflows updated for Cyrius toolchain (matching patra/sakshi pattern)
- Makefile rewritten for Cyrius build/test/bench/fuzz targets

### Metrics
- **Modules**: 13 (was 10)
- **Source lines**: 4,573 (was 3,359)
- **Tests**: 470 assertions (was 407)
- **Binary size**: 307 KB (was 152 KB — includes patra SQL engine)
- **dist bundle**: 4,477 lines

## [1.0.0] — 2026-04-11

### Changed — **Cyrius Port**
- **Complete rewrite from Rust to Cyrius** — sovereign, zero-dependency implementation
- All 8 modules ported: error, device, event, storage, optical, udev, linux, udev_rules
- Direct Linux syscalls replace libc wrappers: mount(165), umount2(166), ioctl(16), socket(41), stat(4)
- Function pointer callbacks replace Rust trait objects for event listeners
- Manual struct layout with alloc/store64/load64 replaces Rust structs
- Enum integer constants replace Rust enums with derives
- Tagged union Ok/Err replaces Rust Result<T, E>
- sakshi structured logging replaces tracing crate
- Bump allocator replaces Rust ownership/borrowing

### Added
- `src/main.cyr` — CLI device enumeration demo (prints device table)
- `tests/yukti.tcyr` — **407 test assertions** (up from 229 in Rust)
- `benches/bench.bcyr` — 45 batch-timed benchmarks with nanosecond precision
- `fuzz/fuzz_parse_uevent.fcyr` — 1000 mutations + truncation fuzzing for uevent parser
- `fuzz/fuzz_mount_table.fcyr` — 500 mutations + truncation fuzzing for mount table parser
- `BENCHMARKS-rust-v-cyrius.md` — comprehensive Rust vs Cyrius performance comparison
- Extended `lib/str.cyr` with: `str_from_buf`, `str_eq_cstr`, `str_cstr`, `str_substr`, `str_last_index_of`, `str_builder_add_str`, `str_builder_add_byte`, `str_contains_cstr`, `str_index_of_cstr`, `str_to_hex`
- `WIFSIGNALED` and `WTERMSIG` macros to `lib/syscalls.cyr`

### Metrics
- **Binary size**: 152 KB static ELF (vs 449 KB Rust stripped)
- **Source**: 3,359 lines (vs 6,166 Rust)
- **Dependencies**: 0 (vs 47 Rust crates)
- **Tests**: 407 assertions, 0 failures
- **Benchmarks**: 45 operations, batch-timed
- **Fuzz targets**: 2 (parse_uevent, mount_table)

### Archived
- Original Rust source moved to `rust-old/`
- Cargo.toml, Cargo.lock, deny.toml, rust-toolchain.toml archived
- Criterion benchmarks archived as `rust-old/yukti_bench.rs`
- libfuzzer targets archived as `rust-old/fuzz/`

## [0.25.3] — 2026-03-25

### Added
- **`udev-rules` feature**: udev rule management via agnosys integration
  - `udev_rules` module with `render_rule()`, `validate_rule()`, `write_rule()`, `remove_rule()`, `reload_rules()`
  - `trigger_device()`, `query_device()`, `list_devices()` via udevadm
  - Feature-gated behind `dep:agnosys` (optional git dependency, not in `full` or `default`)
  - 13 unit tests
- `full` feature combining `udev`, `storage`, `optical`, `ai`

### Changed
- CI: `--all-features` replaced with `--features full` to avoid requiring private path dependencies
- `deny.toml`: switched to `features = ["full"]`, allow agnosys git source
- Release workflow: strip private deps before `cargo publish`

## [0.22.3] — 2026-03-22

### Added
- `device` module: `DeviceInfo`, `DeviceId`, `DeviceClass` (8 types), `DeviceState`, `Device` trait
- `DeviceCapabilities` bitflags (u16) replacing `Vec<DeviceCapability>` — O(1) membership checks with serde compatibility (serializes as array)
- `DeviceCapability` enum (10 variants) with `.flag()` conversion to bitflags
- `DeviceInfo::display_name()` returns `Cow<str>` (zero-alloc for label/model paths)
- `DeviceInfo::size_display()` returns `Cow<'static, str>` (zero-alloc for "unknown")
- `event` module: `DeviceEvent`, `DeviceEventKind`, `EventListener` trait, `EventCollector` (thread-safe)
- `EventCollector::with_events()` — zero-copy event access via closure
- `storage` module: `Filesystem` enum (12 types) with zero-allocation case-insensitive parsing via `eq_ignore_ascii_case`
- `MountOptions` with builder pattern: `new()`, `mount_point()`, `read_only()`, `fs_type()`, `option()`
- `MountResult`, mount point validation (direct `Path` comparison, no string allocation)
- `find_mount_point()` — parses `/proc/mounts` with octal unescape, testable via internal `find_mount_in()` helper
- `mount()` — `libc::mount()` with auto-detect filesystem, already-mounted check
- `unmount()` — `libc::umount2()` with mount point cleanup under `/run/media/`
- `eject()` — optical via `CDROMEJECT` ioctl (RAII fd guard), USB via sysfs `device/delete`, nvme/mmcblk-aware
- `optical` module: `DiscType` (10 types), `TrayState`, `DiscToc`, `TocEntry`, `TrackType`
- `detect_disc_type()` — zero-allocation case-insensitive media type classification
- `open_tray()`, `close_tray()`, `drive_status()` — optical drive ioctl wrappers
- `read_toc()` — reads CD TOC via `CDROMREADTOCHDR`/`CDROMREADTOCENTRY`, computes track lengths and durations (75 frames/sec)
- `udev` module: `UdevEvent` parsing, `classify_device()`, `extract_capabilities()`, `classify_and_extract()` (single-pass)
- `device_info_from_udev()` — builds `DeviceInfo` from udev properties
- `enumerate_devices()` — walks sysfs, builds `DeviceInfo` for disks and partitions
- `UdevMonitor` — netlink socket (`AF_NETLINK`/`NETLINK_KOBJECT_UEVENT`), `poll()`, `run_with_listener()`, `subscribe()` channel API
- `parse_uevent()` — kernel uevent message parser (null-separated key=value)
- `linux` module: `LinuxDeviceManager` implementing `Device` trait with `Arc<DeviceInfo>` cache
- `LinuxDeviceManager::mount()`, `unmount()`, `eject()` by device ID with state tracking
- `LinuxDeviceManager::start_monitor()` / `stop_monitor()` — background hotplug monitoring
- `LinuxDeviceManager::dispatch_event()` — listener dispatch with class-based filtering
- `error` module: `YuktiError` with 15 variants including `AlreadyMounted`, `Timeout`, `UdevSocket`, `UdevParse`
- `From<&str>` and `From<String>` for `DeviceId` and `Filesystem`
- `#[non_exhaustive]` on all 9 public enums
- `tracing` instrumentation on all I/O operations (mount, unmount, eject, tray, monitor, enumerate)
- RAII `OwnedFd` guard for fd management in ioctl paths, named `ENOMEDIUM` constant
- Safe errno access via `std::io::Error::last_os_error()` (no unsafe errno)
- Feature gates: `udev`, `storage`, `optical` (all require `libc`), `ai` (requires `reqwest`, `tokio`)
- Criterion benchmarks: 45 benchmarks across 9 groups with 3-point history tracking
- `scripts/bench-history.sh`, `scripts/version-bump.sh`
- GitHub Actions CI + release workflows, Makefile, deny.toml, codecov.yml
- `docs/`: architecture overview, threat model, roadmap, testing guide
- CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md
- `examples/detect.rs` — device detection, filesystem parsing, disc type detection
- 175 tests (12 hardware tests `#[ignore]`d), clippy clean with `-D warnings`

[Unreleased]: https://github.com/MacCracken/yukti/compare/v2.1.0...HEAD
[2.1.0]: https://github.com/MacCracken/yukti/releases/tag/v2.1.0
[2.0.0]: https://github.com/MacCracken/yukti/releases/tag/v2.0.0
[1.3.0]: https://github.com/MacCracken/yukti/releases/tag/v1.3.0
[1.2.0]: https://github.com/MacCracken/yukti/releases/tag/v1.2.0
[1.1.2]: https://github.com/MacCracken/yukti/releases/tag/v1.1.2
[1.1.1]: https://github.com/MacCracken/yukti/releases/tag/v1.1.1
[1.1.0]: https://github.com/MacCracken/yukti/releases/tag/v1.1.0
[1.0.0]: https://github.com/MacCracken/yukti/releases/tag/v1.0.0
[0.25.3]: https://github.com/MacCracken/yukti/releases/tag/v0.25.3
[0.22.3]: https://github.com/MacCracken/yukti/releases/tag/v0.22.3
