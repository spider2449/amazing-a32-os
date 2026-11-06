# Experiment: a BLE scan over /dev/stpbt. Prints how many advertising reports came
# from how many addresses, with signal strength, and nothing from the payloads.
import os, select, sys, time

fd = os.open("/dev/stpbt", os.O_RDWR | os.O_NOCTTY)

def send(opcode, params=b""):
    os.write(fd, bytes([0x01, opcode & 0xFF, opcode >> 8, len(params)]) + params)

def read_for(seconds):
    events, end = [], time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], max(0, end - time.time()))
        if not ready:
            break
        events.append(os.read(fd, 4096))
    return events

def status(name, events):
    complete = [e for e in events if len(e) >= 7 and e[1] == 0x0E]
    print(name, "status", complete[-1][6] if complete else "none")

send(0x0C03); status("reset", read_for(1))
send(0x0C01, b"\xff" * 8); status("event_mask", read_for(1))
send(0x2001, b"\x1f" + b"\x00" * 7); status("le_event_mask", read_for(1))
# Passive scan, 30 ms interval and window, public address, accept all.
send(0x200B, bytes([0x00, 0x30, 0x00, 0x30, 0x00, 0x00, 0x00])); status("scan_params", read_for(1))
send(0x200C, bytes([0x01, 0x00])); status("scan_on", read_for(1))

seen, reports = {}, 0
buffer = b"".join(read_for(8))
# Walk H4 events: 0x04, code, length, payload.
at = 0
while at + 3 <= len(buffer):
    if buffer[at] != 0x04:
        at += 1
        continue
    code, length = buffer[at + 1], buffer[at + 2]
    payload = buffer[at + 3:at + 3 + length]
    at += 3 + length
    if code == 0x3E and payload and payload[0] == 0x02:
        count, p = payload[1], 2
        for _ in range(count):
            if p + 9 > len(payload):
                break
            address = ":".join("%02x" % b for b in reversed(payload[p + 2:p + 8]))
            data_length = payload[p + 8]
            rssi = payload[p + 9 + data_length] if p + 9 + data_length < len(payload) else 0
            rssi = rssi - 256 if rssi > 127 else rssi
            reports += 1
            seen.setdefault(address, []).append(rssi)
            p += 10 + data_length

send(0x200C, bytes([0x00, 0x00])); status("scan_off", read_for(1))
print("reports", reports, "addresses", len(seen))
for address, values in sorted(seen.items(), key=lambda item: -max(item[1]))[:8]:
    # Only the vendor half of the address, to keep nearby devices private.
    print(" ", address[:8] + ":xx:xx:xx", "best", max(values), "dBm,", len(values), "reports")
os.close(fd)
