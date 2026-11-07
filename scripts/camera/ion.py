# MediaTek ION buffers for the ISP, the way the stock IMemDrvImp does it
# (libcamdrv.so allocVirtBuf/mapPhyAddr, libion_mtk.so ion_alloc_mm):
# allocate from the multimedia heap, configure the buffer for an M4U port, read
# back the device address (MVA) the ISP DMA uses, and mmap it for the CPU.
# Run directly to test: python3 ion.py [size] [port]
import ctypes, fcntl, mmap, os, struct, sys

def iowr(magic, nr, size):
    return (3 << 30) | (size << 16) | (ord(magic) << 8) | nr

ION_IOC_ALLOC = iowr("I", 0, 20)   # { size_t len; size_t align; u32 heap_mask; u32 flags; int handle }
ION_IOC_FREE = iowr("I", 1, 4)     # { int handle }
ION_IOC_SHARE = iowr("I", 4, 8)    # { int handle; int fd }
ION_IOC_CUSTOM = iowr("I", 6, 8)   # { u32 cmd; unsigned long arg }

HEAP_MULTIMEDIA = 0x400            # ion_alloc_mm
CMD_SYSTEM, CMD_MULTIMEDIA = 0, 1
SYS_GET_PHYS = 1
MM_CONFIG_BUFFER = 0
M4U_PORT_IMGO = 15                 # kernel m4u port name table: ..., VDEC_PPWRAP, IMGO, ...

class IonBuffer:
    def __init__(self, ion_fd, size, port, flags=0):
        self.ion, self.size = ion_fd, size
        arg = bytearray(struct.pack("<IIIIi", size, 0, HEAP_MULTIMEDIA, flags, 0))
        fcntl.ioctl(ion_fd, ION_IOC_ALLOC, arg)
        self.handle = struct.unpack_from("<i", arg, 16)[0]
        # ION_MM_CONFIG_BUFFER: { mm_cmd; handle; eModuleID; security; coherent }
        self._custom(CMD_MULTIMEDIA, struct.pack("<IiiII", MM_CONFIG_BUFFER, self.handle, port, 0, 0))
        # ION_SYS_GET_PHYS: { sys_cmd; handle; phy_addr; len }
        out = self._custom(CMD_SYSTEM, struct.pack("<IiII", SYS_GET_PHYS, self.handle, 0, size))
        self.mva = struct.unpack_from("<I", out, 8)[0]
        share = bytearray(struct.pack("<ii", self.handle, -1))
        fcntl.ioctl(ion_fd, ION_IOC_SHARE, share)
        self.fd = struct.unpack_from("<i", share, 4)[0]
        self.map = mmap.mmap(self.fd, size, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE)

    def _custom(self, cmd, payload):
        data = ctypes.create_string_buffer(payload + b"\0" * (64 - len(payload)), 64)
        fcntl.ioctl(self.ion, ION_IOC_CUSTOM, bytearray(struct.pack("<II", cmd, ctypes.addressof(data))))
        return data.raw

    def close(self):
        self.map.close()
        os.close(self.fd)
        fcntl.ioctl(self.ion, ION_IOC_FREE, bytearray(struct.pack("<i", self.handle)))

if __name__ == "__main__":
    size = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0x100000
    port = int(sys.argv[2]) if len(sys.argv) > 2 else M4U_PORT_IMGO
    ion = os.open("/dev/ion", os.O_RDWR)
    buf = IonBuffer(ion, size, port)
    print("handle %d mva 0x%08x fd %d" % (buf.handle, buf.mva, buf.fd))
    buf.map[0:4] = b"\x12\x34\x56\x78"
    print("cpu write/read ok:", buf.map[0:4].hex())
    buf.close()
    os.close(ion)
