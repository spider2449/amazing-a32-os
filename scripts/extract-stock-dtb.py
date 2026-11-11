#!/usr/bin/env python3
"""Extract only a uniquely validated, directly appended FDT; never guess offsets."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
from a32_formats import boot, fdt, strings


def extract(data):
    info = boot(data)
    candidates = info['appended_fdts']
    if len(candidates) != 1 or len(info['fdt_candidates_kernel']) != 1 or info['invalid_fdt_magic_kernel']:
        raise ValueError('ambiguous, missing or malformed FDT candidates; inspect manually')
    item = candidates[0]
    kernel = info['sections']['kernel']
    if item['offset'] != info['zimage']['size'] or item['end'] != kernel['size']:
        raise ValueError('unknown DT container, gap or data after appended FDT')
    start, end = kernel['start'] + item['offset'], kernel['start'] + item['end']
    blob = data[start:end]
    tree = fdt(blob)
    info['stock_dtb'] = dict(item, absolute_offset=start, absolute_end=end,
                            bytes_after_dtb_in_kernel=kernel['end'] - end,
                            bytes_after_dtb_in_image=len(data) - end,
                            model=strings(tree['nodes']['/'].get('model', b'UNKNOWN\0')),
                            compatible=strings(tree['nodes']['/'].get('compatible', b'UNKNOWN\0')))
    info['container'] = 'CONFIRMED direct FDT at zImage end; no intervening MTK/DTBO container'
    return blob, info


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('image', type=Path)
    p.add_argument('--out', type=Path, default=Path('out'))
    p.add_argument('--dtc', default=shutil.which('dtc'))
    p.add_argument('--legacy-unit-addresses', action='store_true',
                   help='disable only dtc unit_address_vs_reg error for legacy vendor node names')
    args = p.parse_args()
    try:
        blob, info = extract(args.image.read_bytes())
        if not args.dtc:
            raise ValueError('dependency missing: dtc; specify existing binary with --dtc; no download attempted')
        args.out.mkdir(parents=True, exist_ok=True)
        dtb, dts = args.out / 'stock-a32.dtb', args.out / 'stock-a32.dts'
        dtb.write_bytes(blob)
        command = [args.dtc, '-I', 'dtb', '-O', 'dts', '-o', str(dts), str(dtb)]
        if args.legacy_unit_addresses:
            command += ['-E', 'no-unit_address_vs_reg']
        subprocess.run(command, check=True)
        info['dtc_command'] = command
        (args.out / 'stock-a32-extraction.json').write_text(json.dumps(info, indent=2) + '\n')
        print(json.dumps(info, indent=2))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
