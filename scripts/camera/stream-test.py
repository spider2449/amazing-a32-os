# Experiment: start the rear sensor (GC5025) streaming over MIPI CSI-2 and check
# that frames reach the sensor interface (seninf) and the ISP timing generator.
# The register sequence copies the stock HAL (libcam.halsensor.so):
# HalSensor::seninfControl for the main sensor, MIPI source, NCSI2 receiver, and
# the SeninfDrvImp setters it calls. No image is captured yet.
# Usage: python3 stream-test.py [seconds]
import ctypes, fcntl, importlib, mmap, os, struct, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")

def iowr(nr, size):
    return (3 << 30) | (size << 16) | (ord("i") << 8) | nr

def io(nr):
    return (ord("i") << 8) | nr

T_OPEN, T_CLOSE = io(0), io(25)
CONTROL = iowr(20, 16)           # { u32 camera; u32 scenario; void *window; void *config }
SET_DRIVER = iowr(35, 8)
SET_CURRENT_SENSOR = iowr(70, 4)  # u32 sensor device; ImgSensorDrv::open sends it before T_OPEN

MAIN, MAIN_INDEX = 1, 1          # GC5025 on the main socket
WIDTH, HEIGHT = 2592, 1944       # GC5025 get_resolution, every scenario
LANES = 1                        # enum: 0 = 1 lane, 1 = 2 lanes
ECC_ORDER = 1                    # SensorPacketECCOrder
SETTLE = 85                      # mipi_data_lp2hs_settle_dc from get_info
# Receiver: 0 = NCSI2 (seninf source 8), 1 = CSI2 (seninf source 0). The HAL
# picks by a sensor config field; set A32_MIPI_TYPE to choose.
MIPI_TYPE = int(os.environ.get("A32_MIPI_TYPE", "1"))
SOURCE_MIPI = 0 if MIPI_TYPE == 1 else 8
PAD_10BIT = 0
TG_FORMAT_RAW = 0

ISP_BASE, ISP_SIZE = 0x15000000, 0x10000
MIPI_BASE, MIPI_SIZE = 0x10215000, 0x3000
ANA = 0x800                      # MIPI RX analog block inside MIPI_BASE

class Block:
    def __init__(self, fd, base, size):
        self.map = mmap.mmap(fd, size, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE, offset=base)
        self.words = memoryview(self.map).cast("I")

    def rd(self, off):
        return self.words[off // 4]

    def wr(self, off, val):
        self.words[off // 4] = val & 0xFFFFFFFF

    def set(self, off, bits):
        self.wr(off, self.rd(off) | bits)

    def clr(self, off, bits):
        self.wr(off, self.rd(off) & ~bits)

    def field(self, off, shift, width, val):
        mask = ((1 << width) - 1) << shift
        self.wr(off, (self.rd(off) & ~mask) | ((val << shift) & mask))

# --- SeninfDrvImp setters (seninf = ISP + 0x8000, TG in CAM = ISP + 0x4000) ---

def set_tg1_grab_range(isp, px_start, px_end, ln_start, ln_end):
    isp.field(0x4418, 16, 15, px_end)
    isp.field(0x4418, 0, 15, px_start)
    isp.field(0x441C, 16, 13, ln_end)
    isp.field(0x441C, 0, 13, ln_start)

def set_tg1_cfg(isp, fmt):
    isp.clr(0x4420, 3)
    isp.field(0x4010, 16, 3, fmt)

def set_tg1_view_finder_mode(isp, single):
    isp.set(0x4410, 1)
    isp.set(0x4410, 4)
    isp.set(0x4414, 0x1000)
    isp.field(0x4414, 1, 1, single)

def set_seninf1_mux_ctrl(sen, hs_pol, vs_pol, src, fmt):
    r = 0x120
    sen.set(r, 1 << 31)
    sen.field(r, 12, 4, src)
    sen.clr(r, 0x100)
    if fmt == 7:
        sen.clr(r, 0x30000000)
    else:
        sen.wr(r, (sen.rd(r) & ~0x30000000) | 0x10000000)
    v = sen.rd(r) & ~0x0FC00000
    if src != 0 and src < 8:
        if fmt == 7:
            sen.wr(r, v | 0x06000000); push = 30
        else:
            sen.wr(r, v | 0x06C00000); push = 31
        sen.field(r, 16, 6, push)
    else:
        if fmt == 7:
            sen.wr(r, v | 0x06000000); sen.field(r, 16, 6, 30)
        else:
            sen.wr(r, v | 0x0EC00000); sen.set(r, 0x3F0000)
    sen.field(r, 10, 1, hs_pol)
    sen.field(r, 9, 1, vs_pol)

def set_seninf1_ctrl(sen, pad, src):
    sen.set(0x100, 1)
    sen.field(0x100, 28, 3, pad)
    sen.field(0x100, 12, 4, src)

def set_seninf_top_mux_ctrl(sen, index, value):
    v = sen.rd(0x8)
    if index == 0:
        v = (v & 0xFFF0) | (value & 0xF)
    sen.wr(0x8, v)

def set_seninf1_ncsi2(sen, mipi, settle, lanes, ecc, data_type):
    a = ANA
    # Analog calibration (enable path, MIPI type NCSI2).
    for off in (76, 80):
        mipi.wr(a + off, mipi.rd(a + off) & 0xFEFBEFBE)
    for off in (0, 4, 8, 12, 16):
        mipi.set(a + off, 8)
    mipi.set(a + 36, 1)
    time.sleep(30e-6)
    mipi.set(a + 32, 3)
    time.sleep(1e-6)
    for off in (0, 4, 8, 12, 16):
        mipi.set(a + off, 1)
    for bit in (0x10, 1, 2, 4, 8):
        sen.set(0x3D8, bit)
    sen.set(0x338, 1)
    sen.wr(0x33C, 0x1541)
    sen.set(0x338, 4)
    time.sleep(500e-6)
    cal = (sen.rd(0x344), sen.rd(0x348))
    sen.clr(0x338, 1)
    for bit in (0x10, 1, 2, 4, 8):
        sen.clr(0x3D8, bit)
    count = (settle or 85) * 300 // 1000
    sen.wr(0x3AC, {42: 0xF000, 48: 0x80, 49: 0x100}.get(data_type, 0))
    lane_mask = (1 << (lanes + 1)) - 1
    if MIPI_TYPE == 1:
        # CSI2 receiver (MIPI_OPHY_CSI2): seninf source 0, CSI2 registers.
        sen.clr(0x100, 0xF000)
        sen.wr(0x364, (count << 16) & 0x00FF00FF)
        sen.set(0x360, lane_mask | 0x30)
        sen.set(0x368, 0xF)
    else:
        # NCSI2 receiver (MIPI_OPHY_NCSI2): seninf source 8.
        sen.wr(0x100, (sen.rd(0x100) & ~0xF000) | 0x8000)
        sen.clr(0x3A0, 0x80)
        sen.wr(0x3A8, (count << 8) & 0xFFFF)
        sen.set(0x3A0, lane_mask | (ecc << 16) | 0x4000000 | 0x10)
        sen.wr(0x3B0, 0x80007FFF)
    v = sen.rd(0x120)
    sen.wr(0x120, v | 3)
    sen.wr(0x120, v & ~3)
    return cal

FEATURE_CONTROL = iowr(15, 16)   # { u32 camera; u32 feature; u8 *para; u32 *len }
# Read from the GC5025 kernel driver's feature_control jump table: 3009 is not
# handled, 3010 writes {reg, data} and 3011 reads para[1] = reg(para[0]).
SET_REGISTER, GET_REGISTER = 3010, 3011

def feature(cam, camera, feature_id, para):
    buf = (ctypes.c_uint32 * len(para))(*para)
    length = ctypes.c_uint32(4 * len(para))
    arg = bytearray(struct.pack("<IIII", camera, feature_id, ctypes.addressof(buf), ctypes.addressof(length)))
    fcntl.ioctl(cam, FEATURE_CONTROL, arg)
    return list(buf)

def sensor_read(cam, reg):
    return feature(cam, MAIN, GET_REGISTER, [reg, 0])[1]

def sensor_write(cam, reg, value):
    feature(cam, MAIN, SET_REGISTER, [reg, value])

def sensor_state(cam, title):
    # GC5025: 0xf0/0xf1 chip ID, 0xfe page select, page 3 0x10 MIPI/stream control.
    chip = (sensor_read(cam, 0xF0), sensor_read(cam, 0xF1))
    page = sensor_read(cam, 0xFE)
    regs = [sensor_read(cam, r) for r in range(0x00, 0x20)]
    print("   sensor %s: id %02x%02x page %02x, regs 00-1f: %s" % (
        title, chip[0], chip[1], page, " ".join("%02x" % v for v in regs)))

def dump(isp, sen, title):
    print("== " + title)
    print("   seninf  CTRL %08x MUX_CTRL %08x MUX_INTSTA %08x MUX_SIZE %08x TOP_MUX %08x" % (
        sen.rd(0x100), sen.rd(0x120), sen.rd(0x128), sen.rd(0x12C), sen.rd(0x8)))
    print("   ncsi2   CTL %08x LNRD %08x INT_EN %08x INT_STA %08x DBG %08x %08x %08x %08x" % (
        sen.rd(0x3A0), sen.rd(0x3A8), sen.rd(0x3B0), sen.rd(0x3B4),
        sen.rd(0x3B8), sen.rd(0x3BC), sen.rd(0x3C0), sen.rd(0x3C4)))
    print("   csi2    " + " ".join("%03x:%08x" % (o, sen.rd(o)) for o in range(0x360, 0x3A0, 4)))
    print("   tg      " + " ".join("%03x:%08x" % (o, isp.rd(o)) for o in range(0x4410, 0x4450, 4)))

def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 1.0
    ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
    cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
    isp = Block(ispfd, ISP_BASE, ISP_SIZE)
    sen = Block(ispfd, ISP_BASE + 0x8000, 0x4000)
    mipi = Block(ispfd, MIPI_BASE, MIPI_SIZE)
    seninf = probe.Regs(ispfd, probe.SENINF_BASE, 0x4000)
    probe.seninf_init(seninf)
    probe.mclk_on(cam, seninf, 24000, 0)
    fcntl.ioctl(cam, SET_DRIVER, bytearray(struct.pack("<II", (MAIN << 16) | MAIN_INDEX, 0)))
    fcntl.ioctl(cam, SET_CURRENT_SENSOR, bytearray(struct.pack("<I", MAIN)))
    start = time.time()
    fcntl.ioctl(cam, T_OPEN)
    print("sensor open in %.3f s" % (time.time() - start))
    try:
        sensor_state(cam, "after open")
        dump(isp, sen, "after open")
        # HalSensor::seninfControl, main sensor, MIPI source, NCSI2.
        set_tg1_grab_range(isp, 0, WIDTH, 0, HEIGHT)
        set_tg1_cfg(isp, TG_FORMAT_RAW)
        set_seninf1_mux_ctrl(sen, 0, 1, SOURCE_MIPI, TG_FORMAT_RAW)
        set_tg1_view_finder_mode(isp, 0)
        set_seninf1_ctrl(sen, PAD_10BIT, SOURCE_MIPI)
        cal = set_seninf1_ncsi2(sen, mipi, SETTLE, LANES, ECC_ORDER, 0x2B)
        set_seninf_top_mux_ctrl(sen, 0, 0)
        print("calibration status %08x %08x" % cal)
        # Sensor: preview scenario starts streaming.
        window = (ctypes.c_uint8 * 256)()
        config = (ctypes.c_uint8 * 256)()
        arg = bytearray(struct.pack("<IIII", MAIN, 0, ctypes.addressof(window), ctypes.addressof(config)))
        fcntl.ioctl(cam, CONTROL, arg)
        print("preview scenario set")
        sensor_state(cam, "after preview")
        dump(isp, sen, "configured")
        # TG data enable (VF_CON bit 0), normally set when the ISP pass starts.
        isp.set(0x4414, 1)
        end = time.time() + seconds
        while time.time() < end:
            # Status bits are sticky: clear them, then see what comes back.
            sen.wr(0x3B4, 0xFFFFFFFF)
            sen.wr(0x128, 0xFFFFFFFF)
            time.sleep(min(0.25, seconds))
            dump(isp, sen, "t+%.2f" % (seconds - (end - time.time())))
        isp.clr(0x4414, 1)
    finally:
        fcntl.ioctl(cam, T_CLOSE)
        probe.mclk_off(cam, seninf)
        os.close(cam)
        os.close(ispfd)

if __name__ == "__main__":
    main()
