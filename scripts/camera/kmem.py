# Reads words and strings from the decompressed kernel image by virtual address
# (the image is loaded at 0xc0008000), to decode tables the disassembly points at.
# Usage: python3 kmem.py word ADDR [COUNT] | str ADDR | powertable LITERAL_ADDR
import struct, sys

BASE = 0xc0008000
image = open("/root/cam/Image", "rb").read()

def word(addr):
    return struct.unpack_from("<I", image, addr - BASE)[0]

def string(addr):
    if not BASE <= addr < BASE + len(image):
        return "<%08x outside image>" % addr
    raw = image[addr - BASE:addr - BASE + 64]
    return raw.split(b"\0")[0].decode("latin-1")

cmd = sys.argv[1]
addr = int(sys.argv[2], 16)
if cmd == "word":
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    for i in range(count):
        print("%08x: %08x" % (addr + 4 * i, word(addr + 4 * i)))
elif cmd == "str":
    print(string(addr))
elif cmd == "powertable":
    # Literal holds the table address. Entry: char *name, then 10 steps of
    # 3 words (type, value, delay), then 6 more words (148 bytes in all).
    table = word(addr)
    print("table at %08x" % table)
    for i in range(16):
        entry = table + 148 * i
        name_ptr = word(entry)
        if name_ptr == 0:
            break
        words = [word(entry + 4 * k) for k in range(37)]
        print("[%d] %s" % (i, string(name_ptr)))
        for s in range(12):
            step = words[1 + 3 * s:4 + 3 * s]
            if len(step) == 3 and any(step):
                label = string(step[0]) if BASE <= step[0] < BASE + len(image) else "%d" % step[0]
                print("    %-24s %10d %6d" % (label, step[1], step[2]))
