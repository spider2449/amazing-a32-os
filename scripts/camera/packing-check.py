# Decides how a RAW10 frame is packed: decodes some lines both ways and compares
# the mean difference between neighbouring same-color pixels to the mean level.
# The correct packing gives a smooth image (small ratio); the wrong one mixes
# bits between pixels and looks like noise.
# Usage: python3 packing-check.py frame.raw [stride]
import sys

data = open(sys.argv[1], "rb").read()
stride = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0xCB2
width = 2592

def decode(row, packing):
    if packing == "lsb":
        bits = int.from_bytes(row, "little")
        return [(bits >> (10 * i)) & 0x3FF for i in range(width)]
    out = []
    for i in range(0, len(row), 5):
        lo = row[i + 4]
        out.extend((row[i + k] << 2) | ((lo >> (2 * k)) & 3) for k in range(4))
    return out

for packing in ("lsb", "mipi"):
    diff = level = count = 0
    hist_low = [0] * 4
    for y in range(400, 1600, 100):
        px = decode(data[y * stride:y * stride + width * 10 // 8], packing)
        for x in range(0, width - 2):
            diff += abs(px[x] - px[x + 2])
            level += px[x]
            count += 1
            hist_low[px[x] & 3] += 1
    print("%-4s mean level %.1f, mean neighbour difference %.2f, ratio %.3f, low-2-bit histogram %s" % (
        packing, level / count, diff / count, diff / max(level, 1), hist_low))
