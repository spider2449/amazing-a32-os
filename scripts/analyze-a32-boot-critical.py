#!/usr/bin/env python3
"""Reproduce local A32 DER/trailer and DT graph evidence; no device access."""
import argparse
import itertools
import json
from pathlib import Path
import subprocess
import tempfile
from a32_formats import boot, fdt
from a32_boot_critical import (byte_diff, final_block, regulator_graph,
                               signature_layout, hashes)


def verify(block, candidates, certificate, directory):
    """Only verification/recovery with the embedded public key; never signing."""
    cert = directory / 'certificate.der'
    cert.write_bytes(certificate)
    key = directory / 'public.pem'
    encoding = 'PEM' if certificate.startswith(b'-----BEGIN') else 'DER'
    pub = subprocess.run(['openssl', 'x509', '-inform', encoding, '-in', str(cert),
                          '-pubkey', '-noout'], capture_output=True, check=True)
    key.write_bytes(pub.stdout)
    sig = directory / 'final.bin'
    sig.write_bytes(block)
    results = {}
    recovery = subprocess.run(['openssl', 'pkeyutl', '-verifyrecover', '-pubin',
                               '-inkey', str(key), '-in', str(sig),
                               '-pkeyopt', 'rsa_padding_mode:none'], capture_output=True)
    results['raw_public_operation'] = {'returncode': recovery.returncode,
        'errors': recovery.stderr.decode(errors='replace'),
        'recovered_hex': recovery.stdout.hex(),
        'warning': 'a successful raw operation alone does not verify a signature'}
    recovered = subprocess.run(['openssl', 'pkeyutl', '-verifyrecover', '-pubin',
                               '-inkey', str(key), '-in', str(sig),
                               '-pkeyopt', 'rsa_padding_mode:pkcs1'], capture_output=True)
    results['pkcs1_recovery'] = {'returncode': recovered.returncode,
        'recovered_hex': recovered.stdout.hex(), 'errors': recovered.stderr.decode(errors='replace')}
    for name, content in candidates.items():
        data = directory / 'candidate.bin'
        data.write_bytes(content)
        for digest in ('sha1', 'sha256', 'sha512'):
            for padding in ('pkcs1', 'pss'):
                command = ['openssl', 'dgst', '-' + digest, '-verify', str(key),
                           '-signature', str(sig), '-sigopt', 'rsa_padding_mode:' + padding]
                if padding == 'pss':
                    command += ['-sigopt', 'rsa_pss_saltlen:auto']
                r = subprocess.run(command + [str(data)], capture_output=True)
                results[name + '/' + digest + '/' + padding] = {
                    'returncode': r.returncode, 'output': r.stdout.decode(errors='replace').strip(),
                    'errors': r.stderr.decode(errors='replace').strip()}
    return results


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--image', nargs=2, action='append', metavar=('LABEL', 'PATH'), required=True)
    p.add_argument('--dtb', type=Path, required=True)
    p.add_argument('--out', type=Path, default=Path('out'))
    p.add_argument('--public-certificate', type=Path, action='append', default=[],
                   help='additional existing public certificate only; no download')
    args = p.parse_args()
    try:
        args.out.mkdir(parents=True, exist_ok=True)
        reports, blocks, layouts = {}, {}, {}
        for label, filename in args.image:
            if label in reports:
                raise ValueError('duplicate image label')
            image = Path(filename).read_bytes()
            info = boot(image)
            tail = image[info['logical_end']:]
            # Envelope boundary is observed from the nonzero span, not DER length.
            span = info['nonzero_span']
            if not span or span[0] != info['logical_end'] or span[1] - span[0] != 2304:
                raise ValueError('unrecognized active envelope extent')
            active = tail[:2304]
            layout = signature_layout(active)
            layout['absolute_image_offset'] = info['logical_end']
            layout['image_sha256'] = info['image_sha256']
            layouts[label] = layout
            schema = layout['schema']
            if schema['signed_length'] != info['logical_end'] or schema['der_size'] > 2048:
                raise ValueError('inconsistent signed length/envelope')
            a, b = schema['certificate_range']
            certificate = active[a:b]
            a, b = schema['signature_range']
            signature = active[a:b]
            a, b = schema['attributes_range']
            attributes = active[a:b]
            sections = {k: image[v['start']:v['end']] for k, v in info['sections'].items()}
            candidates = {'header_1632': image[:1632], 'header_page': image[:info['pagesize']],
                'kernel': sections['kernel'], 'ramdisk': sections['ramdisk'],
                'kernel+ramdisk': sections['kernel'] + sections['ramdisk'],
                'logical_image': image[:info['logical_end']], 'envelope_2048': active[:2048],
                'DER': active[:schema['der_size']], 'certificate': certificate,
                'first_signature': signature,
                'logical_image+attributes': image[:info['logical_end']] + attributes,
                'image+envelope_2048': image[:info['logical_end']] + active[:2048]}
            block = active[2048:2304]
            report = final_block(block, candidates)
            report.update(image_sha256=info['image_sha256'], logical_end=info['logical_end'],
                          certificate_hashes=hashes(certificate),
                          der_to_2048_zero_padding=not any(active[schema['der_size']:2048]))
            with tempfile.TemporaryDirectory(prefix='a32-verify-') as tmp:
                directory = Path(tmp)
                report['embedded_key_attempts'] = verify(block, candidates, certificate, directory)
                # Repeat the known first signature check from original image bytes.
                (directory / 'first.bin').write_bytes(signature)
                (directory / 'signed.bin').write_bytes(candidates['logical_image+attributes'])
                r = subprocess.run(['openssl', 'dgst', '-sha256', '-verify', str(directory/'public.pem'),
                                    '-signature', str(directory/'first.bin'), str(directory/'signed.bin')],
                                   capture_output=True)
                report['first_signature_verification'] = {'returncode': r.returncode,
                    'output': r.stdout.decode().strip(), 'errors': r.stderr.decode().strip()}
                report['additional_public_certificates'] = {}
                for public_cert in args.public_certificate:
                    content = public_cert.read_bytes()
                    report['additional_public_certificates'][hashes(content)['sha256']] = verify(
                        block, candidates, content, directory)
            reports[label], blocks[label] = report, block
        diffs = {a + ':' + b: byte_diff(blocks[a], blocks[b])
                 for a, b in itertools.combinations(blocks, 2)}
        (args.out/'boot-signature-layout.json').write_text(json.dumps(layouts, indent=2)+'\n')
        (args.out/'final-trailer-analysis.json').write_text(json.dumps(
            {'images': reports, 'byte_differences': diffs}, indent=2)+'\n')
        tree = fdt(args.dtb.read_bytes())
        graphs = {path: regulator_graph(tree['nodes'], path) for path in tree['nodes']
                  if path.endswith('/msdc0@11230000') or path.endswith('/regulators_supply')}
        (args.out/'a32-regulator-graph.json').write_text(json.dumps(graphs, indent=2)+'\n')
        print(json.dumps({'images': {k: {'classification': v['classification'], 'entropy': v['entropy'],
                         'first_verification': v['first_signature_verification'],
                         'final_signature_successes': [n for n, r in v['embedded_key_attempts'].items()
                                            if '/' in n and r['returncode'] == 0]} for k,v in reports.items()},
                          'diff_counts': {k: len(v) for k,v in diffs.items()}, 'graphs': graphs}, indent=2))
        return 0
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print('error:', error)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
