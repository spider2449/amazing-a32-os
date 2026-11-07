# Experiment: find the argument size the kd_camera_hw driver expects for a
# command number. The driver answers EPERM ("No such command") for any command
# whose encoded size does not match, so the size that gives another answer wins.
# Usage: python3 ioctl-size.py NR [DIR]   (DIR 3 = read/write, the default)
import fcntl, os, sys

nr = int(sys.argv[1])
direction = int(sys.argv[2]) if len(sys.argv) > 2 else 3
fd = os.open("/dev/kd_camera_hw", os.O_RDWR)
for size in range(0, 129, 4):
    cmd = (direction << 30) | (size << 16) | (ord("i") << 8) | nr
    buf = bytearray(max(size, 4))  # all zero: no socket selected
    try:
        fcntl.ioctl(fd, cmd, buf)
        print(size, "ok")
    except OSError as error:
        if error.errno != 1:
            print(size, error)
os.close(fd)
