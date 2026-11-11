"""Offline evidence primitives. Observations never authorize a device operation."""
import hashlib
import re
from a32_formats import boot_signature, cells, der_item, tail_evidence


REQUIRED = ('memory', 'GIC', 'timer', 'UART0', 'minimum clocks', 'PWRAP',
            'PMIC', 'eMMC power path', 'watchdog handoff')


def readiness_gate(rows, timing):
    """Missing, duplicate, malformed or unproven rows close the gate."""
    if not isinstance(rows, list) or not isinstance(timing, dict):
        return False
    by_name = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('component'), str):
            return False
        name = row.get('component')
        if name in by_name or row.get('ready') not in ('YES', 'PARTIAL', 'NO'):
            return False
        by_name[name] = row
    if any(by_name.get(n, {}).get('ready') != 'YES' or
           not isinstance(by_name[n].get('evidence'), str) or
           not by_name[n]['evidence'].strip() for n in REQUIRED):
        return False
    return (timing.get('ready') in ('YES', 'PARTIAL') and
            timing.get('stock_conservative_proven') is True and
            isinstance(timing.get('evidence'), str) and bool(timing['evidence'].strip()))


def pmic_evidence(text):
    """Extract candidates, not hardware identity, from content rather than filenames."""
    candidates = sorted(set(x.upper() for x in re.findall(
        r'\bMT(?:6328|6350|6351|6353|6397)(?=\b|_)', text, re.I)))
    ids = {name: int(value, 16) for name, value in re.findall(
        r'^\s*#define\s+(PMIC\d+_\w*CID_CODE)\s+(0x[0-9a-fA-F]+)\b', text, re.M)}
    bases = {name: int(value, 16) for name, value in re.findall(
        r'^\s*#define\s+(MT\d+_PMIC_REG_BASE)\s+\(?(0x[0-9a-fA-F]+)\)?\s*$', text, re.M)}
    registers = []
    for name, base, offset in re.findall(
            r'^\s*#define\s+(MT\d+_(?:HWCID|SWCID))\s+\(\((MT\d+_PMIC_REG_BASE)\s*\+\s*(0x[0-9a-fA-F]+)\)\)', text, re.M):
        prefix = name.replace('_HWCID', '_PMIC_HWCID').replace('_SWCID', '_PMIC_SWCID')
        def literal(suffix):
            match = re.search(r'^\s*#define\s+' + prefix + suffix +
                              r'\s+(0x[0-9a-fA-F]+|[0-9]+)\s*$', text, re.M)
            return int(match.group(1), 0) if match else None
        registers.append({'symbol': name, 'address': bases[base] + int(offset, 16)
                          if base in bases else None, 'mask': literal('_MASK'),
                          'shift': literal('_SHIFT')})
    return {'candidates': candidates, 'expected_ids': ids, 'cid_registers': registers,
            'hardware_identity': 'UNKNOWN', 'confidence': 'UNKNOWN',
            'generic_compatible_present': 'mediatek,mt_pmic' in text}


def regulator_graph(nodes, start):
    """Expand supply and known vendor phandles, including indirect supply nodes.

    No inference from similarly named rails. Missing MSDC supplies stay UNKNOWN.
    A graph edge means a reference exists, not that the consumer driver uses it.
    """
    index = {}
    for path, props in nodes.items():
        if ('phandle' in props and 'linux,phandle' in props and
                props['phandle'] != props['linux,phandle']):
            raise ValueError('conflicting phandle aliases')
        for key in ('phandle', 'linux,phandle'):
            if key in props:
                values = cells(props[key])
                if len(values) != 1 or values[0] in (0, 0xffffffff):
                    raise ValueError('invalid phandle')
                handle = values[0]
                if handle in index and index[handle] != path:
                    raise ValueError('duplicate phandle')
                index[handle] = path
    if start not in nodes:
        raise ValueError('missing consumer')
    edges, visited, active = [], set(), set()

    def visit(path):
        if path in active:
            raise ValueError('supply/reference cycle')
        if path in visited:
            return
        active.add(path)
        for key, raw in nodes[path].items():
            if not (key.endswith('-supply') or key in (
                    'pinctl', 'register_setting', 'pinctl_hs200', 'pinctl_ddr50')
                    or re.fullmatch(r'pinctrl-\d+', key)):
                continue
            handles = cells(raw)
            if not handles:
                raise ValueError('empty phandle reference')
            if not key.startswith('pinctrl-') and len(handles) != 1:
                raise ValueError('expected a single provider phandle')
            for handle in handles:
                if handle not in index:
                    raise ValueError('unresolved phandle')
                dest = index[handle]
                edges.append({'from': path, 'property': key, 'to': dest})
                visit(dest)
        active.remove(path)
        visited.add(path)
    visit(start)
    return {'root': start, 'edges': edges, 'vmmc': 'REFERENCED' if
            'vmmc-supply' in nodes[start] else 'UNKNOWN',
            'vqmmc': 'REFERENCED' if 'vqmmc-supply' in nodes[start] else 'UNKNOWN'}


def translate_edge(raw, old_semantics=None, new_semantics=None):
    """Vendor properties are u8; unknown 3.18 semantics must not pick an edge."""
    if len(raw) != 1 or raw[0] not in (0, 1):
        raise ValueError('edge requires one byte, value 0 or 1')
    if old_semantics != {0: 'rising', 1: 'falling'} or old_semantics != new_semantics:
        return {'action': 'BLOCKED', 'value': None, 'reason': 'unproven binding equivalence'}
    return {'action': 'PRESERVE', 'value': raw[0], 'meaning': new_semantics[raw[0]]}


def detect_fixups(text):
    """Lexical leads only; comments/strings do not prove executed FDT writes."""
    clean = re.sub(r'/\*.*?\*/|//[^\n]*|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'',
                   lambda m: '\n' * m.group().count('\n') + ' ', text, flags=re.S)
    names = r'fdt_(?:setprop\w*|add_subnode\w*|open_into|pack)|(?:\w*fixup\w*|\w*atag\w*)'
    return [{'line': clean.count('\n', 0, m.start()) + 1, 'symbol': m.group(1),
             'status': 'REQUIRES_CALL_PATH_AUDIT'}
            for m in re.finditer(r'\b(' + names + r')\s*\(', clean, re.I)]


def memory_overlaps(regions):
    """Half-open intervals; containment also counts, adjacency does not."""
    checked = []
    for r in regions:
        start, size = r['start'], r['size']
        if type(start) is not int or type(size) is not int or start < 0 or size <= 0 or start + size > 2**64:
            raise ValueError('invalid memory interval')
        checked.append((start, start + size, r['owner']))
    return [{'a': a[2], 'b': b[2], 'start': max(a[0], b[0]), 'end': min(a[1], b[1])}
            for i, a in enumerate(checked) for b in checked[i + 1:]
            if max(a[0], b[0]) < min(a[1], b[1])]


def signature_layout(tail):
    """Read every TLV offset from DER length fields; offsets relative to tail."""
    schema = boot_signature(tail)
    tag, content, end = der_item(tail, 0, len(tail))
    result = [{'name': 'SEQUENCE', 'offset': 0, 'length': end,
               'content_offset': content, 'content_length': end - content, 'tag': tag}]
    pos = content
    for name in ('formatVersion', 'certificate', 'algorithmIdentifier',
                 'authenticatedAttributes', 'signature'):
        tag, value, finish = der_item(tail, pos, end)
        result.append({'name': name, 'offset': pos, 'length': finish - pos,
                       'content_offset': value, 'content_length': finish - value, 'tag': tag})
        if name == 'authenticatedAttributes':
            sub = value
            for child in ('target', 'signedLength'):
                ct, cv, ce = der_item(tail, sub, finish)
                result.append({'name': name + '.' + child, 'offset': sub,
                               'length': ce - sub, 'content_offset': cv,
                               'content_length': ce - cv, 'tag': ct})
                sub = ce
        pos = finish
    return {'classification': 'BootSignature-v1-like', 'schema': schema, 'sections': result,
            'after_der': {'offset': end, 'length': len(tail) - end}}


def hashes(data):
    return {name: hashlib.new(name, data).hexdigest() for name in ('sha1', 'sha256', 'sha512')}


def final_block(block, candidates):
    if len(block) != 256:
        raise ValueError('expected exactly 256 bytes')
    try:
        tag, start, end = der_item(block, 0, len(block))
        asn1 = {'tag': tag, 'end': end, 'whole_tlv': end == len(block),
                'note': 'TLV framing only; no ASN.1 schema established'}
    except ValueError as error:
        asn1 = {'error': str(error)}
    comparisons = {}
    for name, data in candidates.items():
        comparisons[name] = {'hashes': hashes(data), 'raw_equals_block': data == block,
                            'digest_equals_block': any(hashlib.new(h, data).digest() == block
                                                      for h in ('sha1', 'sha256', 'sha512'))}
    return {'classification': 'UNKNOWN', 'bytes': 256, 'hashes': hashes(block),
            'entropy': tail_evidence(block)['entropy_bits_per_byte'], 'asn1': asn1,
            'all_zero_or_ff': len(set(block)) == 1 and block[0] in (0, 255),
            'plaintext_pkcs1_prefix': block[:2].hex() if block[:2] in (b'\0\x01', b'\0\x02') else None,
            'rsa2048_size_only': True, 'candidate_comparisons': comparisons}


def byte_diff(a, b):
    if len(a) != len(b):
        raise ValueError('different block lengths')
    return [{'offset': i, 'a': x, 'b': y, 'xor': x ^ y}
            for i, (x, y) in enumerate(zip(a, b)) if x != y]
