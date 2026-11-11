#!/usr/bin/env python3
"""Content search of existing local roots; no download, no hardware identity inference.

Binary/oversized files and errors are counted explicitly. No missing source or
empty lexical result is proof that bootloader fixups are absent.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
from a32_boot_critical import detect_fixups, pmic_evidence


PATTERN = re.compile(r'mt_pmic|pwrap|pmic|MT6328|MT6350|MT6351|MT6353|MT6397|'
                     r'regulator|VEMC|VMC|VMCH|VIO18|VCORE|VPROC|VSRAM|VUSB', re.I)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('roots', nargs='+', type=Path)
    p.add_argument('--out', type=Path, default=Path('out/source-evidence'))
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    report = {'roots': [str(x.resolve()) for x in args.roots], 'text_files': 0,
              'matching_lines': 0, 'skipped_binary': 0, 'skipped_oversized': [],
              'errors': [], 'source_candidates': [], 'pmic': [], 'fixup_leads': []}
    output_root = args.out.resolve()

    def error(e):
        report['errors'].append(str(e))

    with (args.out/'matches.txt').open('w', encoding='utf-8') as output:
        for root in args.roots:
            if not root.is_dir():
                report['errors'].append('missing directory: '+str(root))
                continue
            for base, dirs, files in os.walk(root, onerror=error, followlinks=False):
                dirs[:] = [d for d in dirs if d not in ('.git', '__pycache__') and
                           (Path(base)/d).resolve() != output_root]
                for name in files:
                    path = Path(base)/name
                    try:
                        if path.stat().st_size > 3_000_000:
                            report['skipped_oversized'].append(str(path))
                            continue
                        data = path.read_bytes()
                    except OSError as e:
                        error(e)
                        continue
                    if b'\0' in data:
                        report['skipped_binary'] += 1
                        continue
                    text = data.decode('utf-8', errors='replace')
                    report['text_files'] += 1
                    if name in ('BootSignature.java', 'boot_signer', 'AGENTS.md') or re.search(
                            r'/(?:lk|preloader)/', path.as_posix()):
                        report['source_candidates'].append(str(path))
                    hits = [(i, line) for i, line in enumerate(text.splitlines(), 1) if PATTERN.search(line)]
                    report['matching_lines'] += len(hits)
                    for number, line in hits:
                        output.write(f'{path}:{number}:{line}\n')
                    if ('PMIC_REG_BASE' in text and 'SWCID' in text) or 'CID_CODE' in text:
                        report['pmic'].append({'path': str(path), 'sha256': hashlib.sha256(data).hexdigest(),
                                               'observations': pmic_evidence(text)})
                    if path.suffix in ('.c', '.h', '.cpp', '.cc') and re.search(
                            r'/(?:lk|preloader)/', path.as_posix()):
                        report['fixup_leads'].append({'path': str(path), 'leads': detect_fixups(text)})
    report['scope_warning'] = 'Lexical inventory only; absent source and skipped inputs remain UNKNOWN.'
    (args.out/'inventory.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: report[k] for k in ('text_files','matching_lines','source_candidates','errors')}, indent=2))
    return 2 if report['errors'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
