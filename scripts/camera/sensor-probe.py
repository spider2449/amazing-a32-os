# Experiment: find which image sensor answers on each camera socket, using
# MediaTek's kd_camera_hw driver. The sensor clock (MCLK) is turned on first the
# way the stock HAL does it (libcam.halsensor.so, SeninfDrvImp::setMclk1): a PLL
# ioctl, then the seninf TG1 clock registers. Then for each sensor list index and
# socket the driver is set and the kernel powers the sensor on and reads its ID.
# Usage: python3 sensor-probe.py [max_index] [mclk_khz] [polarity]
import fcntl, mmap, os, struct, sys, time

def iowr(nr, size):
    # _IOWR('i', nr, size) on 32-bit ARM.
    return (3 << 30) | (size << 16) | (ord("i") << 8) | nr

def io(nr):
    return (ord("i") << 8) | nr

# Command numbers read from this kernel: they go up in steps of 5.
CHECK_IS_ALIVE = io(30)
SET_DRIVER = iowr(35, 8)      # u32 SensorIndex[2]
SET_MCLK_PLL = iowr(60, 8)    # { u8 on; u32 freq_source }  (0xC008693C)

SENINF_BASE = 0x15008000
TOP_CTRL = 0x000
# PH_CNT and SEN_CK of TG1 (MCLK1) and TG2 (MCLK2). The HAL turns both on
# before searching (HalSensorList::enumerateSensor_Locked).
TG_REGS = ((0x200, 0x204), (0x600, 0x604))

DUAL_SHIFT = 16
SOCKETS = {"main": 1, "sub": 2}

class Regs:
    def __init__(self, fd, base, size):
        self.map = mmap.mmap(fd, size, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE, offset=base)
        self.words = memoryview(self.map).cast("I")  # 32-bit accesses

    def read(self, offset):
        return self.words[offset // 4]

    def write(self, offset, value):
        self.words[offset // 4] = value & 0xFFFFFFFF

def mclk_setting(khz):
    # Same arithmetic as HalSensor::setTgPhase: source 1 is 48 MHz, 2 is 208 MHz.
    if 48000 % khz == 0:
        source, base = 1, 48000
    else:
        source, base = 2, 208000
    count = min((base + khz // 2) // khz - 1, 15)
    return source, count

def seninf_init(seninf):
    # The register writes at the end of SeninfDrvImp::init.
    for offset in (0x120, 0x520, 0x920, 0xD20):
        seninf.write(offset, seninf.read(offset) | (1 << 31))
    for offset in (0x100, 0x500, 0x900):
        seninf.write(offset, seninf.read(offset) | 1)

def mclk_on(cam, seninf, khz, polarity):
    # SeninfDrvImp::setMclk1/2 with the arguments enumerateSensor_Locked passes.
    source, count = mclk_setting(khz)
    fcntl.ioctl(cam, SET_MCLK_PLL, bytearray(struct.pack("<BxxxI", 1, source)))
    for ph_reg, ck_reg in TG_REGS:
        seninf.write(ph_reg, seninf.read(ph_reg) | (1 << 31))             # PCEN
        seninf.write(TOP_CTRL, seninf.read(TOP_CTRL) | 0xC00)
        falling = (count + 1) >> 1 if count >= 2 else 1
        ck = seninf.read(ck_reg) & ~0x3F3F3F
        seninf.write(ck_reg, ck | falling | (count << 16))                 # CLKFL, CLKCNT
        ph = seninf.read(ph_reg)
        ph = (ph & ~(1 << 2)) | ((0 if count & 1 else 1) << 2)            # CLKFL_POL
        ph = (ph & ~(1 << 28)) | (polarity << 28)
        ph = (ph & ~3) | 1                                                 # TGCLK_SEL
        ph &= ~(1 << 6)                                                    # pad PCLK not inverted
        seninf.write(ph_reg, ph)
        print("mclk %d kHz: source %d count %d, PH_CNT %08x SEN_CK %08x" % (
            khz, source, count, seninf.read(ph_reg), seninf.read(ck_reg)))
    time.sleep(0.001)

def mclk_off(cam, seninf):
    for ph_reg, _ in TG_REGS:
        seninf.write(ph_reg, seninf.read(ph_reg) & ~(1 << 31))
    fcntl.ioctl(cam, SET_MCLK_PLL, bytearray(struct.pack("<BxxxI", 0, 1)))

def probe(fd, socket, index):
    buf = bytearray(struct.pack("<II", (SOCKETS[socket] << DUAL_SHIFT) | index, 0))
    try:
        fcntl.ioctl(fd, SET_DRIVER, buf)
    except OSError as error:
        return "set_driver failed: %s" % error
    try:
        fcntl.ioctl(fd, CHECK_IS_ALIVE)
    except OSError as error:
        return "not alive: %s" % error
    return "ALIVE"

def main():
    max_index = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    khz = int(sys.argv[2]) if len(sys.argv) > 2 else 24000
    isp = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
    cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
    seninf = Regs(isp, SENINF_BASE, 0x4000)
    polarity = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    seninf_init(seninf)
    mclk_on(cam, seninf, khz, polarity)
    try:
        for socket in SOCKETS:
            for index in range(max_index + 1):
                print(socket, index, probe(cam, socket, index))
                sys.stdout.flush()
    finally:
        mclk_off(cam, seninf)
    os.close(cam)
    os.close(isp)

if __name__ == "__main__":
    main()
