# Reports which lines of a captured RAW10 frame were written (bytes other than
# the 0xA5 fill), and basic pixel statistics of the written lines.
# Usage: python3 raw-info.py frame.raw [stride] [lines]
import sys

path = sys.argv[1]
stride = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0xCB2
lines = int(sys.argv[3]) if len(sys.argv) > 3 else 1944
data = open(path, "rb").read()
written = [y for y in range(lines) if data[y * stride:y * stride + 3240].count(0xA5) < 3000]
print("lines written: %d of %d" % (len(written), lines))
if written:
    runs, start, prev = [], written[0], written[0]
    for y in written[1:] + [None]:
        if y != prev + 1:
            runs.append((start, prev))
            if y is not None:
                start = y
        if y is not None:
            prev = y
    print("runs:", runs[:10])
    y = written[len(written) // 2]
    row = data[y * stride:y * stride + 3240]
    # RAW10 packed: 4 pixels in 5 bytes, high 8 bits in the first four bytes.
    pixels = [row[i + k] for i in range(0, 3240, 5) for k in range(4)]
    print("line %d: min %d max %d mean %.1f (8-bit)" % (y, min(pixels), max(pixels), sum(pixels) / len(pixels)))
    print("first bytes:", row[:20].hex())
