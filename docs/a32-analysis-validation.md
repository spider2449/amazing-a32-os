# Offline A32 analysis validation

2026-11-11; branch kernel-4.9; baseline ed3343c9aaa9039993ed032cccda87b44abe7dbb.
Local detailed evidence is in ignored `out/validation.log`, extraction JSON,
boot/recovery/OTA tail JSON and DTS. Binary evidence is deliberately untracked.

## Checks

| Check | Result |
|---|---|
| New parser/inventory/semantic unit tests | 23 passed |
| Existing boot-image preservation/rejection tests | 6 passed |
| Stock FDT binary structure, size and SHA256 | Valid; 60,245 bytes; documented hash |
| dtc DTB -> DTS -> DTB | Success; all 472 nodes, every raw property and memreserve entry equal |
| dtc version | Existing kernel-built DTC 1.4.1-g53bf130b |
| Legacy unit-name check | Explicitly disabled only unit_address_vs_reg; no force-output flag |
| fdtdump | Not installed; dtc used as requested alternative |
| file / strings / hex / entropy / magic scans | Completed; layout and findings in boot image format document |
| Original Android image IDs | Match in boot, recovery and OTA |
| Embedded RSA/SHA256 signature verification | Verified OK in all three; trust and final block remain UNKNOWN |
| Actual prepare-boot-test.sh on stock input | Exit 2; no image or output report created |
| Stable build artifact and exported copy | Both retain SHA256 903407f54fac63359a73dc20df082e10d44bd0176e7bcb2dd0e72ceaa2a069fc |
| A32 board DTS / DTB build | Deferred because boot-critical evidence is insufficient |
| Offline A32 kernel+DTB candidate | Not created |
| Kernel config modifications | None |
| Device boot/flash/reboot/partition operations | None |

Reproducible test commands:

```sh
python3 scripts/test-a32-formats.py
python3 scripts/test-boot-test-image.py
```

New tests cover valid FDT at a discovered arbitrary offset, invalid/truncated
headers/structure, overlapping blocks, unknown tokens, multiple FDTs, false magic,
container gaps, page padding, nonzero padding, unknown/no/zero trailers, malformed
boot headers, DER length/schema/truncation, references with provider cell counts,
phandle renumbering, symbolic overlay target resolution and missing symbols,
MMC path aliases, missing dtc and CLI failure without artifacts.

The stock DTS roundtrip is an analysis validation, not a Linux 4.9 board DT build.
Unit tests use synthetic data; they neither bundle proprietary fixtures nor touch
devices. Actual stock signature checks are local read-only integration evidence.
No expanded driver test/build is relevant until a board source/config is justified.

## Added files

All 14 tracked additions are text source or documentation:

```text
docs/plans/2026-11-11-a32-stock-dt-migration.md
docs/a32-stock-device-tree.md
docs/a32-stock-dt-inventory.md
docs/a32-dt-semantic-comparison.md
docs/a32-early-boot-debug.md
docs/a32-kernel-config-diff.md
docs/a32-boot-image-format.md
docs/a32-analysis-validation.md
scripts/a32_formats.py
scripts/extract-stock-dtb.py
scripts/analyze-boot-image-tail.py
scripts/a32-dt-inventory.py
scripts/compare-a32-dt.py
scripts/test-a32-formats.py
```

No stock/recovery/OTA image, extracted DTB, kernel, boot image, proprietary blob,
or local evidence binary is staged. Stable artifacts, existing prepare scripts,
kernel source, Makefile and configuration remain unchanged.

## Decision

**NOT READY** for the first non-persistent boot test. Exact blockers and the
evidence required to resolve them are in
[a32-stock-device-tree.md](a32-stock-device-tree.md). The unknown final 256-byte
tail layer and bootloader policy independently keep packaging fail-closed.
