#!/usr/bin/env bash
# MediaTek's 4.9 drvgen.mk invokes "$python tools/dct/DrvGen.py ...".
# The k37 DWS in the pinned MT6737-family tree is an old binary DWS, and
# DrvGen.py immediately delegates that case to tools/dct/old_dct/DrvGen.
# This wrapper skips the Python-2-only front-end and performs that delegation
# directly so the kernel can be built on a modern host without Python 2.
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: mtk-drvgen-wrapper.sh PATH/TO/DrvGen.py DWS [DrvGen args...]" >&2
  exit 2
fi

drvgen_py="$1"
shift
dct_dir="$(cd "$(dirname "$drvgen_py")" && pwd)"
old_drvgen="$dct_dir/old_dct/DrvGen"

if [[ ! -x "$old_drvgen" ]]; then
  echo "old MediaTek DrvGen is missing or not executable: $old_drvgen" >&2
  exit 126
fi

exec "$old_drvgen" "$@"
