# cyrius — `cc5_aarch64` moved out of `bin/` to tarball top-level in 5.7.48 (install.sh + downstream CIs miss it)

**Filed:** 2026-04-30 (during yukti 2.1.3 release CI failure)
**Cyrius version observed:** 5.7.48 (the regression release; 5.7.42 confirmed still has it under `bin/`)
**Tools affected:** the bundled `install.sh`; any downstream consumer that copies `cyrius-X.Y.Z-x86_64-linux/bin/*` to `~/.cyrius/bin/` (yukti CI, patra CI, vidya CI all follow this pattern)
**Severity:** HIGH (silent: nothing fails at install time, breakage surfaces at first aarch64 cross-build)

## Symptom

yukti's CI fails on the aarch64 cross-build step with:

```
Error: cc5_aarch64 missing from Cyrius  — aarch64 cross-build is required since 2.1.3
Error: Process completed with exit code 1.
```

(The empty version slot in the error string is also a `set -u` -ish
glitch in our gate — the gate runs before `CYRIUS_VERSION` is in
the same shell scope. Cosmetic, not the real bug.)

## Root cause

Layout of `cyrius-5.7.48-x86_64-linux.tar.gz`:

```
cyrius-5.7.48-x86_64-linux/
├── bin/
│   ├── cc5
│   ├── cyrius
│   ├── cyrfmt
│   ├── cyrlint
│   ├── ...
├── cc5_aarch64           ← TOP-LEVEL, not under bin/
├── lib/
│   ├── ...
├── install.sh
└── VERSION
```

Compared to the local install layout from earlier 5.7.x (e.g. 5.7.42):

```
~/.cyrius/versions/5.7.42/
└── bin/
    ├── cc5
    ├── cc5_aarch64       ← was here
    ├── cyrius
    └── ...
```

So `cc5_aarch64` was relocated from `bin/` to the tarball top
level somewhere between 5.7.42 and 5.7.48. The bundled
`install.sh`'s tarball-extract path:

```sh
if [ -d "$EXTRACTED/bin" ]; then
    cp -r "$EXTRACTED/bin"/* "$CYRIUS_HOME/versions/$VERSION/bin/"
    chmod +x "$CYRIUS_HOME/versions/$VERSION/bin"/*
    info "binaries installed"
fi
```

…only copies `bin/*` and never picks up `$EXTRACTED/cc5_aarch64`.
Downstream CI install steps follow the same shape:

```sh
cp "$CYRIUS_DIR/bin/"* "$HOME/.cyrius/bin/" 2>/dev/null || true
```

Net effect: `cc5_aarch64` ships in the tarball but never lands on
disk. Any check like `[ -x "$HOME/.cyrius/bin/cc5_aarch64" ]`
returns false, and aarch64 cross-builds get skipped or fail.

`install.sh` does have an aarch64 path elsewhere (lines 252-254 of
the 5.7.48 copy), but it lives in the **source-bootstrap branch**
— it builds `cc5_aarch64` from `src/main_aarch64.cyr` and copies
it from `./build/cc5_aarch64`. The tarball-extract branch (which
99% of users hit) has no equivalent.

## Workaround applied in yukti 2.1.3

Both `.github/workflows/ci.yml` and
`.github/workflows/release.yml` install steps now do an explicit
defensive copy after the `bin/*` copy:

```sh
[ -f "$CYRIUS_DIR/cc5_aarch64" ] && cp "$CYRIUS_DIR/cc5_aarch64" "$HOME/.cyrius/bin/"
```

This handles both layouts: pre-5.7.48 tarballs (cc5_aarch64 in
bin/, picked up by the `bin/*` copy) and 5.7.48+ tarballs
(cc5_aarch64 at top level, picked up by the explicit copy).

Same workaround should land in patra and vidya CIs since they
follow the same install pattern and will hit the same wall the
moment they exercise aarch64 cross-build.

## Suggested upstream fix

Two paths, either works:

1. **Move `cc5_aarch64` back to `bin/` in the release tarball.** If
   the relocation was accidental (script change, tarball-build path
   regression), revert. Matches every downstream CI's existing
   `cp bin/*` install pattern.
2. **Patch `install.sh`'s tarball-extract path to also copy
   top-level binaries.** Something like:
   ```sh
   for top in "$EXTRACTED"/cc5_aarch64; do
       [ -f "$top" ] && cp "$top" "$CYRIUS_HOME/versions/$VERSION/bin/"
   done
   ```
   Less ergonomic for downstream CIs that bypass install.sh, but
   matches the new packaging without re-relocating the file.

A regression test against the install path (`cyrius install` →
verify `cc5_aarch64` is on `$PATH`) would also have caught this.

## Cross-references

- yukti 2.1.3 CHANGELOG entry — re-enabled aarch64 in CI as a hard
  requirement, which surfaced this immediately.
- yukti `.github/workflows/{ci,release}.yml` — the defensive
  workaround commits.
- patra/vidya CIs — should adopt the same workaround pre-emptively.
