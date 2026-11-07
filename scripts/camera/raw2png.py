# Converts a captured RAW10 Bayer frame to a small PNG preview, in pure Python.
# Each output pixel comes from one 2x2 Bayer quad, taking every STEP-th quad,
# with gray-world white balance and a 2.2 gamma.
# Packing: "lsb" = little-endian bit packing (pixel 0 in bits 0-9 of the line),
# "mipi" = four high bytes then one byte of low bits. Bayer order is the color
# of the top-left pixel: R, Gr, Gb or B.
# Usage: python3 raw2png.py in.raw out.png [packing] [order] [step] [stride] [w] [h] [black]
import struct, sys, zlib

src, dst = sys.argv[1], sys.argv[2]
packing = sys.argv[3] if len(sys.argv) > 3 else "lsb"
order = sys.argv[4] if len(sys.argv) > 4 else "R"
step = int(sys.argv[5]) if len(sys.argv) > 5 else 2
stride = int(sys.argv[6], 0) if len(sys.argv) > 6 else 0xCB2
width = int(sys.argv[7]) if len(sys.argv) > 7 else 2592
height = int(sys.argv[8]) if len(sys.argv) > 8 else 1944
data = open(src, "rb").read()

def line(y):
    row = data[y * stride:y * stride + width * 10 // 8]
    if packing == "lsb":
        bits = int.from_bytes(row, "little")
        return [(bits >> (10 * i)) & 0x3FF for i in range(width)]
    out = []
    for i in range(0, len(row), 5):
        lo = row[i + 4]
        out.extend((row[i + k] << 2) | ((lo >> (2 * k)) & 3) for k in range(4))
    return out

# Positions of R and B in a quad; G is the average of the other two.
r_pos = {"R": (0, 0), "Gr": (0, 1), "Gb": (1, 0), "B": (1, 1)}[order]
b_pos = (1 - r_pos[0], 1 - r_pos[1])
quads = []
for qy in range(0, height // 2, step):
    top, bottom = line(2 * qy), line(2 * qy + 1)
    rows = (top, bottom)
    row_out = []
    for qx in range(0, width // 2, step):
        x = 2 * qx
        q = [[rows[0][x], rows[0][x + 1]], [rows[1][x], rows[1][x + 1]]]
        r = q[r_pos[0]][r_pos[1]]
        b = q[b_pos[0]][b_pos[1]]
        g = (q[r_pos[0]][1 - r_pos[1]] + q[1 - r_pos[0]][r_pos[1]]) / 2
        row_out.append((r, g, b))
    quads.append(row_out)

n = sum(len(r) for r in quads)
means = [sum(p[c] for row in quads for p in row) / n for c in range(3)]
black = int(sys.argv[9]) if len(sys.argv) > 9 else 0
gains = [(means[1] - black) / max(m - black, 1) for m in means]
peak = max(max(p) for row in quads for p in row)
print("channel means R G B: %.1f %.1f %.1f, peak %d" % (means[0], means[1], means[2], peak))

def tone(v, c):
    v = max(v - black, 0) * max(gains[c], 0) / max(peak - black, 1)
    return max(0, min(255, int(255 * min(max(v, 0.0), 1.0) ** (1 / 2.2))))

h, w = len(quads), len(quads[0])
raw = b"".join(b"\0" + bytes(tone(p[c], c) for p in row for c in range(3)) for row in quads)
def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")
open(dst, "wb").write(png)
print("wrote %s (%dx%d)" % (dst, w, h))
