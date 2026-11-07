#!/usr/bin/perl
# Prints the slots of a C++ vtable in an ARM shared library: the byte offset a
# virtual call uses (ldr rX, [vptr, #OFFSET]) and the function it reaches, named
# from the objdump listing's function headers.
# Usage: perl vtable.pl LIB.so LISTING.ann VTABLE_HEX [SLOTS]
use strict;
use warnings;

my ($lib, $listing, $vtable, $slots) = @ARGV;
$vtable = hex $vtable;
$slots //= 40;

open my $fh, '<:raw', $lib or die "$lib: $!";
my $data = do { local $/; <$fh> };
close $fh;

# ELF program headers: map virtual addresses to file offsets.
my ($phoff) = unpack 'V', substr($data, 0x1C, 4);
my ($phentsize, $phnum) = unpack 'vv', substr($data, 0x2A, 4);
my @loads;
for my $i (0 .. $phnum - 1) {
    my ($type, $offset, $vaddr, undef, $filesz) = unpack 'V5', substr($data, $phoff + $i * $phentsize, 20);
    push @loads, [$vaddr, $offset, $filesz] if $type == 1;
}
sub word {
    my ($addr) = @_;
    for my $l (@loads) {
        my ($vaddr, $offset, $filesz) = @$l;
        return unpack 'V', substr($data, $offset + $addr - $vaddr, 4)
            if $addr >= $vaddr && $addr + 4 <= $vaddr + $filesz;
    }
    return undef;
}

my %names;
open my $lf, '<', $listing or die "$listing: $!";
while (<$lf>) {
    $names{hex $1} = $2 if /^([0-9a-f]+) <(.*?)(?:\@\@Base)?>:/;
}
close $lf;

# Slots filled by symbol relocations hold 0 in the file; `readelf -rW -C` output
# (LIB.so.rel next to the library) names their targets.
my %reloc;
if (open my $rf, '<', "$lib.rel") {
    while (<$rf>) {
        # offset info type symvalue symname [+ addend]
        $reloc{hex $1} = $2 if /^([0-9a-f]{8})\s+\S+\s+R_ARM_\S+\s+[0-9a-f]+\s+(.*?)\s*(?:\+ 0)?$/;
    }
    close $rf;
}

# Slot 0 is two words after the vtable symbol (offset-to-top, typeinfo).
for my $k (0 .. $slots - 1) {
    my $at = $vtable + 8 + 4 * $k;
    my $target = word($at);
    last unless defined $target;
    my $func = $target & ~1;
    my $name = $func ? ($names{$func} // '?') : ($reloc{$at} // '?');
    printf "#%-4d 0x%05x  %s\n", 4 * $k, $func, $name;
}
