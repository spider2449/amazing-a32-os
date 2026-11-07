# Makes every domain of a binary SELinux policy permissive, by replacing the
# policy's permissive-type bitmap. Used only for a RAM-booted stock Android test
# image, so its camera can be read while it runs; never flash such an image.
#
# Layout (Linux security/selinux/ss/policydb.c, policydb_read): magic, string
# length, "SE Linux", then u32 version, config, sym_num, ocon_num; then for
# version >= 22 the policy-capabilities ebitmap and for version >= 23 the
# permissive-map ebitmap. An ebitmap is u32 mapsize (64), u32 highbit, u32 node
# count, then per node u32 startbit and u64 map.
# Usage: python3 sepolicy-permissive.py IN OUT [highbit]
import struct, sys

MAGIC = 0xF97CFF8C

def read_ebitmap(data, pos):
    mapsize, highbit, count = struct.unpack_from("<III", data, pos)
    if mapsize != 64 or highbit % 64 or count > highbit // 64:
        raise ValueError("unexpected ebitmap at %d: %d %d %d" % (pos, mapsize, highbit, count))
    end = pos + 12 + count * 12
    return (highbit, count), end

def main():
    src, dst = sys.argv[1], sys.argv[2]
    highbit = int(sys.argv[3]) if len(sys.argv) > 3 else 8192
    data = open(src, "rb").read()
    magic, length = struct.unpack_from("<II", data, 0)
    if magic != MAGIC or data[8:8 + length] != b"SE Linux":
        raise ValueError("not a kernel SELinux policy")
    pos = 8 + length
    version, config, sym_num, ocon_num = struct.unpack_from("<IIII", data, pos)
    pos += 16
    print("version %d config %d sym_num %d ocon_num %d" % (version, config, sym_num, ocon_num))
    if version < 23:
        raise ValueError("policy has no permissive map")
    caps, pos = read_ebitmap(data, pos)
    print("policycaps ebitmap highbit %d nodes %d" % caps)
    perm_start = pos
    perm, perm_end = read_ebitmap(data, pos)
    print("permissive ebitmap at %d: highbit %d nodes %d" % ((perm_start,) + perm))
    # Every type permissive: nodes covering bits 0 .. highbit-1, all set.
    nodes = highbit // 64
    new = struct.pack("<III", 64, highbit, nodes)
    for i in range(nodes):
        new += struct.pack("<IQ", i * 64, 0xFFFFFFFFFFFFFFFF)
    out = data[:perm_start] + new + data[perm_end:]
    open(dst, "wb").write(out)
    print("wrote %s: %d -> %d bytes" % (dst, len(data), len(out)))

main()
