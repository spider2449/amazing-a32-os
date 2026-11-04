# Installs Alpine Linux into /data/alpine on the phone, with SSH that accepts one
# public key and no passwords. The phone must be running this system on a network
# that this computer can reach, because apk downloads packages over the network.
#
# Check the archive first: compare its SHA-256 with the release, and verify its GPG
# signature. The phone can then only be sent that file, because this reads it back
# over the serial console and compares the hash before unpacking anything.
param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$PublicKey = "$env:USERPROFILE\.ssh\a32.pub",
    [string]$Port = 'COM3',
    [int]$TransferPort = 5202
)

$ErrorActionPreference = 'Stop'
$prompt = 'a32> '
$remote = '/data/alpine-rootfs.tar'

function Send-Line($Serial, [string]$Line, [int]$Seconds = 15) {
    $Serial.WriteLine($Line)
    Read-Output $Serial $Seconds
}

function Read-Output($Serial, [int]$Seconds) {
    $text = ''
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
        $text += $Serial.ReadExisting()
        if ($text.EndsWith($prompt)) {
            $lines = $text.Substring(0, $text.Length - $prompt.Length) -split "`r?`n"
            return (($lines | Select-Object -Skip 1) -join "`n").Trim()
        }
    }
    throw "no prompt within $Seconds s"
}

# Sends a line that must end by printing a marker, and stops the install if it does not.
function Send-Step($Serial, [string]$Line, [int]$Seconds = 15) {
    $marker = 'step-ok'
    $output = Send-Line $Serial "$Line && echo $marker" $Seconds
    $output
    if ($output -notmatch "(^|\s)$marker\s*$") { throw "this step failed: $(($Line -split ' ')[0..1] -join ' ')" }
}

$key = (Get-Content $PublicKey -Raw).Trim()
if ($key -notmatch '^ssh-ed25519 [A-Za-z0-9+/=]+( [^\r\n'']*)?$') { throw "not one ed25519 public key: $PublicKey" }
# Unpacked here, because the stock gzip on the phone cannot decompress, and this way
# the install does not depend on what tools a device happens to have.
$tarFile = [IO.Path]::GetTempFileName()
$source = [IO.File]::OpenRead((Resolve-Path $Archive))
try {
    $gzip = New-Object System.IO.Compression.GZipStream($source, [IO.Compression.CompressionMode]::Decompress)
    $output = [IO.File]::Create($tarFile)
    try { $gzip.CopyTo($output) } finally { $output.Close() }
}
finally { $source.Close() }
$size = (Get-Item $tarFile).Length
$hash = (Get-FileHash $tarFile -Algorithm SHA256).Hash

$serial = New-Object System.IO.Ports.SerialPort $Port, 115200, 'None', 8, 'One'
$serial.DtrEnable = $true
$serial.RtsEnable = $true
$serial.NewLine = "`n"
$serial.WriteTimeout = 5000
$serial.ReadBufferSize = 2MB
$serial.Open()
try {
    Start-Sleep -Milliseconds 300
    [void]$serial.ReadExisting()
    if ((Send-Line $serial 'sh ls /data/alpine/bin/sh 2>/dev/null') -match 'bin/sh') { throw 'Alpine is already installed' }
    $address = [regex]::Match((Send-Line $serial 'wifi'), 'ip_address=(\S+)').Groups[1].Value
    if (-not $address) { throw 'the phone has no address, join a network first' }

    "--- sending $size bytes to $address"
    [void](Send-Line $serial "sh rm -f $remote")
    $serial.WriteLine("recv $remote $TransferPort")
    Start-Sleep -Seconds 1
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $client.Connect($address, $TransferPort)
        $file = [IO.File]::OpenRead($tarFile)
        try { $file.CopyTo($client.GetStream()) } finally { $file.Close() }
    }
    finally { $client.Close() }
    Read-Output $serial 60
}
finally { $serial.Close() }

"--- reading it back to compare"
$copy = [IO.Path]::GetTempFileName()
Remove-Item $copy
try {
    & (Join-Path $PSScriptRoot 'dump-partition.ps1') -OutFile $copy -Port $Port -Path $remote -Size $size | Select-Object -Last 1
    $back = (Get-FileHash $copy -Algorithm SHA256).Hash
    if ($back -ne $hash) { throw "the file on the phone differs: $back, not $hash" }
    "matches $hash"
}
finally {
    Remove-Item $copy -ErrorAction SilentlyContinue
    Remove-Item $tarFile -ErrorAction SilentlyContinue
}

$serial.Open()
try {
    Start-Sleep -Milliseconds 300
    [void]$serial.ReadExisting()
    "--- unpacking"
    try {
        Send-Step $serial "sh mkdir -p /data/alpine && tar -xf $remote -C /data/alpine && test -x /data/alpine/bin/busybox && ls /data/alpine | tr '\n' ' '" 120
    }
    catch {
        [void](Send-Line $serial 'sh rm -rf /data/alpine')
        throw
    }
    [void](Send-Line $serial "sh rm -f $remote; sync")
    "--- repositories"
    Send-Step $serial 'alpine cat /etc/apk/repositories'
    "--- installing OpenSSH"
    Send-Step $serial 'alpine apk update && apk add --no-progress openssh-server' 300
    "--- configuring SSH"
    Send-Step $serial "alpine mkdir -p /root/.ssh /etc/ssh/sshd_config.d && printf '%s\n' '$key' > /root/.ssh/authorized_keys && chmod 700 /root/.ssh && chmod 600 /root/.ssh/authorized_keys && printf 'PermitRootLogin prohibit-password\nPasswordAuthentication no\nKbdInteractiveAuthentication no\n' > /etc/ssh/sshd_config.d/a32.conf && grep -q '^Include /etc/ssh/sshd_config.d' /etc/ssh/sshd_config && echo include-ok"
    Send-Step $serial 'alpine ssh-keygen -A' 90
    "--- effective settings"
    Send-Step $serial "alpine /usr/sbin/sshd -T | grep -E '^(permitrootlogin|passwordauthentication|kbdinteractiveauthentication|pubkeyauthentication) '" 15
}
finally { $serial.Close() }
