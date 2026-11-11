#!/usr/bin/env python3
"""Evaluate a reviewed evidence matrix. Exit 2 on uncertainty; creates no DTS."""
import argparse
import json
from pathlib import Path
from a32_boot_critical import readiness_gate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('matrix', type=Path)
    args = parser.parse_args()
    try:
        matrix = json.loads(args.matrix.read_text())
        ready = readiness_gate(matrix['components'], matrix['eMMC timing'])
    except (OSError, ValueError, KeyError, TypeError):
        ready = False
    print('DTS GATE: ' + ('READY' if ready else 'NOT READY'))
    return 0 if ready else 2


if __name__ == '__main__':
    raise SystemExit(main())
