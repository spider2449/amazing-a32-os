# Dumps the camera hardware registers while another program drives the camera
# (stock Android's camera on a RAM-booted test image), to copy its settings.
# Skips the ISP interrupt status registers, which clear when read and which the
# kernel's ISP_Irq handler reads (CAM +0x24 .. +0x48 and +0xfc).
# Usage: python3 cam-dump.py OUT.txt
import mmap, os, sys

# (name, mapping base the ISP driver accepts, mapping size, first, last)
RANGES = [
    ("cam", 0x15000000, 0x10000, 0x15004000, 0x15005000),
    ("seninf", 0x15008000, 0x4000, 0x15008000, 0x15009000),
    ("mipi_ana", 0x10215000, 0x3000, 0x10215000, 0x10218000),
]
SKIP = {0x15004000 + o for o in (0x24, 0x28, 0x30, 0x34, 0x3C, 0x40, 0x44, 0x48, 0xFC)}

fd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
lines = []
for name, base, size, first, last in RANGES:
    # The driver accepts only these bases, and only read-write mappings.
    m = mmap.mmap(fd, size, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE, offset=base)
    words = memoryview(m).cast("I")
    for addr in range(first, last, 4):
        value = "skipped" if addr in SKIP else "%08x" % words[(addr - base) // 4]
        lines.append("%s %08x %s" % (name, addr, value))
    words.release()
    m.close()
os.close(fd)
open(sys.argv[1], "w").write("\n".join(lines) + "\n")
print("wrote %d registers to %s" % (len(lines), sys.argv[1]))
