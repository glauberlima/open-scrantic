#!/bin/bash
# Usage: sandbox-run.sh <scratch-dir> <wall-seconds> <cmd> [args...]
# OS-enforced sandbox: no network namespace, every writable mount read-only except <scratch-dir>,
# empty environment (allowlisted PATH/HOME/TMPDIR), prlimit CPU/memory/file-size/process caps, wall timeout.
set -euo pipefail
SCR=$(realpath "$1"); WALL=$2; shift 2
exec unshare -n -m -p -f --propagation private /bin/bash -c '
set -e
SCR=$1; WALL=$2; shift 2
mount --make-rprivate /
for m in /dev/shm /run/sandbox-telemetry /; do mount -o remount,ro,bind "$m" 2>/dev/null || true; done
mount -t tmpfs -o size=1m,ro tmpfs /dev/shm 2>/dev/null || true
mount --bind "$SCR" "$SCR"; mount -o remount,rw,bind "$SCR"
exec env -i PATH=/usr/bin:/bin HOME="$SCR" TMPDIR="$SCR" \
  prlimit --as=2147483648 --cpu=60 --fsize=67108864 --nproc=64 --nofile=256 -- \
  timeout -s KILL "$WALL" "$@"
' sandbox "$SCR" "$WALL" "$@"
