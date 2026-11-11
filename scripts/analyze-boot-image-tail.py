#!/usr/bin/env python3
"""Offline Android v0 tail evidence. UNKNOWN never means safe to remove."""
import argparse
import json
from pathlib import Path
import sys
import shutil
import subprocess
from a32_formats import boot, tail_evidence, boot_signature, sha


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('image', type=Path)
    p.add_argument('--out', type=Path, default=Path('out'))
    p.add_argument('--verify-embedded-signature', action='store_true',
                   help='offline openssl check against embedded certificate only; does not prove device trust')
    args = p.parse_args()
    try:
        data = args.image.read_bytes()
        info = boot(data)
        tail = data[info['logical_end']:]
        info['tail_evidence'] = tail_evidence(tail)
        args.out.mkdir(parents=True, exist_ok=True)
        try:
            signature = boot_signature(tail)
            info['der_signature'] = signature
        except (ValueError, UnicodeError) as error:
            signature = None
            info['der_signature'] = {'classification': 'UNKNOWN', 'evidence': str(error)}
        if signature:
            end = signature['der_size']
            (args.out / 'boot-signature.der').write_bytes(tail[:end])
            rest = tail[end:]
            info['after_der_evidence'] = tail_evidence(rest)
            if len(tail) >= 2304:
                info['offset_2048_block'] = {'bytes': 256, 'sha256': sha(tail[2048:2304]),
                                           'evidence': tail_evidence(tail[2048:2304])}
            if args.verify_embedded_signature:
                if not shutil.which('openssl'):
                    raise ValueError('dependency missing: openssl; no download attempted')
                if signature['signed_length'] != info['logical_end']:
                    raise ValueError('signed length differs from Android logical end')
                a, b = signature['certificate_range']
                cert = args.out / 'embedded-certificate.der'
                cert.write_bytes(tail[a:b])
                result = subprocess.run(['openssl', 'x509', '-inform', 'DER', '-in', str(cert),
                                         '-pubkey', '-noout'], capture_output=True, check=True)
                public = args.out / 'embedded-public-key.pem'
                public.write_bytes(result.stdout)
                a, b = signature['attributes_range']
                signed = args.out / 'signed-content.bin'
                signed.write_bytes(data[:info['logical_end']] + tail[a:b])
                a, b = signature['signature_range']
                sigfile = args.out / 'embedded-signature.bin'
                sigfile.write_bytes(tail[a:b])
                result = subprocess.run(['openssl', 'dgst', '-sha256', '-verify', str(public),
                                         '-signature', str(sigfile), str(signed)], capture_output=True, text=True)
                signature['embedded_key_verification'] = {'returncode': result.returncode,
                                                          'output': result.stdout.strip(),
                                                          'errors': result.stderr.strip(),
                                                          'scope': 'cryptographic consistency only; device trust and final 256-byte block UNKNOWN'}
        (args.out / 'boot-trailer.bin').write_bytes(tail)
        (args.out / 'boot-tail.json').write_text(json.dumps(info, indent=2) + '\n')
        # Bounded initial/final views plus first nonzero span; preserve full tail separately.
        span = info['nonzero_span']
        offsets = {0, max(0, len(tail) - 256)}
        if span:
            offsets.add(span[0] - info['logical_end'])
        lines = []
        for offset in sorted(offsets):
            for i in range(offset, min(offset + 256, len(tail)), 16):
                lines.append(f'{info["logical_end"] + i:08x}: {tail[i:i+16].hex(" ")}')
        (args.out / 'boot-trailer.hex.txt').write_text('\n'.join(lines) + '\n')
        print(json.dumps({k: v for k, v in info.items() if k != 'tail_evidence'}, indent=2))
        if args.verify_embedded_signature and (not signature or
                signature.get('embedded_key_verification', {}).get('returncode') != 0):
            print('error: embedded signature verification was not successful; see forensic report', file=sys.stderr)
            return 2
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
