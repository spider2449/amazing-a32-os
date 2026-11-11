#!/usr/bin/env python3
"""Strict Android v0 offline inspection and kernel-only repacking.

The complete header page is retained; base/offsets are never reconstructed from
mkbootimg defaults. Unsupported headers, signatures and unknown data fail closed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


def sha(data):
    return hashlib.sha256(data).hexdigest()


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0]


def align(size, page):
    return (size + page - 1) // page * page


def image_id(sections):
    h = hashlib.sha1()
    for data in sections:
        h.update(data)
        h.update(struct.pack("<I", len(data)))
    return h.digest() + bytes(12)


def arm_kernel(data):
    if len(data) < 48 or u32(data, 36) != 0x016F2818:
        raise ValueError("expected ARM32 zImage (possibly with appended DTB)")
    start, end = u32(data, 40), u32(data, 44)
    length = end - start
    if not 48 <= length <= len(data):
        raise ValueError("invalid zImage length")
    dtbs = []
    offset = length
    while offset < len(data):
        if data[offset:offset + 4] != b"\xd0\x0d\xfe\xed" or offset + 40 > len(data):
            raise ValueError("unrecognized appended kernel data (expected DTB)")
        size = struct.unpack_from(">I", data, offset + 4)[0]
        if size < 40 or offset + size > len(data):
            raise ValueError("truncated appended DTB")
        dtbs.append({"offset": offset, "bytes": size,
                     "sha256": sha(data[offset:offset + size])})
        offset += size
    return {"architecture": "ARM32", "zimage_bytes": length, "appended_dtbs": dtbs}


def parse(data):
    if len(data) < 1632 or data[:8] != b"ANDROID!":
        raise ValueError("unsupported image: expected unwrapped ANDROID! v0")
    if u32(data, 40) != 0:
        raise ValueError("unsupported header version or legacy separate-DT layout")
    page = u32(data, 36)
    if page not in (2048, 4096, 8192, 16384) or len(data) < page:
        raise ValueError("unsupported/truncated header page")
    if any(data[1632:page]):
        raise ValueError("unrecognized nonzero header extension")
    sections = []
    offset = page
    for field in (8, 16, 24):
        size = u32(data, field)
        end = offset + size
        padded = offset + align(size, page)
        if padded > len(data):
            raise ValueError("truncated image section/padding")
        if any(data[end:padded]):
            raise ValueError("unrecognized nonzero section padding")
        sections.append(data[offset:end])
        offset = padded
    if not sections[0] or not sections[1]:
        raise ValueError("kernel and ramdisk must be nonempty")
    info = {
        "header_version": 0, "pagesize": page,
        "kernel_addr": hex(u32(data, 12)), "ramdisk_addr": hex(u32(data, 20)),
        "second_addr": hex(u32(data, 28)), "tags_addr": hex(u32(data, 32)),
        "base_offsets": "not encoded separately; original absolute addresses retained",
        "os_version_raw": hex(u32(data, 44)),
        "name": data[48:64].split(b"\0")[0].decode(errors="replace"),
        "cmdline": (data[64:576].split(b"\0")[0] +
                    data[608:1632].split(b"\0")[0]).decode(errors="replace"),
        "sections": {name: {"bytes": len(b), "sha256": sha(b)}
                     for name, b in zip(("kernel", "ramdisk", "second"), sections)},
        "id_matches": data[576:608] == image_id(sections),
        "payload_end": offset, "trailer_bytes": len(data) - offset,
        "nonzero_trailer": any(data[offset:]),
    }
    info["kernel_layout"] = arm_kernel(sections[0])
    return data[:page], sections, info


def prepare(stock, kernel, output):
    if output.exists() or output.with_suffix(output.suffix + ".json").exists():
        raise ValueError("output or report already exists; select a new output path")
    original, replacement = stock.read_bytes(), kernel.read_bytes()
    header, sections, info = parse(original)
    print(json.dumps({"stock_sha256": sha(original), "stock_header": info}, indent=2))
    if info["nonzero_trailer"]:
        raise ValueError("signed/vendor trailer detected: refusing to retain stale signatures "
                         "or discard unknown data; no test image written")
    if not info["id_matches"]:
        raise ValueError("stock image ID mismatch")
    layout = arm_kernel(replacement)
    if bool(layout["appended_dtbs"]) != bool(info["kernel_layout"]["appended_dtbs"]):
        raise ValueError("replacement changes appended-DTB strategy")
    # Unpack in memory, replace only kernel, retain ramdisk/second byte for byte.
    sections[0] = replacement
    new_header = bytearray(header)
    struct.pack_into("<I", new_header, 8, len(replacement))
    new_header[576:608] = image_id(sections)
    result = bytes(new_header)
    for section in sections:
        result += section + bytes(align(len(section), info["pagesize"]) - len(section))
    if len(result) > len(original):
        raise ValueError("new payload exceeds stock image size; partition capacity not proven")
    # Preserve zero-filled partition extent. No signed trailer reaches this code.
    result += bytes(len(original) - len(result))
    checked_header, checked_sections, new_info = parse(result)
    expected_header = bytearray(checked_header)
    expected_header[8:12] = header[8:12]
    expected_header[576:608] = header[576:608]
    if bytes(expected_header) != header or checked_sections != sections or not new_info["id_matches"]:
        raise ValueError("repack preservation verification failed")
    report = {"stock_path": str(stock.resolve()), "stock_sha256": sha(original),
              "output_sha256": sha(result), "kernel_sha256": sha(replacement),
              "kernel_bytes": len(replacement), "image_bytes": len(result),
              "original_header": info, "output_header": new_info,
              "scope": "packaging only; board compatibility and boot acceptance unverified"}
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as f:
        f.write(result)
    output.with_suffix(output.suffix + ".json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    inspect = sub.add_parser("inspect")
    inspect.add_argument("stock", type=Path)
    prep = sub.add_parser("prepare")
    for name in ("stock", "kernel", "output"):
        prep.add_argument(name, type=Path)
    args = parser.parse_args()
    try:
        if args.command == "inspect":
            data = args.stock.read_bytes()
            print(json.dumps({"sha256": sha(data), "bytes": len(data),
                              "header": parse(data)[2]}, indent=2))
        else:
            prepare(args.stock, args.kernel, args.output)
    except (ValueError, OSError, struct.error) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
