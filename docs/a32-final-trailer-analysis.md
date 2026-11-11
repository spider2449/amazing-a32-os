# A32 final trailer and BootSignature structure

Offline only; inputs and reproduction are in the
[evidence index](a32-boot-critical-evidence-index.md). No signing or bypass was
attempted. `out/boot-signature-layout.json` and `out/final-trailer-analysis.json`
are generated locally, ignored, and not committed.

## Variable-length ASN.1 structure

All offsets below are relative to the Android logical image end. Lengths include
tag/length bytes unless stated. They are **parsed from DER**, not a hardcoded
certificate/signature offset. Recovery has a longer target and different lengths.

| Section | Boot offset / length | Recovery offset / length | OTA offset / length | Type |
|---|---|---|---|---|
| Outer object | 0 / 1320 | 0 / 1324 | 0 / 1320 | SEQUENCE |
| formatVersion | 4 / 3 | 4 / 3 | 4 / 3 | INTEGER 1 |
| certificate | 7 / 1025 | 7 / 1025 | 7 / 1025 | X.509 SEQUENCE; 1021 content bytes |
| algorithmIdentifier | 1032 / 13 | 1032 / 13 | 1032 / 13 | SEQUENCE; sha256WithRSAEncryption OID |
| authenticatedAttributes | 1045 / 15 | 1045 / 19 | 1045 / 15 | SEQUENCE |
| target | 1047 / 7 | 1047 / 11 | 1047 / 7 | PrintableString /boot or /recovery |
| signedLength | 1054 / 6 | 1058 / 6 | 1054 / 6 | INTEGER (4 content bytes) |
| signature TLV | 1060 / 260 | 1064 / 260 | 1060 / 260 | OCTET STRING |
| signature content | 1064 / 256 | 1068 / 256 | 1064 / 256 | RSA-2048 signature, verified below |
| Zero gap | 1320 / 728 | 1324 / 724 | 1320 / 728 | All zero, outside DER |
| Final opaque block | 2048 / 256 | 2048 / 256 | 2048 / 256 | UNKNOWN |

Absolute logical ends: boot 8,820,736; recovery 10,688,512; OTA 8,822,784.
The active envelope ends at logical_end + 2304. Partition backups have additional
zero fill; OTA ends there. Do not confuse final active block with the last 256
bytes of the entire zero-padded partition dump.

All three certificates are identical. Repeated first signature verification over
`logical image || original authenticatedAttributes DER TLV` returns **Verified OK**
with that certificate's public key. This is CONFIRMED cryptographic consistency,
not a statement about a bootloader trusted key or acceptance policy.

## BootSignature implementation comparison

The available repository, Linux source and local cache contain no matching
BootSignature.java, boot_signer, signapk source or AOSP verified-boot-v1 snapshot.
Kernel module signing and dm-verity source are different formats. No AOSP source
was downloaded. Existing project docs and the five-element schema support the
BootSignature v1 analogy, but there is no local standard implementation to compare
byte-for-byte or prove the vendor's extension handling.

One content-search hit is a **build log**, not an implementation:
`S/drivers/misc/mediatek/gpu/gpu_mali/mali_bifrost/mali-r5p0/drivers/gpu/arm/midgard/log:117`
contains an `out/host/linux-x86/bin/boot_signer /boot ... verity.x509.pem ...`
invocation for another reference product. It supports format lineage only;
the referenced executable/source and that product's signing inputs are not used.

Classification: **BootSignature-v1-like**; structural analogy LIKELY,
ASN.1 boundaries and first RSA/SHA256 verification CONFIRMED. The extra 256 bytes
are outside the parsed DER and are not authenticated by that first verification.
They may participate in another verification layer; coverage remains UNKNOWN.

## Cross-image final-block comparison

| Pair | Different byte positions / 256 | Equal positions |
|---|---|---|
| boot / recovery | 254 | 2 |
| boot / OTA | 256 | 0 |
| recovery / OTA | 255 | 1 |

Every differing offset, both bytes and XOR value are retained in the local JSON.
Entropy is the empirical 256-byte histogram entropy, not a randomness proof.

| Block | Entropy bits/byte | SHA1 | SHA256 |
|---|---|---|---|
| boot | 7.158503210927493 | `476b8d916c2a8a199612a55be1c0a51884aeaceb` | `61b7523f248ab9eb0beef74f7ce988db4120817ae8c1e88d6685578e2ffd3a1f` |
| recovery | 7.096646949350854 | `71d6cc3997fb02056324f3734c058e6905ce5c19` | `65c01348b38e88dd0e77cacc2ba5c933f1c5e9a8409fe2b8c2f0cf616f084d17` |
| OTA | 7.160808251809457 | `23ab385653e05051d34fbd08ccf3d29aa81f4366` | `3fb81276ccea37d6a311ae7c1fae7fb449e99532064e8530b2f47c67c61d489a` |

SHA512:

```text
boot     0fe44792c7e4ab4c1ed0b525662386f38822e85142489b96cdb7e31d830920c359d15a71089dc598ef9c815fdde1ce89d9556812de3a708f73284f7244424566
recovery 75305d3bb4f6f0edb3add58c9dbfaad42997711fdfa5aa607e863e95c8eac5b7212cad7c71aaae3dc4daa24fc8879c93ef14402a427b041e43d68248621b70f9
OTA      1954ad0499a738be5cf4a4353d842d850d90df2bf90706b7ae17a6d48bb88cf6b5db86a28cad8be01f1619390efbfc21402cc6ab05aca852351afd685b937e89
```

## Candidate tests and limits

For each image the script hashes SHA1, SHA256 and SHA512 of:
1632-byte header; full 2048-byte header page; kernel; ramdisk; kernel+ramdisk;
Android logical image; first 2048 envelope bytes; DER object; certificate;
first signature; logical-image+attributes; logical-image+2048-byte-envelope.
Candidate hashes and direct equality checks are stored in local JSON.
No candidate equals the block and no unwrapped SHA1/256/512 digest equals it.
This does not rule out padded/truncated/encoded hash constructions.

With the embedded public key, OpenSSL `dgst` verification tries SHA1/256/512,
PKCS#1 v1.5 and PSS (automatic salt length) for every candidate: **none verifies**.
`pkeyutl -verifyrecover` with PKCS#1 padding fails for all three blocks. A raw
no-padding public RSA operation succeeds but produces no verification proof;
arbitrary in-range integers can do that. No claimed RSA DigestInfo was recovered.
The existing kernel `verity_dev_keys.x509` public certificate is also tested;
its results are separate from the embedded-key results and establish no trust.
Public-key comparison shows that kernel development certificate has the **same
public key** as the embedded certificate (PEM public-key SHA256
`140921493374b2cea45665ce218cb01fca2ce37bda942c7e62837832f4ca81da`), so it
adds no independent key coverage. The existing OTA `META-INF/com/android/otacert`
uses a different key (public-key SHA256
`fa6fd11a7e8e3bbc44be1aac0d5b94f4bc18de720dc99b4b35ed17a20c657fe9`);
it is also tested against all candidate/digest/padding combinations: no signature
verifies and PKCS#1 recovery fails for all three blocks. Neither
certificate's public key is established as a bootloader trust anchor.

| Interpretation | Classification | Evidence / limitation |
|---|---|---|
| Exact 256-byte size, different content in each image | CONFIRMED | Original image bounds and byte comparison |
| Whole DER/ASN.1 object | REJECTED for these blocks | Bounded DER framing fails on all three |
| Plain SHA1 / SHA256 / SHA512 digest | REJECTED | Length differs; explicit full-block comparison negative |
| Constant zero/FF padding | REJECTED | Histogram and cross-image diff |
| RSA-2048 signature under embedded key and tested candidate/padding/digests | REJECTED within tested scope | dgst and PKCS#1 recovery failures |
| RSA-2048 signature using another key/coverage | POSSIBLE | Size is compatible; no recovered key/coverage proof |
| Encrypted / opaque vendor authentication block | POSSIBLE | Entropy/content compatible; no format identification |
| Device-specific checksum | UNKNOWN | No independent device samples or implementation |
| Final block overall | UNKNOWN | No cryptographic verification or complete format proof |

Existing LK strings include `verified boot signature: 1`, signature parse failure,
`Cust1 verify failed`, `cust2 verify failed` and `SECLIB_IMG_VERIFY`. They support
the need to audit multiple verification paths, but do not assign this block to a
specific one. Verification policy remains UNKNOWN. Do not remove or regenerate
the trailer, and do not authorize packaging or boot from these results.
