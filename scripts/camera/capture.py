# Captures one raw frame from the rear or front camera into a file. This is the
# working reference for the camera driver:
#   sensor: kd_camera_hw ioctls (set driver, set current sensor, open, preview
#           scenario, exposure features);
#   seninf: the stock HAL's NCSI2 receiver setup, on seninf 1 (rear, CSI0, two
#           lanes) or seninf 2 (front, CSI1, one lane), feeding mux 0 -> TG1;
#   ISP:    pass-1 CAM registers replayed from stock Android's preview
#           (android_cam_regs.py), IMGO into an ION buffer, statistics off.
# Frames are RAW10 packed least significant bit first, Bayer starting with R.
# Usage: python3 capture.py rear|front out.raw [shutter] [gain]
import ctypes, fcntl, importlib, os, struct, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")
stream = importlib.import_module("stream-test")
ion_mod = importlib.import_module("ion")
import android_cam_regs

CAMERAS = {
    # socket, sensor list index, width, height, IMGO stride, seninf, lanes enum, cam table
    "rear": dict(socket=1, index=1, width=2592, height=1944, stride=0xCB2, seninf=1, lanes=1,
                 regs=android_cam_regs.ANDROID_CAM),
    "front": dict(socket=2, index=0, width=1600, height=1200, stride=0x7DA, seninf=2, lanes=0,
                  regs=android_cam_regs.ANDROID_CAM_FRONT),
}
SKIP = {0x4000, 0x400C, 0x4300, 0x4410, 0x4414}
STATS = 0x70000                    # AF, flicker and AE/AWB statistics enables in 0x4004

def seninf_ncsi2(sen, mipi, n, lanes):
    """setSeninfNCtrl + setSeninfNNCSI2 (enable path, NCSI2, RAW10), for seninf n."""
    s = 0x400 * (n - 1)            # seninf register block offset
    a = 0x800 + 0x400 * (n - 1)    # MIPI RX analog block offset (CSI0, CSI1)
    sen.set(s + 0x100, 1)
    sen.field(s + 0x100, 28, 3, 0)                 # PAD_10BIT
    sen.field(s + 0x100, 12, 4, 8)                 # source: MIPI (NCSI2)
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
        sen.set(s + 0x3D8, bit)
    sen.set(s + 0x338, 1)
    sen.wr(s + 0x33C, 0x1541)
    sen.set(s + 0x338, 4)
    time.sleep(500e-6)
    sen.clr(s + 0x338, 1)
    for bit in (0x10, 1, 2, 4, 8):
        sen.clr(s + 0x3D8, bit)
    count = 85 * 300 // 1000
    sen.wr(s + 0x3AC, 0)
    sen.wr(s + 0x100, (sen.rd(s + 0x100) & ~0xF000) | 0x8000)
    sen.clr(s + 0x3A0, 0x80)
    sen.wr(s + 0x3A8, (count << 8) & 0xFFFF)
    sen.set(s + 0x3A0, ((1 << (lanes + 1)) - 1) | (1 << 16) | 0x4000000 | 0x10)
    sen.wr(s + 0x3B0, 0x80007FFF)

def capture(name, out, shutter, gain):
    c = CAMERAS[name]
    frame_bytes = c["stride"] * c["height"]
    ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
    cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
    ionfd = os.open("/dev/ion", os.O_RDWR)
    isp = stream.Block(ispfd, stream.ISP_BASE, stream.ISP_SIZE)
    fcntl.ioctl(ispfd, 0x6B00)                     # ISP_RESET
    sen = stream.Block(ispfd, stream.ISP_BASE + 0x8000, 0x4000)
    mipi = stream.Block(ispfd, stream.MIPI_BASE, stream.MIPI_SIZE)
    buf = ion_mod.IonBuffer(ionfd, frame_bytes, ion_mod.M4U_PORT_IMGO)
    seninf = probe.Regs(ispfd, probe.SENINF_BASE, 0x4000)
    probe.seninf_init(seninf)
    probe.mclk_on(cam, seninf, 24000, 0)
    fcntl.ioctl(cam, stream.SET_DRIVER, bytearray(struct.pack("<II", (c["socket"] << 16) | c["index"], 0)))
    fcntl.ioctl(cam, stream.SET_CURRENT_SENSOR, bytearray(struct.pack("<I", c["socket"])))
    fcntl.ioctl(cam, stream.T_OPEN)
    try:
        stream.set_tg1_grab_range(isp, 0, c["width"], 0, c["height"])
        stream.set_tg1_cfg(isp, 0)
        stream.set_seninf1_mux_ctrl(sen, 0, 0, 8, 0)
        stream.set_tg1_view_finder_mode(isp, 0)
        seninf_ncsi2(sen, mipi, c["seninf"], c["lanes"])
        sen.wr(0x8, (sen.rd(0x8) & 0xFFF0) | (c["seninf"] - 1))   # mux 0 <- seninf n
        window = (ctypes.c_uint8 * 256)()
        config = (ctypes.c_uint8 * 256)()
        fcntl.ioctl(cam, stream.CONTROL, bytearray(struct.pack(
            "<IIII", c["socket"], 0, ctypes.addressof(window), ctypes.addressof(config))))
        stream.feature(cam, c["socket"], 3004, [shutter, 0])
        stream.feature(cam, c["socket"], 3006, [gain, 0])
        for reg, value in c["regs"]:
            if reg not in SKIP:
                isp.wr(reg, value)
        isp.clr(0x4004, STATS)
        isp.wr(0x40F4, 0)
        isp.wr(0x4300, buf.mva)
        isp.wr(0x400C, 1)
        start = isp.rd(0x444C) >> 24
        isp.set(0x4414, 1)
        # Wait for a few frames so the new exposure has taken effect.
        deadline = time.time() + 3
        while time.time() < deadline and ((isp.rd(0x444C) >> 24) - start) & 0xFF < 4:
            time.sleep(0.05)
        isp.clr(0x4414, 1)
        time.sleep(0.15)
        frames = ((isp.rd(0x444C) >> 24) - start) & 0xFF
        with open(out, "wb") as f:
            f.write(buf.map[:frame_bytes])
        print("%s: %dx%d, %d frames, wrote %s" % (name, c["width"], c["height"], frames, out))
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
    capture(sys.argv[1], sys.argv[2],
            int(sys.argv[3]) if len(sys.argv) > 3 else 2900,
            int(sys.argv[4]) if len(sys.argv) > 4 else 512)
