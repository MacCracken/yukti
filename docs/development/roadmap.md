# Roadmap

Forward-looking only. `CHANGELOG.md` is the authoritative record of
completed work — don't duplicate it here.

## Next minor — 2.2.0

### Audio device discovery — vani consumer (added 2026-04-30)

vani 0.1.x ships a stub `vani_open_yukti(desc, direction)` that
returns `VANI_ERR_YUKTI_DESCRIPTOR` because yukti has no audio
domain yet. Vani's roadmap items v0.3.0 #8 (typed yukti audio
descriptor adapter) and #9 (multi-device routing — onboard / USB
/ HDMI) are blocked on this landing. After 2.2.0 ships, vani
0.3.x can fill in those items and then ship 1.0.0.

Domain shape mirrors the existing `gpu` / `optical` / `network`
modules: a flat `src/audio.cyr` enumerator backed by Linux's
`/dev/snd/` and `/proc/asound/` views, surfaced through the
unified `DeviceInfo` flow plus an audio-specific descriptor
struct that carries the fields vani needs to route to
`audio_open_playback` / `audio_open_capture`.

- [ ] `src/audio.cyr` — enumerate `/dev/snd/pcmC{card}D{device}{p|c}`,
      cross-reference `/proc/asound/cards` for card name + driver
      and `/proc/asound/card{N}/pcm{D}{p|c}/info` for PCM device
      names + capabilities flags.
- [ ] `AudioDeviceInfo` struct extending the unified `DeviceInfo`
      shape with the audio-specific fields:
      - `card` (i64, 0..99) — ALSA card number
      - `device` (i64, 0..99) — ALSA device number on that card
      - `subdevice` (i64) — ALSA subdevice (0 for the common case)
      - `direction` (i64) — `YUKTI_AUDIO_PLAYBACK = 0` /
        `YUKTI_AUDIO_CAPTURE = 1` (matches vani's
        `VaniDirection` 1:1 so the adapter is a copy, not a map)
      - `name` (cstr ptr) — friendly name from
        `/proc/asound/card{N}/pcm{D}{p|c}/info`
      - `driver` (cstr ptr) — kernel driver name from
        `/proc/asound/cards` (e.g. `HDA-Intel`, `USB-Audio`)
      - `hw_id` (cstr ptr) — stable identifier across hotplug
        (USB vendor:product string + serial when available;
        falls back to `card{N}_dev{M}_p` for built-in audio)
- [ ] `yukti_audio_devices()` — top-level enumerator returning
      a vec of `AudioDeviceInfo` pointers.
- [ ] `yukti_audio_devices_for_direction(direction)` — filter
      to playback or capture only.
- [ ] `yukti_audio_devices_for_card(card)` — filter by card.
- [ ] udev hotplug subscription for `SUBSYSTEM=sound` so USB
      audio plug / unplug emits the same `DeviceEvent` shape
      the existing block / optical path uses.
- [ ] `device_db` persistence for audio devices — same
      first-seen / last-seen / friendly-name shape as block
      and optical, scoped by `hw_id` so re-plugging the same
      USB DAC carries forward its history.
- [ ] **Vani descriptor adapter API** (the contract vani 0.3.x
      consumes): typed accessors against an `AudioDeviceInfo`
      pointer — `yukti_audio_card(d)`, `yukti_audio_device(d)`,
      `yukti_audio_subdevice(d)`, `yukti_audio_direction(d)`,
      `yukti_audio_name(d)`, `yukti_audio_hw_id(d)`. Stable
      shape so vani's `vani_open_yukti(desc, direction)` can
      route descriptors to `audio_open_playback` /
      `audio_open_capture` with no further translation.

**Out of scope for yukti.** Capability queries (rate / format /
channel range support) belong on the vani side via
`SNDRV_PCM_IOCTL_HW_REFINE` — vani already has
`audio_query_caps` (v0.2.0). Yukti reports presence + identity
only; vani opens the device and asks the kernel what it
actually supports.

**Test surface**: real-HW PASS on at least onboard HDA Generic
+ one USB audio interface + one HDMI output (matches vani's
v0.2.0 #6/#7 hardware coverage roadmap).

### Ecosystem Integration
- [ ] jalwa — hotplug → detect → mount → import pipeline
- [ ] argonaut — policy-driven automount on boot
- [ ] aethersafha — notifications for mount/unmount events
- [ ] **vani — audio device discovery → descriptor → open**
      (lands once `src/audio.cyr` is in place per the section
      above)

### Platform
- [ ] aarch64 native build — cross-compile path is wired, but
      Cyrius 5.4.6's `cc5_aarch64` emitted an unallocated ARMv8-A
      opcode (`0x800000d6`) that `SIGILL`ed on real hardware.
      Needs retest on the 5.7.43 toolchain — the 5.5.x → 5.7.x
      arc lands real aarch64 fixes (EW alignment assert v5.4.19,
      Apple Silicon Mach-O probe v5.5.11, f64 basic ops v5.7.30,
      EB() codebuf cap raised v5.7.34). Cortex-A72 Linux repro
      has not yet been re-run. See
      `docs/development/issues/2026-04-19-cc5-aarch64-repro.md` and
      `scripts/retest-aarch64.sh`.
- [ ] Container-aware enumeration (detect host vs container devices)

## Future

Ideas we're not committing to yet — park here if they're
interesting but not scheduled.

- [ ] Refactor `device_db` SQL layer to patra prepared statements
      once patra grows a `patra_bind_*` API (replaces the string-
      escape defense from 2.0.0 HIGH-1)
- [ ] `open_tree(2)` + `move_mount(2)` path for atomic mount
      (closes the narrow TOCTOU window that `newfstatat` still
      leaves open in 2.0.0 MED-2)
- [ ] Bulk DeviceInfo pool with `enumerate_devices_into(pool)` —
      needs caller lifecycle cooperation, investigate with jalwa
      / argonaut integration
- [ ] Optional compression of mount history records in patra
- [ ] M.2 / NVMe namespace reporting beyond the single-namespace
      default
