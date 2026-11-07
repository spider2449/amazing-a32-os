# Finds the phone's current address and writes it into the "Host a32" entry of the
# SSH config, so that "ssh a32" keeps working when DHCP hands out another address.
#
# It asks the phone over the serial console first. Without the cable, or when the
# port is busy, it scans this computer's IPv4 networks (/24 only) for port 22 and
# keeps the host whose host key matches the one known_hosts holds for "a32".
param(
    [string]$Port = 'COM3',
    [string]$Config = "$env:USERPROFILE\.ssh\config",
    [string]$KnownHosts = "$env:USERPROFILE\.ssh\known_hosts",
    [string]$Alias = 'a32',
    [switch]$ScanOnly
)

$ErrorActionPreference = 'Stop'

function Get-SerialAddress {
    $prompt = 'a32> '
    $serial = New-Object System.IO.Ports.SerialPort $Port, 115200, 'None', 8, 'One'
    $serial.DtrEnable = $true
    $serial.RtsEnable = $true
    $serial.NewLine = "`n"
    $serial.Open()
    try {
        Start-Sleep -Milliseconds 300
        [void]$serial.ReadExisting()
        $serial.WriteLine('wifi')
        $text = ''
        $deadline = (Get-Date).AddSeconds(5)
        while ((Get-Date) -lt $deadline -and -not $text.EndsWith($prompt)) {
            Start-Sleep -Milliseconds 200
            $text += $serial.ReadExisting()
        }
        return [regex]::Match($text, 'ip_address=(\d+\.\d+\.\d+\.\d+)').Groups[1].Value
    }
    finally { $serial.Close() }
}

# Whether the SSH server at an address shows the host key stored for the alias.
# It asks ssh, not ssh-keyscan, because the ssh-keyscan that ships with Windows
# (9.5) cannot agree on a key exchange with the phone's OpenSSH 10.3. No login is
# tried: "none" authentication fails right after the host key check. Through cmd,
# because PowerShell 5.1 turns native stderr into an error under 'Stop'.
function Test-HostKey([string]$Address) {
    $output = cmd /c "ssh -v -o BatchMode=yes -o ConnectTimeout=3 -o StrictHostKeyChecking=yes -o HostKeyAlias=$Alias -o PreferredAuthentications=none -o `"UserKnownHostsFile=$KnownHosts`" root@$Address exit 2>&1"
    return [bool]($output | Select-String -SimpleMatch "Host '$Alias' is known and matches")
}

function Find-ByScan {
    $prefixes = Get-NetIPAddress -AddressFamily IPv4 |
        Where-Object { $_.PrefixOrigin -ne 'WellKnown' -and $_.IPAddress -notlike '169.254.*' -and $_.IPAddress -ne '127.0.0.1' } |
        ForEach-Object { ($_.IPAddress -split '\.')[0..2] -join '.' } | Sort-Object -Unique
    foreach ($prefix in $prefixes) {
        "scanning $prefix.0/24 for port 22" | Write-Host
        # Start all connects at once, then collect those that finished in time.
        $tries = 1..254 | ForEach-Object {
            $client = New-Object System.Net.Sockets.TcpClient
            [pscustomobject]@{ Address = "$prefix.$_"; Client = $client; Task = $client.ConnectAsync("$prefix.$_", 22) }
        }
        Start-Sleep -Milliseconds 1500
        $open = $tries | Where-Object { $_.Task.Status -eq 'RanToCompletion' } | ForEach-Object { $_.Address }
        $tries | ForEach-Object { $_.Client.Dispose() }
        foreach ($address in $open) {
            if (Test-HostKey $address) { return $address }
        }
    }
    return ''
}

$address = ''
if (-not $ScanOnly) {
    try { $address = Get-SerialAddress } catch { Write-Host "serial console not usable: $($_.Exception.Message)" }
    if ($address) { Write-Host "serial console says $address" }
}
if (-not $address) { $address = Find-ByScan }
if (-not $address) { throw 'the phone was not found' }

# Rewrite only the HostName line inside the alias's Host block.
if (-not (Test-Path $Config)) { throw "no SSH config at $Config; it needs a 'Host $Alias' entry" }
$lines = [System.Collections.Generic.List[string]](Get-Content $Config)
$inBlock = $false
$changed = $false
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^\s*Host\s+(.+)$') { $inBlock = ($Matches[1] -split '\s+') -contains $Alias; continue }
    if ($inBlock -and $lines[$i] -match '^(\s*)HostName\s+(\S+)') {
        if ($Matches[2] -ne $address) { $lines[$i] = "$($Matches[1])HostName $address"; $changed = $true }
        $inBlock = $false
    }
}
if ($changed) {
    [IO.File]::WriteAllLines($Config, $lines)
    "Host $Alias now points at $address"
} else {
    "Host $Alias already points at $address"
}
exit 0
