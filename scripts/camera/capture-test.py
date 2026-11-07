# Experiment: capture one raw frame from the rear sensor through the ISP's IMGO
# DMA into an ION buffer. Sensor and seninf setup come from stream-test.py. The
# CAM register meanings are an assumption, from MediaTek's older frame-based
# ISP layout that these _FrmB libraries were ported from, and are checked here
# against what the hardware does:
#   0x4004 module enable (TG1 bit 0), 0x400C DMA enable (IMGO bit 0),
#   0x4010 format select (TG1 format bits 16-18), 0x4020 interrupt enable,
#   0x4024 interrupt status, 0x4028 DMA interrupt status,
#   IMGO: 0x4300 base, 0x4304 offset, 0x4308 x size-1, 0x430C y size-1, 0x4310 stride.
# Usage: python3 capture-test.py [out.raw]
import ctypes, fcntl, importlib, os, struct, sys, time, zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")
stream = importlib.import_module("stream-test")
ion_mod = importlib.import_module("ion")

LINE_BYTES = stream.WIDTH * 10 // 8          # RAW10 packed
STRIDE = (LINE_BYTES + 15) & ~15
FRAME_BYTES = STRIDE * stream.HEIGHT

def cam_dump(isp, title):
    regs = [0x4000, 0x4004, 0x4008, 0x400C, 0x4010, 0x4018, 0x401C, 0x4020, 0x4024, 0x4028]
    print("== " + title)
    print("   ctl  " + " ".join("%03x:%08x" % (r & 0xFFF, isp.rd(r)) for r in regs))
    print("   imgo " + " ".join("%03x:%08x" % (r & 0xFFF, isp.rd(r)) for r in range(0x4300, 0x4320, 4)))
    print("   tg   " + " ".join("%03x:%08x" % (r & 0xFFF, isp.rd(r)) for r in range(0x4410, 0x4450, 4)))

def buffer_state(buf):
    data = buf.map[:FRAME_BYTES]
    nonzero = sum(1 for i in range(0, FRAME_BYTES, 4096) if data[i] not in (0, 0xA5))
    return "crc %08x, sampled bytes changed %d/%d" % (zlib.crc32(data) & 0xFFFFFFFF, nonzero, FRAME_BYTES // 4096)

def main():
    out = sys.argv[1] if len(sys.argv) > 1 else None
    ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
    cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
    ionfd = os.open("/dev/ion", os.O_RDWR)
    isp = stream.Block(ispfd, stream.ISP_BASE, stream.ISP_SIZE)
    fcntl.ioctl(ispfd, 0x6B00)              # ISP_RESET, as IspDrvImp::reset does at init
    sen = stream.Block(ispfd, stream.ISP_BASE + 0x8000, 0x4000)
    mipi = stream.Block(ispfd, stream.MIPI_BASE, stream.MIPI_SIZE)
    buf = ion_mod.IonBuffer(ionfd, FRAME_BYTES, ion_mod.M4U_PORT_IMGO)
    buf.map[:FRAME_BYTES] = b"\xA5" * FRAME_BYTES
    print("buffer mva 0x%08x, %d bytes, stride %d" % (buf.mva, FRAME_BYTES, STRIDE))
    seninf = probe.Regs(ispfd, probe.SENINF_BASE, 0x4000)
    probe.seninf_init(seninf)
    probe.mclk_on(cam, seninf, 24000, 0)
    fcntl.ioctl(cam, stream.SET_DRIVER, bytearray(struct.pack("<II", (stream.MAIN << 16) | stream.MAIN_INDEX, 0)))
    fcntl.ioctl(cam, stream.SET_CURRENT_SENSOR, bytearray(struct.pack("<I", stream.MAIN)))
    fcntl.ioctl(cam, stream.T_OPEN)
    try:
        cam_dump(isp, "after open")
        stream.set_tg1_grab_range(isp, 0, stream.WIDTH, 0, stream.HEIGHT)
        stream.set_tg1_cfg(isp, stream.TG_FORMAT_RAW)
        stream.set_seninf1_mux_ctrl(sen, 0, 1, stream.SOURCE_MIPI, stream.TG_FORMAT_RAW)
        stream.set_tg1_view_finder_mode(isp, 0)
        stream.set_seninf1_ctrl(sen, stream.PAD_10BIT, stream.SOURCE_MIPI)
        stream.set_seninf1_ncsi2(sen, mipi, stream.SETTLE, stream.LANES, stream.ECC_ORDER, 0x2B)
        stream.set_seninf_top_mux_ctrl(sen, 0, 0)
        window = (ctypes.c_uint8 * 256)()
        config = (ctypes.c_uint8 * 256)()
        fcntl.ioctl(cam, stream.CONTROL, bytearray(struct.pack(
            "<IIII", stream.MAIN, 0, ctypes.addressof(window), ctypes.addressof(config))))
        # IMGO DMA and pass-1 enables.
        isp.wr(0x4300, buf.mva)
        isp.wr(0x4304, 0)
        isp.wr(0x4308, LINE_BYTES - 1)
        isp.wr(0x430C, stream.HEIGHT - 1)
        isp.wr(0x4310, (isp.rd(0x4310) & ~0x3FFF) | STRIDE)
        isp.set(0x4004, 1)        # TG1_EN
        isp.set(0x400C, 1)        # IMGO_EN
        cam_dump(isp, "configured")
        print("   buffer before: " + buffer_state(buf))
        isp.set(0x4414, 1)        # VF_CON: start the timing generator
        for step in range(8):
            time.sleep(0.25)
            cam_dump(isp, "t+%.2f" % (0.25 * (step + 1)))
            print("   buffer: " + buffer_state(buf))
        isp.clr(0x4414, 1)
        if out:
            with open(out, "wb") as f:
                f.write(buf.map[:FRAME_BYTES])
            print("wrote", out)
    finally:
        isp.clr(0x4414, 1)
        isp.clr(0x400C, 1)
        isp.clr(0x4004, 1)
        fcntl.ioctl(cam, stream.T_CLOSE)
        probe.mclk_off(cam, seninf)
        buf.close()
        os.close(ionfd)
        os.close(cam)
        os.close(ispfd)

if __name__ == "__main__":
    main()
