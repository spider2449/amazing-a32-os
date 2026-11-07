# Experiment: show MediaTek clock manager state (/proc/clkmgr) for the camera
# blocks at idle, with the ISP open and MCLK set up, and while a sensor is being
# powered by the kernel's alive check (sampled from a second thread).
# Usage: python3 clk-watch.py
import importlib, os, re, sys, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
probe = importlib.import_module("sensor-probe")

PATTERN = re.compile(r"cam|img|sen|tg|isp|seninf|larb2|smi|mm_|disp0_smi", re.I)

def snapshot():
    lines = []
    for name in ("clk_stat", "mux_stat", "subsys_stat", "pll_stat"):
        try:
            for line in open("/proc/clkmgr/" + name):
                if PATTERN.search(line):
                    lines.append("%s: %s" % (name, " ".join(line.split())))
        except OSError as error:
            lines.append("%s: %s" % (name, error))
    return lines

def show(title, lines):
    print("== " + title)
    for line in lines:
        print("   " + line)

idle = snapshot()
show("idle", idle)
isp = os.open("/dev/camera-isp", os.O_RDWR)
cam = os.open("/dev/kd_camera_hw", os.O_RDWR)
seninf = probe.Regs(isp, probe.SENINF_BASE, 0x4000)
probe.mclk_on(cam, seninf, 24000, 0)
opened = snapshot()
show("isp open + mclk (changes)", [l for l in opened if l not in idle])

samples, stop = [], threading.Event()
def sampler():
    while not stop.is_set():
        samples.append(snapshot())
thread = threading.Thread(target=sampler)
thread.start()
print("main 0:", probe.probe(cam, "main", 0))
stop.set()
thread.join()
seen = []
for sample in samples:
    for line in sample:
        if line not in opened and line not in seen:
            seen.append(line)
show("during alive check (%d samples, changes)" % len(samples), seen)
probe.mclk_off(cam, seninf)
os.close(cam)
os.close(isp)
