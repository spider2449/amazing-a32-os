# Experiment: capture one raw frame from the rear sensor by replaying the CAM
# (ISP pass-1) register values stock Android used for its preview, captured
# with cam-dump.py on a RAM-booted test image (android_cam_regs.py). Changes:
# IMGO writes to our own ION buffer, only the IMGO DMA is enabled (Android's
# other DMAs point at its own buffers), frame buffer control stays off, and
# the timing generator is started last.
# Usage: A32_MIPI_TYPE=0 python3 capture-android.py [out.raw]
import ctypes, fcntl, importlib, os, struct, sys, time, zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")
stream = importlib.import_module("stream-test")
ion_mod = importlib.import_module("ion")
from android_cam_regs import ANDROID_CAM

STRIDE = 0xCB2                      # what Android used: 3240 data bytes + 10
FRAME_BYTES = STRIDE * stream.HEIGHT
SKIP = {0x4000, 0x400C, 0x4300, 0x4410, 0x4414}

def buffer_state(buf):
    data = buf.map[:FRAME_BYTES]
    changed = sum(1 for i in range(0, FRAME_BYTES, 4096) if data[i] != 0xA5)
    return "crc %08x, sampled bytes changed %d/%d" % (zlib.crc32(data) & 0xFFFFFFFF, changed, FRAME_BYTES // 4096)

def main():
    out = sys.argv[1] if len(sys.argv) > 1 else None
    ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
    cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
    ionfd = os.open("/dev/ion", os.O_RDWR)
    isp = stream.Block(ispfd, stream.ISP_BASE, stream.ISP_SIZE)
    fcntl.ioctl(ispfd, 0x6B00)                       # ISP_RESET
    sen = stream.Block(ispfd, stream.ISP_BASE + 0x8000, 0x4000)
    mipi = stream.Block(ispfd, stream.MIPI_BASE, stream.MIPI_SIZE)
    buf = ion_mod.IonBuffer(ionfd, FRAME_BYTES, ion_mod.M4U_PORT_IMGO)
    buf.map[:FRAME_BYTES] = b"\xA5" * FRAME_BYTES
    print("buffer mva 0x%08x, %d bytes" % (buf.mva, FRAME_BYTES))
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
        # Manual exposure: shutter in lines (frame is 2992 lines), gain with 64 = 1x.
        # The driver reads both as 64-bit values.
        shutter = int(os.environ.get("A32_SHUTTER", "2900"))
        gain = int(os.environ.get("A32_GAIN", "256"))
        stream.feature(cam, stream.MAIN, 3004, [shutter, 0])
        stream.feature(cam, stream.MAIN, 3006, [gain, 0])
        print("shutter %d lines, gain %.2fx" % (shutter, gain / 64))
        for reg, value in ANDROID_CAM:
            if reg not in SKIP:
                isp.wr(reg, value)
        # Statistics modules write through DMAs left off here, so they can stall
        # the pass; A32_EN1_CLEAR / A32_EN2_CLEAR clear enable bits for testing.
        isp.clr(0x4004, int(os.environ.get("A32_EN1_CLEAR", "0x70000"), 0))
        isp.clr(0x4008, int(os.environ.get("A32_EN2_CLEAR", "0"), 0))
        print("EN1 %08x EN2 %08x SEL %08x FMT %08x" % (isp.rd(0x4004), isp.rd(0x4008), isp.rd(0x4018), isp.rd(0x4010)))
        isp.wr(0x40F4, 0)                            # IMGO frame buffer control off
        isp.wr(0x4300, buf.mva)
        isp.wr(0x400C, 0x1)                          # IMGO DMA only
        print("buffer before: " + buffer_state(buf))
        isp.set(0x4414, 1)                           # start the timing generator
        for step in range(8):
            time.sleep(0.25)
            print("t+%.2f tg 444c %08x 4448 %08x int %08x dma %08x | %s" % (
                0.25 * (step + 1), isp.rd(0x444C), isp.rd(0x4448), isp.rd(0x4024), isp.rd(0x4028),
                buffer_state(buf)))
        isp.clr(0x4414, 1)
        if out:
            with open(out, "wb") as f:
                f.write(buf.map[:FRAME_BYTES])
            print("wrote", out)
    finally:
        isp.clr(0x4414, 1)
        isp.wr(0x400C, 0)
        isp.wr(0x4004, 0)
        fcntl.ioctl(cam, stream.T_CLOSE)
        probe.mclk_off(cam, seninf)
        buf.close()
        os.close(ionfd)
        os.close(cam)
        os.close(ispfd)

if __name__ == "__main__":
    main()
