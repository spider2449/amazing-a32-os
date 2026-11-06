# Experiment: talk HCI to MediaTek's /dev/stpbt. Each command is an H4 packet
# (0x01, opcode little-endian, length, parameters); replies are H4 events (0x04).
import os, select, sys, time

fd = os.open("/dev/stpbt", os.O_RDWR | os.O_NOCTTY)

def read_events(seconds):
    events, end = [], time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], max(0, end - time.time()))
        if not ready:
            break
        events.append(os.read(fd, 1024))
    return events

def command(name, opcode, params=b"", wait=2.0):
    packet = bytes([0x01, opcode & 0xFF, opcode >> 8, len(params)]) + params
    os.write(fd, packet)
    events = read_events(wait)
    print(name, "->", [event.hex() for event in events] or "no reply")
    sys.stdout.flush()
    return events

command("reset", 0x0C03)
command("read_local_version", 0x1001)
events = command("read_bd_addr", 0x1009)
for event in events:
    if len(event) >= 13 and event[0] == 0x04 and event[1] == 0x0E:
        print("bd_addr", ":".join("%02x" % b for b in reversed(event[7:13])))
os.close(fd)
