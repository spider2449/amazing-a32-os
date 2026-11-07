# Copies a small file to the phone through the serial console, as base64 lines,
# then prints its SHA-256 on both sides. For scripts and other small files; use
# `recv` (see install-alpine.ps1) for large ones.
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Remote,
    [string]$Port = 'COM3'
)

$ErrorActionPreference = 'Stop'
$bytes = [IO.File]::ReadAllBytes((Resolve-Path $Path))
$text = [Convert]::ToBase64String($bytes)
$lines = @("sh rm -f $Remote.b64")
for ($i = 0; $i -lt $text.Length; $i += 400) {
    $chunk = $text.Substring($i, [Math]::Min(400, $text.Length - $i))
    $lines += "sh echo $chunk >> $Remote.b64"
}
$lines += "sh base64 -d $Remote.b64 > $Remote && rm $Remote.b64 && sha256sum $Remote"
& "$PSScriptRoot/console.ps1" -Port $Port -Lines $lines | Select-Object -Last 1
$hash = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLower()
"local  $hash"
