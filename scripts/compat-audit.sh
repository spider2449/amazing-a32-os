#!/bin/sh
# Classify an Alpine package failure before blaming the stock Linux 3.18 kernel.
# This script is read-only: it never installs or removes a package.
set -u

package="${1:-}"

echo "===== Amazing A32 compatibility audit ====="
echo

echo "--- kernel ---"
uname -a 2>&1 || true
cat /proc/version 2>/dev/null || true

echo
echo "--- Alpine / apk ---"
if [ -r /etc/alpine-release ]; then
    printf "Alpine: "
    cat /etc/alpine-release
else
    echo "Alpine: unknown"
fi
if command -v apk >/dev/null 2>&1; then
    apk --version 2>&1 || true
    printf "apk arch: "
    apk --print-arch 2>&1 || true
else
    echo "apk: not found"
fi

echo
echo "--- repositories ---"
cat /etc/apk/repositories 2>/dev/null || true

echo
echo "--- namespaces ---"
for name in mnt user pid net; do
    if [ -e "/proc/self/ns/$name" ]; then
        echo "yes $name"
    else
        echo "no  $name"
    fi
done

echo
echo "--- cgroups ---"
cat /proc/cgroups 2>/dev/null || true

echo
echo "--- filesystems ---"
cat /proc/filesystems 2>/dev/null || true

echo
echo "--- registered misc devices ---"
for name in tun loop-control; do
    if [ -e "/sys/class/misc/$name" ]; then
        echo "yes $name"
    else
        echo "no  $name"
    fi
done

if [ -n "$package" ] && command -v apk >/dev/null 2>&1; then
    echo
    echo "--- exact package search: $package ---"
    apk search -x "$package" 2>&1 || true

    echo
    echo "--- apk policy: $package ---"
    apk policy "$package" 2>&1 || true

    echo
    echo "--- simulated install: $package ---"
    apk add --simulate -v "$package" 2>&1 || true
fi

echo
echo "--- runtime tracing ---"
if command -v strace >/dev/null 2>&1; then
    echo "strace: installed"
    echo "For a program that installs but fails at runtime:"
    echo "  strace -f -s 256 -o /tmp/program.strace PROGRAM ARGUMENTS"
    echo "  grep -E 'ENOSYS|EPERM|EOPNOTSUPP|EINVAL|SIGSYS|SIGILL' /tmp/program.strace"
else
    echo "strace: not installed"
    echo "Install it with: apk add strace"
fi

echo
echo "Interpretation:"
echo "  no exact package / no armv7 candidate -> repository or architecture problem"
echo "  relocation / interpreter failure       -> userspace ABI problem"
echo "  ENOSYS                                  -> missing syscall; fallback or newer kernel"
echo "  EPERM/EOPNOTSUPP + namespace/cgroup     -> missing/restricted kernel feature"
