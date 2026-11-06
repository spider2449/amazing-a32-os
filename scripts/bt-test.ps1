# Tests the Bluetooth radio without the kernel's Bluetooth stack, which this kernel
# lacks: the Python scripts in scripts/bluetooth send HCI commands straight to
# MediaTek's /dev/stpbt. Needs Alpine with python3 (apk add python3). The scripts are
# copied over the serial console and removed afterwards.
param([string]$Port = 'COM3')

$ErrorActionPreference = 'Stop'
$console = Join-Path $PSScriptRoot 'console.ps1'
foreach ($name in 'hci-probe.py', 'ble-scan.py') {
    $bytes = [IO.File]::ReadAllBytes((Join-Path $PSScriptRoot "bluetooth/$name"))
    $b64 = [Convert]::ToBase64String($bytes)
    $lines = @('sh rm -f /data/alpine/root/upload.b64')
    for ($at = 0; $at -lt $b64.Length; $at += 400) {
        $lines += "sh echo -n $($b64.Substring($at, [Math]::Min(400, $b64.Length - $at))) >> /data/alpine/root/upload.b64"
    }
    $lines += "alpine base64 -d /root/upload.b64 > /tmp/$name && rm /root/upload.b64 && python3 /tmp/$name 2>&1; rm -f /tmp/$name"
    # Only the output of the last line is of interest.
    $output = & $console -Port $Port -Seconds 40 -Lines $lines
    $start = [array]::LastIndexOf([string[]]$output, ($output | Where-Object { $_ -like '### alpine base64*' } | Select-Object -Last 1))
    "== $name"
    $output[($start + 1)..($output.Count - 1)]
}
