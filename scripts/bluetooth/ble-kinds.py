# Experiment: what kinds of data nearby BLE devices advertise. Prints, for each
# service data UUID and manufacturer ID, how many devices sent it, and nothing of
# the data itself, to keep nearby devices private.
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

buffer, end = b"", time.time() + 20
while time.time() < end:
    if select.select([fd], [], [], 0.5)[0]:
        buffer += os.read(fd, 1024)
send(0x200C, bytes([0x00, 0x00]))
os.close(fd)

kinds = {}
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
        p += 10 + data_length
        q = 0
        while q < len(data) and data[q] and q + 1 + data[q] <= len(data):
            kind, value = data[q + 1], data[q + 2:q + 1 + data[q]]
            if kind == 0x16 and len(value) >= 2:
                key = "service data 0x%04x" % (value[0] | value[1] << 8)
            elif kind == 0xFF and len(value) >= 2:
                key = "manufacturer 0x%04x" % (value[0] | value[1] << 8)
            else:
                key = None
            if key:
                kinds.setdefault(key, set()).add(address)
            q += 1 + data[q]

for key, addresses in sorted(kinds.items(), key=lambda item: -len(item[1])):
    print(key, len(addresses), "devices")
