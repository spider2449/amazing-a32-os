# Experiment: with the rear sensor streaming over the CSI2 receiver, try the
# seninf mux sync polarities (MUX_CTRL bit 10 hsync, bit 9 vsync) and report the
# mux status, mux size and TG counters for each, with the TG running.
# Usage: A32_MIPI_TYPE=1 python3 mux-sweep.py
import ctypes, fcntl, importlib, os, struct, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")
stream = importlib.import_module("stream-test")

ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
isp = stream.Block(ispfd, stream.ISP_BASE, stream.ISP_SIZE)
fcntl.ioctl(ispfd, 0x6B00)                  # ISP_RESET, as IspDrvImp::reset does at init
sen = stream.Block(ispfd, stream.ISP_BASE + 0x8000, 0x4000)
mipi = stream.Block(ispfd, stream.MIPI_BASE, stream.MIPI_SIZE)
seninf = probe.Regs(ispfd, probe.SENINF_BASE, 0x4000)
probe.seninf_init(seninf)
probe.mclk_on(cam, seninf, 24000, 0)
fcntl.ioctl(cam, stream.SET_DRIVER, bytearray(struct.pack("<II", (stream.MAIN << 16) | stream.MAIN_INDEX, 0)))
fcntl.ioctl(cam, stream.SET_CURRENT_SENSOR, bytearray(struct.pack("<I", stream.MAIN)))
fcntl.ioctl(cam, stream.T_OPEN)
try:
    stream.set_tg1_grab_range(isp, 0, stream.WIDTH, 0, stream.HEIGHT)
    stream.set_tg1_cfg(isp, stream.TG_FORMAT_RAW)
    stream.set_seninf1_mux_ctrl(sen, 0, 0, stream.SOURCE_MIPI, stream.TG_FORMAT_RAW)
    stream.set_tg1_view_finder_mode(isp, 0)
    stream.set_seninf1_ctrl(sen, stream.PAD_10BIT, stream.SOURCE_MIPI)
    stream.set_seninf1_ncsi2(sen, mipi, stream.SETTLE, stream.LANES, stream.ECC_ORDER, 0x2B)
    stream.set_seninf_top_mux_ctrl(sen, 0, 0)
    window = (ctypes.c_uint8 * 256)()
    config = (ctypes.c_uint8 * 256)()
    fcntl.ioctl(cam, stream.CONTROL, bytearray(struct.pack(
        "<IIII", stream.MAIN, 0, ctypes.addressof(window), ctypes.addressof(config))))
    def isp_irqs():
        for line in open("/proc/interrupts"):
            if line.rstrip().endswith(" ISP"):
                return int(line.split()[1])
        return -1
    sen.field(0x120, 9, 1, 1)                  # vsync polarity that clears the mux errors
    print("0x4160 before %08x 0x4164 %08x" % (isp.rd(0x4160), isp.rd(0x4164)))
    isp.wr(0x4160, isp.rd(0x4160) | 0x6000)    # ISP_TOP_CTRL::_enable writes 0x6000 before pass 1
    print("0x4160 after  %08x" % isp.rd(0x4160))
    isp.set(0x4004, 1)
    isp.wr(0x4020, 0x7FFFFFFF)                 # every pass-1 interrupt, so the kernel counts them
    before = isp_irqs()
    isp.set(0x4414, 1)
    time.sleep(1.0)
    print("ISP interrupts in 1 s with TG running: %d" % (isp_irqs() - before))
    isp.wr(0x4020, 0)
    print("seninf top " + " ".join("%03x:%08x" % (o, sen.rd(o)) for o in range(0x0, 0x20, 4)))
    print("seninf1    " + " ".join("%03x:%08x" % (o, sen.rd(o)) for o in range(0x100, 0x140, 4)))
    for hs, vs in ((0, 0), (1, 0), (0, 1), (1, 1)):
        sen.field(0x120, 10, 1, hs)
        sen.field(0x120, 9, 1, vs)
        v = sen.rd(0x120)
        sen.wr(0x120, v | 3)
        sen.wr(0x120, v & ~3)
        sen.wr(0x128, 0xFFFFFFFF)
        time.sleep(0.3)
        print("hs %d vs %d: MUX_CTRL %08x INTSTA %08x SIZE %08x | tg 430-448: %s" % (
            hs, vs, sen.rd(0x120), sen.rd(0x128), sen.rd(0x12C),
            " ".join("%08x" % isp.rd(o) for o in range(0x4430, 0x444C, 4))))
finally:
    isp.clr(0x4414, 1)
    isp.clr(0x4004, 1)
    fcntl.ioctl(cam, stream.T_CLOSE)
    probe.mclk_off(cam, seninf)
