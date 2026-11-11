#!/usr/bin/env python3
"""Produce reproducible raw-property inventory; phandles resolve by node path.

No binding compatibility is inferred from numerical phandle equality.
"""
import argparse
import json
from pathlib import Path
import re
from a32_formats import fdt, cells, strings, sha


def value(data):
    if not data:
        return 'true (boolean)'
    if data[-1:] == b'\0' and all(part and all(b in (9, 10, 13) or 32 <= b < 127 for b in part)
                                for part in data[:-1].split(b'\0')):
        return json.dumps(data[:-1].decode().split('\0'))
    if len(data) > 256:
        return f'{len(data)} bytes; sha256={sha(data)} (large property omitted)'
    if len(data) % 4 == 0:
        return '<' + ' '.join(hex(n) for n in cells(data)) + '>'
    return '[' + data.hex(' ') + ']'


def references(name, data, tree):
    # Only declared reference properties; cells in reg/IRQs are never phandles.
    provider = {'clocks': '#clock-cells', 'resets': '#reset-cells',
                'power-domains': '#power-domain-cells', 'phys': '#phy-cells',
                'dmas': '#dma-cells', 'pwms': '#pwm-cells',
                'gpios': '#gpio-cells', 'interrupts-extended': '#interrupt-cells'}
    simple = name in ('interrupt-parent', 'pinctl', 'register_setting', 'cpu', 'memory-region',
                      'operating-points-v2', 'next-level-cache', 'remote-endpoint') or name.endswith('-supply') or re.fullmatch(r'pinctrl-\d+', name) or name.startswith('pinctl_')
    key = 'gpios' if name.endswith('-gpios') else name
    if not simple and key not in provider:
        return []
    if len(data) % 4:
        return ['UNKNOWN non-cell reference encoding']
    result, p, values = [], 0, cells(data)
    while p < len(values):
        h = values[p]
        path = tree['handles'].get(h)
        if path is None:
            result.append(f'UNKNOWN phandle {hex(h)}')
            break
        p += 1
        n = 0
        if not simple:
            raw = tree['nodes'][path].get(provider[key])
            if raw is None or len(raw) != 4:
                result.append(f'{path}: UNKNOWN {provider[key]}')
                break
            n = cells(raw)[0]
        if p + n > len(values):
            result.append(f'{path}: TRUNCATED specifier')
            break
        result.append(path + (' args=' + str(values[p:p+n]) if n else ''))
        p += n
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('dtb', type=Path)
    p.add_argument('output', type=Path)
    args = p.parse_args()
    blob = args.dtb.read_bytes()
    tree = fdt(blob)
    lines = ['# DT property inventory', '', f'Input: `{args.dtb}`; {len(blob)} bytes; SHA256 `{sha(blob)}`.',
             '', 'Generated from validated FDT, corresponding to dtc decompilation. Raw cell arrays',
             'retain parent bus encodings; do not interpret every reg tuple as 32-bit address/size.',
             'Absent status means enabled by DT convention, subject to parent status and driver binding.',
             'Missing properties are UNKNOWN, never synthesized. Large properties are hashed.',
             'Reference resolution uses provider cell counts; vendor custom bindings still require driver audit.',
             '', f'Memreserve entries: `{tree["reservations"]}`.', '']
    for path, props in tree['nodes'].items():
        lines += [f'## `{path}`', '', '| Property | Value | Resolved references |', '|---|---|---|']
        if 'status' not in props:
            lines += ['| status | ABSENT (default enabled; check ancestors) | |']
        for name, data in props.items():
            val = value(data).replace('|', '\\|').replace('\n', ' ')
            refs = '; '.join(references(name, data, tree))
            lines += [f'| {name} | `{val}` | {refs} |']
        lines += ['']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join(lines).rstrip() + '\n')


if __name__ == '__main__':
    main()
