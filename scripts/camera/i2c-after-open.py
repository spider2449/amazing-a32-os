# Experiment: register reads through the GC5025 driver time out after T_OPEN.
# This opens the sensor and times single reads (feature 3011) of the chip ID at
# each step: before open (via the alive check), right after open, and after a
# short wait, to find when the I2C bus stops answering.
# Usage: python3 i2c-after-open.py
import fcntl, importlib, os, struct, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")
stream = importlib.import_module("stream-test")

def timed_read(cam, reg):
    start = time.time()
    try:
        value = "%02x" % stream.sensor_read(cam, reg)
    except OSError as error:
        value = str(error)
    return "%s in %.3f s" % (value, time.time() - start)

ispfd = os.open("/dev/camera-isp", os.O_RDWR | os.O_SYNC)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
seninf = probe.Regs(ispfd, probe.SENINF_BASE, 0x4000)
probe.seninf_init(seninf)
probe.mclk_on(cam, seninf, 24000, 0)
print("alive check:", probe.probe(cam, "main", stream.MAIN_INDEX))
fcntl.ioctl(cam, stream.SET_DRIVER, bytearray(struct.pack("<II", (stream.MAIN << 16) | stream.MAIN_INDEX, 0)))
start = time.time()
fcntl.ioctl(cam, stream.T_OPEN)
print("open took %.3f s" % (time.time() - start))
print("PH_CNT %08x %08x" % (seninf.read(0x200), seninf.read(0x600)))
for label in ("right after open", "after 0.5 s"):
    print(label, "id f0:", timed_read(cam, 0xF0), " f1:", timed_read(cam, 0xF1))
    time.sleep(0.5)
fcntl.ioctl(cam, stream.T_CLOSE)
probe.mclk_off(cam, seninf)
