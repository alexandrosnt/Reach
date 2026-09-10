#!/usr/bin/env bash
# @reach-recipe
# id: disk-report
# name: Disk usage report
# description: Where the space went: largest directories, inode pressure and mounts.
# version: 1.0.0
# author: alexandrosnt
# tags: diagnostics, disk
# targets: debian, ubuntu, rhel, alpine
# danger: benign
# param: SCAN_PATH | Directory to measure | /
# param: TOP_N | How many entries to list | 15
# @end

# Reads only. Safe to run on anything, including a box that is already unhappy.
set -euo pipefail

echo "== Filesystems =="
df -hT -x tmpfs -x devtmpfs 2>/dev/null || df -h

echo
echo "== Inodes =="
# A filesystem can be 20% full and still refuse to create a file. This is the
# check people forget, and the one that explains the confusing failures.
df -i -x tmpfs -x devtmpfs 2>/dev/null || df -i

echo
echo "== Largest directories under ${SCAN_PATH} =="
# One filesystem only: without -x this wanders into /proc and network mounts
# and takes minutes to tell you nothing.
du -xh --max-depth=3 "${SCAN_PATH}" 2>/dev/null \
  | sort -rh \
  | head -n "${TOP_N}"

echo
echo "== Largest single files =="
find "${SCAN_PATH}" -xdev -type f -printf '%s\t%p\n' 2>/dev/null \
  | sort -rn \
  | head -n "${TOP_N}" \
  | awk -F'\t' '{ printf "%8.1f MiB  %s\n", $1/1048576, $2 }'

echo
echo "== Deleted files still held open =="
# The classic "df says full, du says empty": a process holding a deleted file
# keeps the blocks allocated until it exits.
if command -v lsof >/dev/null 2>&1; then
  lsof -nP 2>/dev/null | awk '/deleted/ { sum[$1] += $7 } END { for (p in sum) printf "%8.1f MiB  %s\n", sum[p]/1048576, p }' | sort -rn | head -n 10
else
  echo "lsof not installed - skipping"
fi
