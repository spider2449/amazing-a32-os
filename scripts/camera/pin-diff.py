# Experiment: show which GPIO pins change while a sensor is powered on. Opens the
# sensor with kd_camera_hw (set driver, then T_OPEN, which powers it and keeps it
# on even if its ID read fails), dumps /sys .../mtgpio/pin, then closes it.
# Usage: python3 pin-diff.py SOCKET INDEX   (SOCKET 1 = main, 2 = sub)
import fcntl, os, struct, sys, time

PINS = "/sys/devices/virtual/misc/mtgpio/pin"

def iowr(nr, size):
    return (3 << 30) | (size << 16) | (ord("i") << 8) | nr

def io(nr):
    return (ord("i") << 8) | nr

T_OPEN, T_CLOSE, SET_DRIVER = io(0), io(25), iowr(35, 8)

def pins():
    table = {}
    for line in open(PINS).read().splitlines()[1:]:
        if ":" in line:
            pin, bits = line.split(":", 1)
            table[int(pin)] = bits.strip()
    return table

socket, index = int(sys.argv[1]), int(sys.argv[2])
before = pins()
isp = os.open("/dev/camera-isp", os.O_RDWR)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
fcntl.ioctl(cam, SET_DRIVER, bytearray(struct.pack("<II", (socket << 16) | index, 0)))
try:
    fcntl.ioctl(cam, T_OPEN)
    print("open ok")
except OSError as error:
    print("open:", error)
time.sleep(0.05)
during = pins()
try:
    fcntl.ioctl(cam, T_CLOSE)
except OSError as error:
    print("close:", error)
after = pins()
os.close(cam)
os.close(isp)
print("pin: before -> during -> after   (MODE PULL_SEL DIN DOUT PULL_EN DIR IES SMT)")
for pin in sorted(before):
    if before[pin] != during.get(pin) or before[pin] != after.get(pin):
        print("%3d: %s -> %s -> %s" % (pin, before[pin], during.get(pin), after.get(pin)))
