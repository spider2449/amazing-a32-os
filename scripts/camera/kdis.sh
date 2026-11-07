#!/bin/sh
# Disassembles one function of the running kernel, from the decompressed kernel
# image, using /proc/kallsyms for its address and length. Call targets are named.
# The image is the gzip payload of the boot image's zImage, loaded at 0xc0008000.
# Usage: sh kdis.sh SYMBOL [IMAGE]
sym=$1
image=${2:-/root/cam/Image}
syms=/tmp/kallsyms.sorted
[ -s $syms ] || sort /proc/kallsyms > $syms
start=$(awk -v s="$sym" '$3 == s { print $1; exit }' $syms)
[ -n "$start" ] || { echo "no symbol $sym"; exit 1; }
stop=$(awk -v a="$start" '$1 > a { print $1; exit }' $syms)
objdump -D -b binary -m arm --adjust-vma=0xc0008000 \
    --start-address=0x$start --stop-address=0x$stop "$image" |
awk -v syms=$syms '
    BEGIN { while ((getline line < syms) > 0) { split(line, f, " "); name[f[1]] = f[3] } }
    /\tbl?\t[0-9a-f]+/ {
        n = split($0, parts, "\t"); target = parts[n]; sub(/ .*/, "", target)
        t = sprintf("%08s", target); gsub(/ /, "0", t)
        if (t in name) $0 = $0 "   <" name[t] ">"
    }
    { print }'
