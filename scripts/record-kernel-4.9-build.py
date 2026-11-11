#!/usr/bin/env python3
"""Record and verify the built kernel artifacts without rebuilding them."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import zlib

root = Path(__file__).resolve().parent.parent
source = Path(os.environ.get("A32_KERNEL49_DIR", root / "phone-tools/kernel-4.9"))
out = Path(os.environ.get("A32_KERNEL49_OUT", root / "target/kernel-4.9/build"))


def command(*args):
    return subprocess.check_output(args).decode().strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def artifact(path):
    data = path.read_bytes()
    return {"bytes": len(data), "sha256": digest(data)}


boot = out / "arch/arm/boot"
image = (boot / "Image").read_bytes()
zimage = (boot / "zImage").read_bytes()
appended = (boot / "zImage-dtb").read_bytes()
dtb = (boot / "dts/mt6735m.dtb").read_bytes()
decoded = False
for match in re.finditer(b"\x1f\x8b\x08", zimage):
    try:
        if zlib.decompress(zimage[match.start():], 31) == image:
            decoded = True
            break
    except zlib.error:
        pass
if not decoded:
    raise SystemExit("zImage gzip payload does not match Image")
if appended != zimage + dtb:
    raise SystemExit("zImage-dtb does not match zImage + mt6735m.dtb")
if (root / "target/kernel-4.9/zImage-dtb").read_bytes() != appended:
    raise SystemExit("exported zImage-dtb differs from build output")
banner = re.search(rb"Linux version [^\x00\n]+", image)
if banner is None:
    raise SystemExit("Linux version banner missing from decompressed Image")
files = [boot / "Image", boot / "zImage", boot / "zImage-dtb"]
files += sorted(boot.rglob("*.dtb"))
report = {
    "kernelversion": command("make", "-s", "-C", str(source), "kernelversion"),
    "linux_banner": banner.group().decode(),
    "source_commit": command("git", "-C", str(source), "rev-parse", "HEAD"),
    "source_diff_sha256": digest(subprocess.check_output(
        ["git", "-C", str(source), "diff", "--binary", "HEAD"])),
    "config": artifact(out / ".config"),
    "bringup_defconfig": artifact(Path(os.environ.get(
        "A32_KERNEL49_CONFIG", root / "kernel-4.9/configs/a32_bringup_defconfig"))),
    "patches": {p.name: artifact(p) for p in sorted((root / "kernel-4.9/patches").glob("*.patch"))},
    "artifacts": {str(p.relative_to(out)): artifact(p) for p in files},
    "verified": ["zImage decompresses to Image", "zImage-dtb equals zImage + mt6735m.dtb",
                 "exported zImage-dtb matches build output"],
    "validation_scope": "Build and artifact verification only; no device boot or flash performed.",
}
destination = root / "target/kernel-4.9/build-manifest.json"
destination.write_text(json.dumps(report, indent=2) + "\n")
print(destination)
print(json.dumps(report, indent=2))
