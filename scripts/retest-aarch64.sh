#!/usr/bin/env bash
# retest-aarch64.sh — reproducer + regression guard for the
# cc5_aarch64 SIGILL bug documented in
#   docs/development/issues/2026-04-19-cc5-aarch64-repro.md
#
# Cross-build every yukti target for aarch64, copy to a real
# aarch64 host, run each, report pass/fail. First time every
# target returns 0 is the signal that Cyrius has fixed its
# aarch64 code emitter and yukti can claim native aarch64.
#
# Usage:
#   scripts/retest-aarch64.sh [SSH_TARGET]
#
# Environment:
#   SSH_TARGET    default: runner@agnosarm.local
#   SSHPASS       if set + `sshpass` on PATH, used for password auth
#                 (do NOT export a real password into a shell rc;
#                 pass inline: `SSHPASS=... scripts/retest-aarch64.sh`)
#
# Exit codes:
#   0   every target ran and exited 0 on the aarch64 host
#   1   one or more targets crashed (SIGILL or other)
#   2   prerequisites missing (cc5_aarch64, ssh/scp, host unreachable)

set -e

SSH_TARGET="${1:-${SSH_TARGET:-runner@agnosarm.local}}"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o BatchMode=${SSHPASS:+no}${SSHPASS:-yes}"
REMOTE_DIR="/tmp/yukti-aarch64-retest-$$"

ssh_cmd() {
    if [ -n "$SSHPASS" ] && command -v sshpass >/dev/null 2>&1; then
        sshpass -e ssh $SSH_OPTS "$@"
    else
        ssh $SSH_OPTS "$@"
    fi
}
scp_cmd() {
    if [ -n "$SSHPASS" ] && command -v sshpass >/dev/null 2>&1; then
        sshpass -e scp $SSH_OPTS "$@"
    else
        scp $SSH_OPTS "$@"
    fi
}

echo "=== yukti aarch64 retest ==="
echo "  target: $SSH_TARGET"

# 1. Prerequisites
if [ ! -x "$HOME/.cyrius/bin/cc5_aarch64" ]; then
    echo "  FAIL: ~/.cyrius/bin/cc5_aarch64 not installed"
    echo "  Install it (or copy from a cyrius source build) then retry."
    exit 2
fi

if ! ssh_cmd "$SSH_TARGET" 'uname -m' > /tmp/remote_arch.txt 2>&1; then
    echo "  FAIL: can't ssh to $SSH_TARGET"
    cat /tmp/remote_arch.txt
    exit 2
fi
remote_arch=$(cat /tmp/remote_arch.txt | tr -d '\r\n' | tail -c 20)
if [ "$remote_arch" != "aarch64" ]; then
    echo "  FAIL: $SSH_TARGET is $remote_arch, need aarch64"
    exit 2
fi
echo "  remote: aarch64 ok"

# 2. Cross-build every target
echo ""
echo "=== cross-build ==="
mkdir -p build
targets=()
for spec in \
    "src/main.cyr build/yukti-aarch64" \
    "programs/core_smoke.cyr build/core_smoke-aarch64" \
    "tests/tcyr/yukti.tcyr build/yukti-test-aarch64" \
    "fuzz/fuzz_mount_table.fcyr build/fuzz_mount_table-aarch64" \
    "fuzz/fuzz_parse_uevent.fcyr build/fuzz_parse_uevent-aarch64" \
    "fuzz/fuzz_partition_table.fcyr build/fuzz_partition_table-aarch64"
do
    src=${spec% *}
    out=${spec#* }
    printf "  %-40s " "$out"
    if cyrius build --aarch64 "$src" "$out" > /tmp/build.log 2>&1; then
        echo "ok"
        targets+=("$out")
    else
        echo "BUILD FAIL"
        cat /tmp/build.log
        exit 1
    fi
done

# 3. Transfer + run
echo ""
echo "=== remote run ==="
ssh_cmd "$SSH_TARGET" "mkdir -p $REMOTE_DIR"
scp_cmd "${targets[@]}" "$SSH_TARGET:$REMOTE_DIR/" > /dev/null

fail=0
for bin in "${targets[@]}"; do
    name=$(basename "$bin")
    printf "  %-40s " "$name"
    out=$(ssh_cmd "$SSH_TARGET" "$REMOTE_DIR/$name; echo __rc=\$?" 2>&1)
    rc=$(echo "$out" | sed -n 's/^__rc=//p' | tail -1)
    rc=${rc:-1}
    if [ "$rc" = "0" ]; then
        echo "ok"
    else
        echo "FAIL (exit=$rc)"
        echo "$out" | tail -3 | sed 's/^/      /'
        fail=1
    fi
done

# 4. Cleanup
ssh_cmd "$SSH_TARGET" "rm -rf $REMOTE_DIR" 2>/dev/null || true

echo ""
if [ $fail -eq 0 ]; then
    echo "=== PASS: every aarch64 target ran green on $SSH_TARGET ==="
    echo "  Ready to promote aarch64 from 'held' to 'shipped' in CHANGELOG."
    exit 0
else
    echo "=== FAIL: see docs/development/issues/2026-04-19-cc5-aarch64-repro.md ==="
    echo "  cc5_aarch64 codegen still broken. Stay on 2.1.0."
    exit 1
fi
