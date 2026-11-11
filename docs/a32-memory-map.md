# A32 memory map and missing 16 MiB

[Source provenance](a32-boot-critical-evidence-index.md). Address ends below are
**inclusive**; analysis code uses half-open intervals to avoid off-by-one errors.

| Address start | Address end | Size | Owner | Evidence |
|---|---|---|---|---|
| 0x40000000 | 0x7effffff | 0x3f000000 = 1008 MiB | DT-visible DRAM container, includes reservations below | Stock `/memory@00000000/reg`, four 32-bit cells representing 64-bit base/size |
| 0x43000000 | 0x4302ffff | 0x30000 = 192 KiB | ATF reservation | `/reserved-memory/atf-reserved-memory@43000000`, no-map |
| 0x43f00000 | 0x43f0ffff | 0x10000 = 64 KiB | ram_console | Stock fixed reg |
| 0x43f10000 | 0x43feffff | 0xe0000 = 896 KiB | pstore | Stock fixed reg |
| 0x43ff0000 | 0x43ffffff | 0x10000 = 64 KiB | minirdump | Stock fixed reg |
| UNKNOWN | UNKNOWN | 0x3810000 = 56.0625 MiB | CCCI MD1 dynamic reservation | size; 32 MiB alignment; alloc-ranges 0x40000000 + 0xc0000000; no fixed reg |
| UNKNOWN | UNKNOWN | 0x100000 = 1 MiB | Connectivity dynamic reservation | size; 2 MiB alignment; no fixed reg |
| 0x7f000000 | 0x7fffffff | 0x1000000 = 16 MiB | **UNKNOWN; excluded from kernel RAM** | Difference between stock endpoint 0x7f000000 and reference endpoint 0x80000000 |

Fixed reservation intervals do not overlap one another. The DRAM container
intentionally contains them; container/reservation overlap is not a collision
between reservations. Dynamic allocations cannot be assigned addresses or tested
for final placement overlap from this static DT. No memreserve entries exist in
the stock FDT header; `/reserved-memory` still matters.

The exact **arithmetic** explanation is proven: 0x3f000000 / 0x100000 = 1008 MiB.
The **ownership explanation is UNKNOWN**. The missing region is at the top of the
reference 1 GiB span, not automatically an allocation beginning at physical zero.

| Hypothesis for missing 16 MiB | Assessment |
|---|---|
| Preloader / LK retained allocation | POSSIBLE; no matching source or allocation proof |
| TrustZone / secure world / TEE | POSSIBLE; LK has tee_reserved_mem machinery; no address/size proof |
| Modem | POSSIBLE; known dynamic modem size differs from 16 MiB; no placement proof |
| Framebuffer | POSSIBLE; no final LK framebuffer allocation recovered |
| ATF fixed reservation | Does not by itself explain gap: known ATF region is 192 KiB at 0x43000000 |
| ramoops / ram_console / pstore | Known fixed debug regions are elsewhere and smaller; cannot assign gap to them |
| Hardware hole / unavailable DRAM | POSSIBLE; no physical map proof |

LK contains `/memory`, `/reserved-memory`, rank and TEE handling strings, so the
stock embedded DT is not proof of final runtime RAM size. No matching preloader
source, authentic stock memory-reservation source or final FDT was found.
Kernel 4.9 reference allocations are not board evidence.

Policy: preserve stock base and size; never extend memory to 1 GiB. Keep the top
16 MiB excluded by the memory endpoint, without inventing a named reservation
owner. Preserve all fixed/dynamic stock reservations until their consumers and LK
fixups are understood. A future two-cell-parent DTS must encode 64-bit cells;
the shorthand `<0x40000000 0x3f000000>` only applies with one address/size cell.
No memory DTS was written.
