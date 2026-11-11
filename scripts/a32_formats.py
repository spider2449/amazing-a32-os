"""Read-only, bounded Android v0 and FDT v17 forensic parsers.

Unknown tails are evidence, never authorization to repack. No device I/O.
"""
import hashlib
import math
import struct
from collections import Counter

MAGIC = b'\xd0\x0d\xfe\xed'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def align(value, boundary=4):
    return (value + boundary - 1) // boundary * boundary


def cells(data):
    if len(data) % 4:
        raise ValueError('property is not a cell array')
    return list(struct.unpack('>' + 'I' * (len(data) // 4), data))


def strings(data):
    if not data or data[-1:] != b'\0':
        raise ValueError('unterminated string property')
    return data[:-1].decode('ascii').split('\0')


def fdt(data):
    if len(data) < 40 or data[:4] != MAGIC:
        raise ValueError('invalid/truncated FDT header')
    _, total, so, st, ro, version, last, cpu, ss, sz = struct.unpack_from('>10I', data)
    if not 40 <= total <= len(data) or version != 17 or last > 17:
        raise ValueError('unsupported FDT version or invalid totalsize')
    if so % 4 or ro % 8 or min(so, st, ro) < 40:
        raise ValueError('invalid FDT block offsets/alignment')
    if so + sz > total or st + ss > total:
        raise ValueError('FDT block exceeds totalsize')
    rp = ro
    reservations = []
    while True:
        if rp + 16 > total:
            raise ValueError('unterminated FDT reservations')
        address, size = struct.unpack_from('>QQ', data, rp)
        rp += 16
        if address == size == 0:
            break
        reservations.append([address, size])
    blocks = sorted([(0, 40), (ro, rp), (so, so + sz), (st, st + ss)])
    if any(a[1] > b[0] for a, b in zip(blocks, blocks[1:])):
        raise ValueError('overlapping FDT blocks')
    nodes, stack = {}, []
    p, end = so, so + sz
    ended = False
    while p < end:
        if p + 4 > end:
            raise ValueError('truncated FDT token')
        token = struct.unpack_from('>I', data, p)[0]
        p += 4
        if token == 1:
            z = data.find(b'\0', p, end)
            if z < 0:
                raise ValueError('unterminated node name')
            name = data[p:z].decode('ascii')
            if '/' in name or (not stack and (name or nodes)) or (stack and not name):
                raise ValueError('invalid FDT node name/root')
            path = (stack[-1].rstrip('/') + '/' + name) if stack else '/'
            if path in nodes:
                raise ValueError('duplicate FDT node')
            nodes[path] = {}
            stack.append(path)
            p = align(z + 1)
        elif token == 2:
            if not stack:
                raise ValueError('unbalanced FDT end-node')
            stack.pop()
        elif token == 3:
            if not stack or p + 8 > end:
                raise ValueError('invalid/truncated FDT property')
            length, nameoff = struct.unpack_from('>II', data, p)
            p += 8
            if nameoff >= ss or p + length > end:
                raise ValueError('FDT property out of bounds')
            z = data.find(b'\0', st + nameoff, st + ss)
            if z < 0:
                raise ValueError('unterminated property name')
            name = data[st + nameoff:z].decode('ascii')
            if not name or name in nodes[stack[-1]]:
                raise ValueError('empty/duplicate FDT property')
            nodes[stack[-1]][name] = data[p:p + length]
            p = align(p + length)
        elif token == 4:
            pass
        elif token == 9:
            if stack or '/' not in nodes or any(data[p:end]):
                raise ValueError('invalid FDT end/root')
            ended = True
            break
        else:
            raise ValueError('unknown FDT token')
        if p > end:
            raise ValueError('FDT alignment exceeds structure')
    if not ended:
        raise ValueError('missing FDT end')
    handles = {}
    for path, props in nodes.items():
        if 'phandle' in props and 'linux,phandle' in props and props['phandle'] != props['linux,phandle']:
            raise ValueError('conflicting phandle aliases')
        for key in ('phandle', 'linux,phandle'):
            if key in props:
                values = cells(props[key])
                if len(values) != 1 or values[0] in (0, 0xffffffff):
                    raise ValueError('invalid phandle')
                h = values[0]
                if h in handles and handles[h] != path:
                    raise ValueError('duplicate phandle')
                handles[h] = path
    return {'totalsize': total, 'version': version, 'boot_cpuid': cpu,
            'reservations': reservations, 'nodes': nodes, 'handles': handles}


def scan_fdt(data):
    valid, invalid = [], []
    offset = 0
    while True:
        offset = data.find(MAGIC, offset)
        if offset < 0:
            break
        try:
            tree = fdt(data[offset:])
            valid.append({'offset': offset, 'totalsize': tree['totalsize'],
                          'end': offset + tree['totalsize'],
                          'sha256': sha(data[offset:offset + tree['totalsize']])})
        except (ValueError, UnicodeError, struct.error) as error:
            invalid.append({'offset': offset, 'error': str(error)})
        offset += 4
    return valid, invalid


def boot(data):
    if len(data) < 1632 or data[:8] != b'ANDROID!':
        raise ValueError('expected Android v0 header at offset zero')
    sizes = struct.unpack_from('<10I', data, 8)
    ks, ka, rs, ra, second, sa, tags, page, version, osver = sizes
    if version != 0 or page not in (2048, 4096, 8192, 16384):
        raise ValueError('unsupported version/legacy DT layout/pagesize')
    if len(data) < page or any(data[1632:page]):
        raise ValueError('truncated header or unknown header extension')
    offset, sections = page, {}
    for name, size in [('kernel', ks), ('ramdisk', rs), ('second', second)]:
        end, padded = offset + size, offset + align(size, page)
        if padded > len(data):
            raise ValueError('truncated section/padding')
        if any(data[end:padded]):
            raise ValueError('unknown nonzero section padding')
        sections[name] = {'start': offset, 'size': size, 'end': end, 'padded_end': padded,
                          'sha256': sha(data[offset:end])}
        offset = padded
    if not ks or not rs:
        raise ValueError('empty kernel/ramdisk')
    kernel = data[page:page + ks]
    if len(kernel) < 48 or struct.unpack_from('<I', kernel, 36)[0] != 0x016f2818:
        raise ValueError('expected ARM32 zImage')
    start, end = struct.unpack_from('<II', kernel, 40)
    length = end - start
    if not 48 <= length <= ks:
        raise ValueError('invalid zImage start/end')
    valid, invalid = scan_fdt(kernel)
    appended = [v for v in valid if v['offset'] >= length]
    tail = data[offset:]
    first_nonzero = next((i for i, b in enumerate(tail) if b), None)
    last_nonzero = next((len(tail) - i for i, b in enumerate(reversed(tail)) if b), None)
    image_id = hashlib.sha1()
    for section in sections.values():
        image_id.update(data[section['start']:section['end']])
        image_id.update(struct.pack('<I', section['size']))
    cmdline = data[64:576].split(b'\0')[0] + data[608:1632].split(b'\0')[0]
    return {'pagesize': page, 'header_version': version, 'os_version_raw': hex(osver),
            'name': data[48:64].split(b'\0')[0].decode('ascii', errors='replace'),
            'id_matches': data[576:608] == image_id.digest() + bytes(12),
            'load_addresses': dict(kernel=hex(ka), ramdisk=hex(ra), second=hex(sa), tags=hex(tags)),
            'cmdline': cmdline.decode('ascii', errors='replace'), 'sections': sections,
            'zimage': {'start_field': start, 'end_field': end, 'size': length,
                       'absolute_end': page + length},
            'fdt_candidates_kernel': valid, 'invalid_fdt_magic_kernel': invalid,
            'appended_fdts': appended, 'appended_data_start': page + length,
            'logical_end': offset, 'actual_end': len(data), 'trailer_size': len(tail),
            'trailer_sha256': sha(tail),
            'nonzero_span': ([offset + first_nonzero, offset + last_nonzero] if first_nonzero is not None else None),
            'trailer_classification': 'UNKNOWN' if first_nonzero is not None else 'CONFIRMED zero padding or absent',
            'image_sha256': sha(data)}


def tail_evidence(tail):
    counts = Counter(tail)
    entropy = -sum((n / len(tail)) * math.log2(n / len(tail)) for n in counts.values()) if tail else 0
    magics = [b'AVB0', b'AVBf', b'ANDROID!', b'SEANDROIDENFORCE', b'BFBF', b'CERT',
              b'RSA', b'SHA', b'MediaTek', b'MTK', b'SIGN', b'SIG', b'DTBO', b'\xd7\xb7\xab\x1e',
              b'\x88\x16\x88\x58', MAGIC, b'\xbf\xbf\xbf\xbf']
    hits = {}
    for magic in magics:
        offsets, p = [], 0
        while (p := tail.find(magic, p)) >= 0:
            offsets.append(p)
            p += len(magic)
        if offsets:
            hits[magic.hex()] = offsets
    import re
    return {'entropy_bits_per_byte': entropy, 'byte_counts': dict(sorted(counts.items())),
            'magic_hits_relative': hits,
            'strings': [{'offset': m.start(), 'text': m.group().decode('ascii')}
                        for m in re.finditer(rb'[\x20-\x7e]{4,}', tail)]}


def der_item(data, offset, limit):
    """Bounded single-byte-tag DER TLV; reject indefinite/nonminimal lengths."""
    if offset < 0 or limit < 0 or offset + 2 > limit or limit > len(data):
        raise ValueError('truncated DER tag/length')
    tag, first = data[offset:offset + 2]
    if tag & 31 == 31:
        raise ValueError('unsupported high-tag DER')
    start = offset + 2
    if first & 128:
        count = first & 127
        if not 1 <= count <= 4 or start + count > limit or data[start] == 0:
            raise ValueError('invalid DER length')
        length = int.from_bytes(data[start:start + count], 'big')
        if length < 128:
            raise ValueError('nonminimal DER length')
        start += count
    else:
        length = first
    end = start + length
    if end > limit:
        raise ValueError('truncated DER value')
    return tag, start, end


def boot_signature(tail):
    """Recognize a narrow BootSignature v1 schema, not a bootloader trust claim."""
    tag, start, end = der_item(tail, 0, len(tail))
    if tag != 0x30:
        raise ValueError('tail is not DER sequence')
    parts, pos = [], start
    while pos < end:
        tag, content, finish = der_item(tail, pos, end)
        parts.append((tag, pos, content, finish))
        pos = finish
    if [p[0] for p in parts] != [2, 0x30, 0x30, 0x30, 4]:
        raise ValueError('unknown DER signature schema')
    if tail[parts[0][2]:parts[0][3]] != b'\x01':
        raise ValueError('unsupported signature version')
    # sha256WithRSAEncryption OID, with optional DER NULL parameters.
    alg = tail[parts[2][2]:parts[2][3]]
    if alg not in (bytes.fromhex('06092a864886f70d01010b'), bytes.fromhex('06092a864886f70d01010b0500')):
        raise ValueError('unsupported signature algorithm')
    attrs = parts[3]
    tag, content, finish = der_item(tail, attrs[2], attrs[3])
    if tag != 0x13:
        raise ValueError('unknown target encoding')
    target = tail[content:finish].decode('ascii')
    tag, content, finish = der_item(tail, finish, attrs[3])
    if tag != 2 or finish != attrs[3] or content == finish:
        raise ValueError('invalid signed length')
    integer = tail[content:finish]
    if integer[0] & 128 or (len(integer) > 1 and integer[0] == 0 and integer[1] < 128):
        raise ValueError('noncanonical/negative signed length')
    if parts[4][3] - parts[4][2] != 256:
        raise ValueError('unexpected RSA signature size')
    return {'classification': 'BootSignature-v1-like (schema; trust unproven)',
            'der_size': end, 'target': target, 'signed_length': int.from_bytes(integer, 'big'),
            'algorithm': 'sha256WithRSAEncryption', 'signature_bytes': 256,
            'certificate_range': [parts[1][1], parts[1][3]],
            'attributes_range': [attrs[1], attrs[3]],
            'signature_range': [parts[4][2], parts[4][3]],
            'after_der_bytes': len(tail) - end,
            'after_der_classification': 'UNKNOWN' if any(tail[end:]) else 'CONFIRMED zero padding or absent'}
