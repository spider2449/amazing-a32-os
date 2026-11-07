# Adds the strings that Thumb code loads PC-relatively to an objdump listing:
#   ldr rX, [pc, #n] @ (LIT)   ... later ...   add rX, pc
# gives rX = word(LIT) + address_of_add + 4. When that address holds a printable
# string, the add line gets  ; "string"  appended.
# Usage: python3 annotate.py LIB.so LISTING.txt > OUT.txt   (listing may be .gz)
import gzip, re, struct, sys

lib = open(sys.argv[1], "rb").read()

def segments():
    # Map virtual addresses to file offsets using the ELF program headers.
    phoff, = struct.unpack_from("<I", lib, 0x1C)
    phentsize, phnum = struct.unpack_from("<HH", lib, 0x2A)
    out = []
    for i in range(phnum):
        ptype, offset, vaddr, _, filesz = struct.unpack_from("<IIIII", lib, phoff + i * phentsize)
        if ptype == 1:
            out.append((vaddr, offset, filesz))
    return out

SEGMENTS = segments()

def read(addr, size):
    for vaddr, offset, filesz in SEGMENTS:
        if vaddr <= addr and addr + size <= vaddr + filesz:
            return lib[offset + addr - vaddr:offset + addr - vaddr + size]
    return None

def string(addr):
    raw = read(addr, 1)
    if raw is None:
        return None
    for vaddr, offset, filesz in SEGMENTS:
        if vaddr <= addr < vaddr + filesz:
            data = lib[offset + addr - vaddr:offset + filesz]
            text = data.split(b"\0", 1)[0]
            if len(text) >= 3 and all(32 <= c < 127 or c in (9, 10) for c in text):
                return text.decode()
    return None

LDR = re.compile(r"^\s*([0-9a-f]+):.*\tldr(?:\.w)?\t(r\d+|ip|lr), \[pc, #-?\d+\]\s+@ \(?([0-9a-f]+)")
ADD = re.compile(r"^\s*([0-9a-f]+):.*\tadd\t(r\d+|ip|lr), pc$")
pending = {}
opener = gzip.open if sys.argv[2].endswith(".gz") else open
for line in opener(sys.argv[2], "rt", errors="replace"):
    line = line.rstrip("\n")
    m = LDR.match(line)
    if m:
        word = read(int(m.group(3), 16), 4)
        if word:
            pending[m.group(2)] = struct.unpack("<I", word)[0]
    m = ADD.match(line)
    if m and m.group(2) in pending:
        target = (pending.pop(m.group(2)) + int(m.group(1), 16) + 4) & 0xFFFFFFFF
        text = string(target)
        if text is not None:
            line += '   ; "%s"' % text.replace("\n", "\\n")[:120]
    print(line)
