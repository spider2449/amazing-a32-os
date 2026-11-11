# A32 boot/recovery/OTA format and trailer forensics

All analysis is offline. Original backups and OTA archive are unchanged.
No replacement boot image, signature stripping/zeroing, bypass, LK/preloader
patch or device operation was performed. All offsets are absolute and ends
exclusive unless explicitly called tail-relative.

## Inputs and reproducibility

| Input | Path | SHA256 |
|---|---|---|
| Boot partition backup | phone-tools/backup/boot-mmcblk0p7.img | `79ef60fc9b0d8330175de1fc8ac60cc839542c24cc4b2993cd3b9772633372d1` |
| Recovery partition backup | phone-tools/backup/recovery-stock.img | `fe20fa9eb91697dc859dcd3e3fe9eff77da024589d362057da12ef09919a69cb` |
| OTA boot.img, extracted unchanged | phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip member boot.img | `fd3b6ae38bc9d9d6978a23f1e9a0dd0b2744924d5156ed2695bb1a89b9daf698` |

The OTA ZIP contains boot.img but no recovery.img member. Use Python zipfile to
copy that member unchanged to ignored `out/ota/boot.img`; this is extraction of
an existing image, not repacking or creating a test image. No network is needed.

```sh
python3 scripts/analyze-boot-image-tail.py phone-tools/backup/boot-mmcblk0p7.img \
  --verify-embedded-signature
python3 scripts/analyze-boot-image-tail.py phone-tools/backup/recovery-stock.img \
  --out out/recovery --verify-embedded-signature
python3 scripts/analyze-boot-image-tail.py out/ota/boot.img \
  --out out/ota --verify-embedded-signature
file out/boot-trailer.bin out/boot-signature.der
strings -a -t x out/boot-trailer.bin
openssl asn1parse -inform DER -in out/boot-signature.der
```

The tool emits full `boot-trailer.bin`, JSON layout/strings/byte distribution/
entropy/magic scans, and bounded hex views. Verification requires existing
openssl; absence is reported, never downloaded. Inspection without verification
does not require openssl. All generated binary evidence stays under ignored out/.

## Android layout comparison

All three have ANDROID! at offset 0, v0-compatible header, pagesize 2048,
second_size 0, zero section padding and no separate-DT header field. Load
addresses agree: kernel 0x40008000, ramdisk 0x44000000, second 0x40f00000,
tags 0x4e000000. Header cmdline is `bootopt=64S3,32N2,32N2` in all three.
The raw OS-version field differs: boot/recovery 0x0e000128, OTA 0x0e00012a.
Header name is `091318` for boot/recovery and `110520` for OTA; original
Android SHA1 section IDs match recomputed values in all three.

| Field (bytes or absolute offset) | Boot | Recovery | OTA boot |
|---|---:|---:|---:|
| Actual file end / length | 16,777,216 | 16,777,216 | 8,825,088 |
| Kernel start | 2,048 | 2,048 | 2,048 |
| Kernel size incl DTB | 6,755,597 | 6,755,597 | 6,756,869 |
| Kernel unpadded end | 6,757,645 | 6,757,645 | 6,758,917 |
| Kernel padded end / ramdisk start | 6,758,400 | 6,758,400 | 6,760,448 |
| Kernel zero padding size | 755 | 755 | 1,531 |
| zImage header-derived size | 6,695,352 | 6,695,352 | 6,696,624 |
| zImage absolute end / DTB start | 6,697,400 | 6,697,400 | 6,698,672 |
| DTB size / totalsize | 60,245 | 60,245 | 60,245 |
| DTB absolute end | 6,757,645 | 6,757,645 | 6,758,917 |
| Ramdisk size | 2,060,911 | 3,929,640 | 2,061,216 |
| Ramdisk unpadded end | 8,819,311 | 10,688,040 | 8,821,664 |
| Ramdisk zero padding size | 1,425 | 472 | 1,120 |
| Ramdisk padded end / second end / logical image end | 8,820,736 | 10,688,512 | 8,822,784 |
| Entire bytes after logical end | 7,956,480 | 6,088,704 | 2,304 |
| Active tail envelope before trailing partition zeros | 2,304 | 2,304 | 2,304 |
| Outer DER sequence size | 1,320 | 1,324 | 1,320 |
| Zero gap after DER up to tail-relative 2048 | 728 | 724 | 728 |
| Unidentified block at tail-relative 2048 | 256 | 256 | 256 |
| Remaining partition zero padding | 7,954,176 | 6,086,400 | 0 |

"Active envelope" includes internal zeros and is not a count of nonzero bytes.
Its end is inferred from the last nonzero byte; OTA length independently supports
the observed 2304-byte envelope. It is not a proven universal vendor format size.

Boot and recovery kernel payloads are byte-identical (SHA256
`991a0d74a28ae7f79b8f5711cc02620b0919778b1b462c27420abef72ab30da6`).
OTA kernel payload SHA256 is
`047d7cedeff1e3dfb6cfd1c47086948d9272032739ebdbc52b77987c3a8d6592`.
All three DTBs are identical:
`2bfe064a24aa130fd126b05d54ef3834b5fce674a22211254a115147fc34c3ba`.
Only one valid FDT and no malformed FDT magic candidate were found in each kernel.
There is no intervening MTK DT container or DTBO header at the appended boundary.
Boot/recovery carry Linux 3.18.35+ dated 2018-09-13; OTA carries 3.18.35+ dated
2020-11-05. A newer OTA kernel does not imply a newer board DT.

## Classification with evidence

| Finding | Classification | Evidence |
|---|---|---|
| Header and section boundaries | CONFIRMED | Header sizes, ARM zImage magic/start/end, page-aligned section layout and zero padding validated independently |
| DER sequence and embedded X.509 certificate | CONFIRMED | openssl asn1parse succeeds; first integer 1, embedded certificate, SHA256/RSA algorithm, attributes sequence and 256-byte signature |
| Android BootSignature v1 naming | BootSignature-v1-like | Narrow schema matches version/certificate/algorithm/target-length attributes/signature pattern; no local standard implementation proves exact vendor equivalence; see [follow-up analysis](a32-final-trailer-analysis.md) |
| First-layer cryptographic consistency | CONFIRMED | All three verify with openssl dgst -sha256 using embedded certificate key over bytes [0, logical_end) followed by original DER attributes |
| Embedded key trusted by device | UNKNOWN | Embedded self-description is not a bootloader trust anchor or authorization |
| Final 256 bytes | UNKNOWN | No known magic, opaque content, different for all three; signature-sized is not proof of RSA or of what it covers |
| Vendor envelope interpretation | LIKELY | Each DER is zero-filled to 2048, then 256 opaque bytes; OTA ends there while partition backups contain further zeros |
| Bootloader enforcement and verified layer(s) | UNKNOWN | No LK/preloader source verification, trusted-key evidence or device testing; cannot infer acceptance from cryptographic consistency |
| Whole trailer / ability to repack safely | UNKNOWN | Unexplained final block and unknown trust policy remain; packaging gate must reject |

`file` reports full tail as data (the later 256-byte block prevents treating the
whole tail as one DER object). `strings` finds Android, android@android.com,
California, Mountain View, certificate date strings and /boot or /recovery.
Known marker scan finds no AVB0, AVBf, ANDROID!, SEANDROIDENFORCE, ASCII BFBF,
binary bfbfbfbf, CERT, RSA, SHA, MediaTek, MTK, SIGN, SIG, DTBO or MTK header
magic in these tails. DER OIDs encode algorithms without those ASCII strings;
absence of strings is not absence of signatures, AVB enforcement or other checks.

The same embedded certificate DER (tail-relative [7,1032)) is present in all:
SHA256 `8ad127abae8285b582ea36745f220ab8fe397ffb3b068df19ca22d122c7b3b86`.
Its subject contains Android; certificate signature OID is SHA1/RSA, whereas the
outer image signature OID is SHA256/RSA. These are separate algorithms/layers.

| First-layer field | Boot | Recovery | OTA |
|---|---|---|---|
| Signed target | /boot | /recovery | /boot |
| Signed length | 8,820,736 | 10,688,512 | 8,822,784 |
| Embedded-key offline verification | Verified OK | Verified OK | Verified OK |
| Signed length equals computed logical end | Yes | Yes | Yes |

The verifier saves the exact certificate, public key, attributes-concatenated
signed input and signature under out/, runs openssl, and records return code.
It neither changes trust settings nor emits a boot image. It does not attempt
verification or bypass of the final 256-byte block.

## Tail content comparison

| Evidence | Boot | Recovery | OTA |
|---|---|---|---|
| Active 2304-byte tail SHA256 | `fd821645279b93556e9dab1133c0f38021743aac7ac94b19b416964d75bc2adc` | `93a60e12057577f7533030912f191ec001f9f336d2d3034dfccffdc79a600e5b` | `eeda27230a7ce9f223d1a3ab87a8b5bbf6b005dae24d41e93e936bb6ca964b3c` |
| Final 256-byte SHA256 | `61b7523f248ab9eb0beef74f7ce988db4120817ae8c1e88d6685578e2ffd3a1f` | `65c01348b38e88dd0e77cacc2ba5c933f1c5e9a8409fe2b8c2f0cf616f084d17` | `3fb81276ccea37d6a311ae7c1fae7fb449e99532064e8530b2f47c67c61d489a` |
| Active tail entropy bits/byte | 6.03649 | 6.05148 | 6.06481 |
| Whole tail entropy bits/byte | 0.00418 | 0.00538 | 6.06481 |

Whole-tail entropy in backups is dominated by partition zeros. Active-tail
entropy includes DER structure and zero gap, so it cannot identify encryption or
a signing scheme. Byte distributions and per-block evidence are saved locally.
Different contents, target attributes and signatures explain different first-layer
bytes. Different final blocks suggest content/target dependence, but three samples
cannot establish the function, key, coverage or algorithm of that final block.

The computed first-layer signed range includes header, kernel+DTB, padded sections
and ramdisk. That range has cryptographic evidence; which layers the bootloader
actually verifies is UNKNOWN. No claim is made that recomputing an Android image
ID or preserving the old tail can make a changed kernel acceptable.

## Packaging policy

`prepare-boot-test.sh` and `boot-test-image.py` remain unchanged and fail closed
with exit 2 for these nonzero tails. Identifying the first layer does not relax
this gate. No trailer is stripped, zeroed or transplanted to a replacement image.
No boot.img or A32 kernel+DTB candidate was generated in this phase.
Status: **NOT READY** for a first non-persistent boot test.
