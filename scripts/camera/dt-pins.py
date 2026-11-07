# Decodes the camera node's pinctrl states from the live device tree: for each
# state name, the pins it selects, their mode and output level.
# Usage: python3 dt-pins.py [NODE]
import os, struct, sys

NODE = sys.argv[1] if len(sys.argv) > 1 else "/proc/device-tree/bus/kd_camera_hw1@15008000"

def cells(path):
    data = open(path, "rb").read()
    return list(struct.unpack(">%dI" % (len(data) // 4), data))

phandles = {}
for root, dirs, files in os.walk("/proc/device-tree"):
    for name in ("phandle", "linux,phandle"):
        if name in files:
            phandles[cells(os.path.join(root, name))[0]] = root

def describe(node):
    out = []
    for root, dirs, files in os.walk(node):
        for prop in sorted(files):
            path = os.path.join(root, prop)
            if prop in ("name", "phandle", "linux,phandle"):
                continue
            raw = open(path, "rb").read()
            if prop == "pinmux" or prop == "pins":
                values = cells(path)
                text = ", ".join("pin %d mode %d" % (v >> 8, v & 0xFF) for v in values)
            elif len(raw) % 4 == 0 and raw:
                text = " ".join("%x" % v for v in cells(path))
            else:
                text = raw.rstrip(b"\0").decode("latin-1")
            out.append("%s/%s = %s" % (os.path.relpath(root, node), prop, text))
    return out

names = open(os.path.join(NODE, "pinctrl-names"), "rb").read().rstrip(b"\0").split(b"\0")
for i, name in enumerate(names):
    path = os.path.join(NODE, "pinctrl-%d" % i)
    for handle in cells(path):
        target = phandles.get(handle, "?")
        print("%s -> %s" % (name.decode(), target))
        if target != "?":
            for line in describe(target):
                print("    " + line)
