#!/usr/bin/env python3
"""Verify actual ARM32 4.9 outputs; print a reproducible JSON inventory."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import zlib

from importlib.util import module_from_spec, spec_from_file_location

spec = spec_from_file_location("boot_test", Path(__file__).with_name("boot-test-image.py"))
boot_test = module_from_spec(spec)
spec.loader.exec_module(boot_test)


def main():
    root = Path(__file__).resolve().parent.parent
    source = Path(os.environ.get("A32_KERNEL49_DIR", root / "phone-tools/kernel-4.9"))
    out = Path(os.environ.get("A32_KERNEL49_OUT", root / "target/kernel-4.9/build"))
    config_bytes = (out / ".config").read_bytes()
    config = config_bytes.decode()
    if "CONFIG_ARM=y\n" not in config or "CONFIG_ARM64=y\n" in config:
        raise ValueError("expected ARM32 config; do not package as ARM32")
    version = subprocess.check_output(["make", "-s", "-C", str(source), "kernelversion"], text=True).strip()
    if not version.startswith("4.9."):
        raise ValueError(f"unexpected kernel version {version}")
    boot = out / "arch/arm/boot"
    zimage = (boot / "zImage").read_bytes()
    appended = (boot / "zImage-dtb").read_bytes()
    image = (boot / "Image").read_bytes()
    if not 1024 * 1024 <= len(appended) <= 32 * 1024 * 1024:
        raise ValueError("kernel outside 1-32 MiB sanity range (not a partition limit)")
    layout = boot_test.arm_kernel(appended)
    names = re.search(r'^CONFIG_BUILD_ARM_APPENDED_DTB_IMAGE_NAMES="([^"]+)"$', config, re.M)
    if names is None:
        raise ValueError("missing appended DTB selection")
    dtbs = [boot / "dts" / (name + ".dtb") for name in names[1].split()]
    if appended != zimage + b"".join(p.read_bytes() for p in dtbs):
        raise ValueError("zImage-dtb differs from configured DTB concatenation")
    decoded = None
    for match in re.finditer(b"\x1f\x8b\x08", zimage):
        try:
            decoded = zlib.decompress(zimage[match.start():], 31)
            if decoded == image:
                break
        except zlib.error:
            pass
    if decoded != image:
        raise ValueError("zImage gzip payload differs from Image")
    banner = re.search(rb"Linux version ([^\x00\n]+)", image)
    if not banner or not banner[1].startswith(version.encode()):
        raise ValueError("kernel banner does not match make kernelversion")
    candidates = [out / "arch/arm/boot/zImage", out / "arch/arm/boot/zImage-dtb",
                  out / "arch/arm64/boot/Image", out / "arch/arm64/boot/Image.gz",
                  out / "arch/arm64/boot/Image.gz-dtb"]
    candidates += sorted(out.glob("arch/*/boot/dts/**/*.dtb")) + [boot / "Image"]
    candidates += sorted(out.glob("arch/*/boot/dts/**/*.dtb.merge"))
    candidates += sorted(out.glob("**/dtbo.img"))
    inventory = {}
    for p in candidates:
        inventory[str(p.relative_to(out))] = ({"bytes": p.stat().st_size,
            "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} if p.is_file() else {"exists": False})
    exported = root / "target/kernel-4.9/zImage-dtb"
    if exported.exists() and exported.read_bytes() != appended:
        raise ValueError("exported kernel copy is stale")
    print(json.dumps({"architecture": "ARM32 / ARMv7", "kernelversion": version,
        "linux_banner": banner[0].decode(), "kernel_layout": layout,
        "config_sha256": hashlib.sha256(config_bytes).hexdigest(),
        "artifacts": inventory, "scope": "build integrity only; k37 board is not verified for A32"}, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(2)
