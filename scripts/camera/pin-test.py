# Experiment: the kernel's power sequence turns the camera rails and MCLK on but
# never drives the reset and power-down pins (their pinctrl states change
# nothing). This sets those pins by hand through mtgpio before the kernel's alive
# check, which powers the sensor, reads its ID and powers it off again.
# Usage: python3 pin-test.py SOCKET INDEX [PDN RST]...   (SOCKET 1 main, 2 sub)
# Each PDN RST pair is tried in turn; the default tries all four.
import fcntl, importlib, os, struct, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")

MTGPIO = "/sys/devices/virtual/misc/mtgpio/pin"
RST_PIN = 44                     # shared by both cameras (device tree cam0/cam1_rst)
PDN_PIN = {1: 7, 2: 12}          # cam0_pnd, cam1_pnd

def gpio_out(pin, value):
    with open(MTGPIO, "w") as f:
        f.write("-wdout %d %d" % (pin, value))

socket, index = int(sys.argv[1]), int(sys.argv[2])
pairs = [int(v) for v in sys.argv[3:]] or [0, 0, 1, 0, 0, 1, 1, 1]
isp = os.open("/dev/camera-isp", os.O_RDWR)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
seninf = probe.Regs(isp, probe.SENINF_BASE, 0x4000)
probe.mclk_on(cam, seninf, 24000, 0)
try:
    for pdn, rst in zip(pairs[0::2], pairs[1::2]):
        gpio_out(PDN_PIN[socket], pdn)
        gpio_out(RST_PIN, rst)
        time.sleep(0.01)
        name = {1: "main", 2: "sub"}[socket]
        print("pdn %d rst %d: %s" % (pdn, rst, probe.probe(cam, name, index)))
        sys.stdout.flush()
finally:
    gpio_out(PDN_PIN[socket], 0)
    gpio_out(RST_PIN, 0)
    probe.mclk_off(cam, seninf)
    os.close(cam)
    os.close(isp)
