# A32 LK FDT fixups: offline evidence

[Source provenance](a32-boot-critical-evidence-index.md).
**LK source is not present in the available repository/cache.** Kernel libfdt
source is present, but its fdt_setprop functions are not LK call sites.
No absence-of-fixup conclusion is possible from missing source.

The existing stock partition backup `phone-tools/backup/partitions/lk.img` was
read only. Printable strings give concrete leads at **file offsets**, not load
addresses or executed call sites:

| Offset (decimal) | Evidence |
|---|---|
| 183284 / 183292 | `/memory`; error obtaining fdt_path_offset of memory |
| 183368 / 183388 | `lca_reserved_mem`; `tee_reserved_mem` |
| 238780 / 238800 | `/reserved-memory`; missing reserved-memory warning |
| 238904 | missing reserved dram rank dummy read node warning |
| 244048 | `boot_linux_fdt` |
| 244688 / 244712 / 244748 | fdt_open_into / fdt_check_header / fdt_install_blob failure messages |
| 245024 / 245064 | fdt setup addr/status; setup_fdt failure |
| 246780 / 246832 | `bootargs`; FDT buffer overflow message |
| 286768 / 286808 | reserved DRAM rank0 / rank1 dummy read names |

These establish that the shipped LK contains FDT/memory/reservation handling
machinery. They do not prove the boot path, values or specific write calls.
Source search for `fdt_setprop*`, `fdt_add_subnode`, `fdt_open_into`, `fdt_pack`,
custom fixup and ATAG routines cannot recover unavailable LK source. The added
lexical detector reports leads requiring call-path audit; it never treats no hits
as proof of no modification.

| Property | Original DT | LK modification | Runtime value |
|---|---|---|---|
| `/memory*/reg` | base 0x40000000, size 0x3f000000 | UNKNOWN; memory FDT path strings present | UNKNOWN |
| `/chosen/bootargs` | tty0 + ttyMT0 921600n1, ram root/initrd/loglevel/Android hardware | UNKNOWN; bootargs and setup_fdt strings present | UNKNOWN |
| `linux,initrd-start/end` | Absent; chosen bootargs has initrd=0x44000000,0x1000000 | UNKNOWN; image header ramdisk address is not final DT | UNKNOWN |
| Serial number / serialno | No proven final serial property | UNKNOWN | UNKNOWN |
| RAM size / ranks | Static 1008 MiB | UNKNOWN; mblock/rank strings present | UNKNOWN |
| reserved-memory | ATF, CCCI, connectivity, ram_console, minirdump, pstore | UNKNOWN; explicit reserved-node strings present | UNKNOWN |
| Modem memory | Dynamic reservation size 0x3810000, alignment 0x2000000 | UNKNOWN | UNKNOWN |
| Framebuffer | Display-related DT exists; no proven final framebuffer allocation | UNKNOWN | UNKNOWN |
| ATF / secure memory | Explicit 0x43000000..0x4302ffff; tee_reserved_mem string in LK | UNKNOWN | UNKNOWN |

Boot and recovery partition backups are not post-fixup memory captures. Existing
TEE images contain memory-map messages but no evidence assigning the missing top
16 MiB. No serial number, certificate or proprietary data was committed.

Needed evidence: authentic matching LK/preloader source or an existing post-fixup
FDT / boot log with provenance. A deeper offline disassembly can narrow individual
write paths but has not established runtime values here. **Fixup readiness NO.**
