# Experiment: list which kd_camera_hw commands the driver accepts. Every command
# is sent with a zeroed argument, which selects no sensor. Only the answers that
# are not EPERM ("No such command") are printed.
# Usage: python3 ioctl-sweep.py DEVICE NR_FROM NR_TO
import fcntl, os, sys

device = sys.argv[1]
first, last = int(sys.argv[2]), int(sys.argv[3])
fd = os.open(device, os.O_RDWR)
for nr in range(first, last + 1):
    for direction in (0, 1, 2, 3):
        sizes = [0] if direction == 0 else range(4, 129, 4)
        for size in sizes:
            cmd = (direction << 30) | (size << 16) | (ord("i") << 8) | nr
            buf = bytearray(max(size, 4))
            try:
                if direction == 0:
                    result = fcntl.ioctl(fd, cmd)
                else:
                    result = fcntl.ioctl(fd, cmd, buf)
                print("nr", nr, "dir", direction, "size", size, "->", result, buf[:16].hex())
            except OSError as error:
                if error.errno != 1:
                    print("nr", nr, "dir", direction, "size", size, "->", error)
            sys.stdout.flush()
os.close(fd)
