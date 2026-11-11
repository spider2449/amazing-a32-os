# A32 offline evidence index and reproduction

Date: 2026-11-11. Main repository branch `kernel-4.9`, investigation baseline
`6f838bf48371057ffeb942ebf18f14f315356d19`; initial main worktree clean.
This pass changes scripts/tests/docs only. It does not add Rust or operate a device.

## Available and unavailable source

`S` in the migration documents means `phone-tools/kernel-4.9`, resolving in WSL
to `/home/morefunfun/.cache/a32-kernel-casefix/source`, pinned upstream HEAD
`f42baea1a221dea8b56c44d24d931a537a8aea85`, Linux 4.9.117. Existing build
compatibility patches are present (eight modified source paths); they were not
changed in this investigation. HEAD alone does not describe that patched worktree.
The main repository clean state and external source patch state are distinct.

Windows cannot traverse this WSL symlink; source excerpts were first read in
`phone-tools/kernel-4.9-case-backup`. The ten critical PMIC, PWRAP, MMC, UART,
watchdog, ARM/PSCI and clock files were then byte-compared to the live WSL source:
all equal. Local validation JSON records their SHA256. Other reference files are
from the same pinned source family; no reference file is declared A32 stock code.

Inventory covered the repository including ignored backup/build areas and the
local `/home/morefunfun/.cache` source/build roots. Only Linux 4.9 source/build
Makefiles were found. **No authentic kernel 3.18 source, LK source, preloader
source, A32 vendor PRODUCT/PROJECT source or AOSP BootSignature implementation
snapshot is available in those locations.** Build paths embedded in stock Images
are provenance strings, not locally available source directories.

The initial content sweep of the Windows backup found symlink-access errors, so
it was not treated as complete. A separate full live Linux source content scan
read 65,668 text files and recorded 160,382 matching lines (initial scan); binary
and oversized files are not silently represented as text. Search terms include
all requested PMIC/model/rail terms, not just filenames. Findings were narrowed
by Makefile/Kconfig/platform selection and stock binary evidence.
The committed scanner additionally records skipped inputs, CID definitions and
candidate bootloader source fixup leads. Empty results never clear a gate.

Defconfig/generated `.config`, DT base/k37 overlay, headers, driver registrations,
platform devices and supply properties were traced. Existing OTA `boot.img` was
examined; no new OTA was fetched or extracted to the device. OTA metadata and
family source do not supply a board-specific PMIC identity or final FDT.
Archive member inventory additionally covered both backup tar files (14 and 2,108
members) and the existing OTA zip (15 members). No kernel/LK/preloader source
candidate was found; the Alpine archive only contains Python pyconfig.h / Makefile
among source-like matches. OTA metadata identifies `pre-device=Amazing_A32` and
`TaiwanMobile/Amazing_A32/Amazing_A32:7.0/NRD90M/110520:user/release-keys`;
scatter/updater metadata has no PMIC/rail evidence. Archive payloads were not
installed or executed. Local evidence is `out/archive-source-inventory.json`.

## Local stock evidence

| Artifact | Path | Provenance / use |
|---|---|---|
| Boot partition backup | `phone-tools/backup/boot-mmcblk0p7.img` | Android v0 header, stock kernel/DT and trailer; parser records whole-image SHA256 |
| Recovery backup | `phone-tools/backup/recovery-stock.img` | Independent envelope and first-signature check |
| Existing OTA boot | `out/ota/boot.img` | Third envelope; finite 2304-byte tail independent of partition fill |
| Stock DTB | `out/stock-a32.dtb` | 60,245 bytes; SHA256 `2bfe064a24aa130fd126b05d54ef3834b5fce674a22211254a115147fc34c3ba` |
| Decompressed stock kernels | `out/boot-Image`, `out/recovery-Image`, `out/ota-Image` | Read-only string evidence; offsets in PMIC document refer to these files |
| Stock LK backup | `phone-tools/backup/partitions/lk.img` | String offsets in LK/PMIC/watchdog docs are file offsets, not executed call addresses |
| TEE backups | `phone-tools/backup/partitions/tee1.img`, `tee2.img` | Memory strings inspected; no missing-region owner proven |
| Existing public certificate | `S/verity_dev_keys.x509` | Additional negative signature tests only; no device-trust assertion |
| OTA public certificate | Existing zip `META-INF/com/android/otacert`, local `out/ota-public-certificate-0.pem` | Different public key; offline verification only |

No certificate, private key, proprietary image, raw DTB or kernel is committed.
Generated forensic JSON contains hashes, layouts, local verification results and
reference graphs, and remains under ignored `out/`.

## Reproduction (existing local WSL tools only)

At repository root in WSL:

```sh
python3 -c 'import zipfile,pathlib; z=zipfile.ZipFile("phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip"); pathlib.Path("out/ota-public-certificate-0.pem").write_bytes(z.read("META-INF/com/android/otacert"))'

python3 scripts/scan-a32-source-evidence.py \
  /home/morefunfun/.cache/a32-kernel-casefix/source \
  --out out/reproducible-source-scan

python3 scripts/analyze-a32-boot-critical.py \
  --image boot phone-tools/backup/boot-mmcblk0p7.img \
  --image recovery phone-tools/backup/recovery-stock.img \
  --image ota out/ota/boot.img --dtb out/stock-a32.dtb \
  --public-certificate phone-tools/kernel-4.9/verity_dev_keys.x509 \
  --public-certificate out/ota-public-certificate-0.pem

python3 scripts/test-a32-boot-critical.py
python3 scripts/test-a32-formats.py
python3 scripts/test-boot-test-image.py
python3 scripts/check-a32-dts-readiness.py docs/a32-minimal-dts-readiness.json
# Last command intentionally exits 2: NOT READY.
```

The synthetic tests do not require proprietary fixtures. The forensic CLI needs
the original local images and OpenSSL; missing/malformed inputs exit 2, never
enable packaging or create a DTS. The source scanner is lexical evidence, not C
preprocessor execution, call graph proof or hardware detection.

Validation: 36 new tests + existing 23 format/DT tests + 6 boot tests = **65 passed**.
Actual prepare on stock input still exits **2**, without creating its requested
output. Fixed reservation collision check reports none; dynamic placements remain
UNKNOWN. All three first signatures reverify **Verified OK**; no tested final-block
signature verifies with the embedded, kernel development or distinct OTA certificate key.

Both stable artifacts retain SHA256:
`903407f54fac63359a73dc20df082e10d44bd0176e7bcb2dd0e72ceaa2a069fc`:
`target/kernel-4.9/zImage-dtb` and the build copy at
`target/kernel-4.9/build/arch/arm/boot/zImage-dtb`.
No board DTS, generated DTB, new zImage concatenation or boot.img was created.

## Reading order

1. [PMIC](a32-pmic-identification.md), [eMMC/power graph](a32-emmc-migration.md).
2. [Clocks](a32-clock-migration.md), [UART](a32-uart0-migration.md),
   [watchdog](a32-watchdog-handoff.md), [CPU](a32-cpu-handoff.md).
3. [LK fixups](a32-lk-dt-fixups.md), [memory](a32-memory-map.md),
   [DER/final trailer](a32-final-trailer-analysis.md).
4. [Readiness gate](a32-minimal-dts-readiness.md).
