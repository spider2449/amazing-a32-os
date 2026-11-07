# Experiment: run the alive check for each socket and sensor index on its own,
# printing the kernel's sensor driver lines for that attempt next to the ioctl
# result, because the two did not agree when run back to back.
# Usage: python3 probe-each.py [max_index]
import importlib, os, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")

def kernel_lines():
    # dmesg -c clears the buffer, so each call returns only the new lines.
    out = subprocess.run(["dmesg", "-c"], capture_output=True, text=True).stdout
    keep = ("sensor", "Sensor", "kd_", "camera")
    return [l.split("]", 2)[-1].strip() for l in out.splitlines() if any(k in l for k in keep)]

max_index = int(sys.argv[1]) if len(sys.argv) > 1 else 2
isp = os.open("/dev/camera-isp", os.O_RDWR)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
seninf = probe.Regs(isp, probe.SENINF_BASE, 0x4000)
probe.seninf_init(seninf)
probe.mclk_on(cam, seninf, 24000, 0)
kernel_lines()
try:
    for socket in probe.SOCKETS:
        for index in range(max_index + 1):
            result = probe.probe(cam, socket, index)
            print("%s %d: %s" % (socket, index, result))
            for line in kernel_lines():
                print("    " + line)
            sys.stdout.flush()
finally:
    probe.mclk_off(cam, seninf)
    os.close(cam)
    os.close(isp)
