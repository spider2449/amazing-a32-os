# Boots an image from RAM without writing to flash: asks the running system for
# fastboot over the serial console, sends the image, then waits for the console of
# the new session. A normal restart afterwards returns to what is flashed.
param(
    [Parameter(Mandatory)][string]$Image,
    [string]$Port = 'COM3',
    [int]$ConsoleWaitSeconds = 90
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fastboot = Join-Path $root 'phone-tools/platform-tools/fastboot.exe'
if (-not (Test-Path $Image)) { throw "no image at $Image" }

function Test-Fastboot { [bool]((& $fastboot devices) -match 'fastboot') }

if (-not (Test-Fastboot)) {
    # The console closes as the phone restarts, so an error here is expected.
    try { & (Join-Path $PSScriptRoot 'console.ps1') -Port $Port -Seconds 3 -Lines 'reboot bootloader' | Out-Null } catch {}
    $deadline = (Get-Date).AddSeconds(60)
    while (-not (Test-Fastboot)) {
        if ((Get-Date) -gt $deadline) { throw 'fastboot did not appear within 60 s' }
        Start-Sleep -Seconds 2
    }
}

# fastboot reports progress on stderr, which Windows PowerShell would turn into an error.
$ErrorActionPreference = 'Continue'
$output = & $fastboot boot $Image 2>&1 | Out-String
$ErrorActionPreference = 'Stop'
if ($LASTEXITCODE -ne 0) { throw "fastboot boot failed: $output" }
$sent = Get-Date

$deadline = $sent.AddSeconds($ConsoleWaitSeconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 3
    try {
        $uptime = & (Join-Path $PSScriptRoot 'console.ps1') -Port $Port -Seconds 5 -Lines 'cat /proc/uptime' 2>$null |
            Select-Object -Last 1
        if ($uptime -match '^\d') {
            "console ready $([int]((Get-Date) - $sent).TotalSeconds) s after sending, uptime $uptime"
            return
        }
    } catch {}
}
throw "no console within $ConsoleWaitSeconds s"
