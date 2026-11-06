# Experiment: identify Xiaomi MiBeacon devices (service data 0xFE95) nearby. Prints
# each one's product ID, frame control and whether its payload is encrypted, which
# are header fields, and nothing of the payload or the address.
import os, select, time

fd = os.open("/dev/stpbt", os.O_RDWR | os.O_NOCTTY)

def send(opcode, params=b""):
    os.write(fd, bytes([0x01, opcode & 0xFF, opcode >> 8, len(params)]) + params)
    end = time.time() + 1
    while time.time() < end and select.select([fd], [], [], 0.2)[0]:
        os.read(fd, 1024)

send(0x0C03)
send(0x0C01, b"\xff" * 8)
send(0x2001, b"\x1f" + b"\x00" * 7)
send(0x200B, bytes([0x00, 0x30, 0x00, 0x30, 0x00, 0x00, 0x00]))
send(0x200C, bytes([0x01, 0x00]))
buffer, end = b"", time.time() + 30
while time.time() < end:
    if select.select([fd], [], [], 0.5)[0]:
        buffer += os.read(fd, 1024)
send(0x200C, bytes([0x00, 0x00]))
os.close(fd)

seen = {}
at = 0
while at + 3 <= len(buffer):
    if buffer[at] != 0x04:
        at += 1
        continue
    code, length = buffer[at + 1], buffer[at + 2]
    payload = buffer[at + 3:at + 3 + length]
    at += 3 + length
    if code != 0x3E or not payload or payload[0] != 0x02:
        continue
    count, p = payload[1], 2
    for _ in range(count):
        if p + 9 > len(payload):
            break
        address = bytes(payload[p + 2:p + 8])
        data_length = payload[p + 8]
        data = payload[p + 9:p + 9 + data_length]
        rssi = payload[p + 9 + data_length] - 256 if p + 9 + data_length < len(payload) else 0
        p += 10 + data_length
        q = 0
        while q < len(data) and data[q] and q + 1 + data[q] <= len(data):
            kind, value = data[q + 1], data[q + 2:q + 1 + data[q]]
            if kind == 0x16 and len(value) >= 7 and value[0] == 0x95 and value[1] == 0xFE:
                control = value[2] | value[3] << 8
                product = value[4] | value[5] << 8
                entry = seen.setdefault(address, {"product": product, "controls": set(), "rssi": rssi, "frames": 0})
                entry["controls"].add(control)
                entry["frames"] += 1
                entry["rssi"] = max(entry["rssi"], rssi)
            q += 1 + data[q]

for number, entry in enumerate(seen.values(), 1):
    controls = sorted(entry["controls"])
    print("device %d: product 0x%04x, %d frames, best %d dBm" % (number, entry["product"], entry["frames"], entry["rssi"]))
    for control in controls:
        print("  frame control 0x%04x: encrypted=%s object=%s mac=%s version=%d" % (
            control, bool(control & 0x08), bool(control & 0x40), bool(control & 0x10), control >> 12))
if not seen:
    print("no MiBeacon heard")
