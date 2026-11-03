# Copies the phone's small partitions to the PC, which include calibration data that
# cannot be restored from firmware. The partition table comes from the phone, so this
# also works on other devices. It only creates read-only device nodes in the tmpfs
# /dev and reads from them. Finished files are skipped, and an interrupted one resumes.
#
# Backups stay out of git, because they hold data unique to the device.
param(
    [string]$Port = 'COM3',
    [string]$OutDir = 'phone-tools/backup/partitions',
    # Too large to copy over serial, or restored from firmware or earlier backups.
    [string[]]$Skip = @('system', 'userdata', 'cache', 'boot', 'recovery')
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$directory = if ([IO.Path]::IsPathRooted($OutDir)) { $OutDir } else { Join-Path $root $OutDir }
New-Item -ItemType Directory -Force -Path $directory | Out-Null
$prompt = 'a32> '

function Send-Line($Serial, [string]$Line, [int]$Seconds = 15) {
    $Serial.WriteLine($Line)
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

$serial = New-Object System.IO.Ports.SerialPort $Port, 115200, 'None', 8, 'One'
$serial.DtrEnable = $true
$serial.RtsEnable = $true
$serial.NewLine = "`n"
$serial.WriteTimeout = 5000
$serial.Open()
$partitions = @()
try {
    Start-Sleep -Milliseconds 300
    [void]$serial.ReadExisting()
    $table = Send-Line $serial 'sh for d in /sys/class/block/mmcblk0p*; do . $d/uevent; echo "$PARTNAME $MAJOR $MINOR $(cat $d/size)"; done' 20
    foreach ($line in $table -split "`n") {
        $name, $major, $minor, $sectors = $line.Trim() -split ' '
        if (-not $sectors) { throw "bad partition line: $line" }
        if ($Skip -contains $name) { continue }
        $node = "/dev/backup-$name"
        [void](Send-Line $serial "sh [ -e $node ] || mknod -m 400 $node b $major $minor")
        $partitions += [pscustomobject]@{ Name = $name; Node = $node; Size = [long]$sectors * 512 }
    }
}
finally { $serial.Close() }

$total = ($partitions | Measure-Object -Property Size -Sum).Sum
"copying $($partitions.Count) partitions, $total bytes"
$manifest = foreach ($partition in $partitions | Sort-Object Size) {
    $file = Join-Path $directory "$($partition.Name).img"
    $done = (Test-Path $file) -and ((Get-Item $file).Length -eq $partition.Size)
    if (-not $done) {
        "$(Get-Date -Format s) $($partition.Name) $($partition.Size) bytes" | Write-Host
        & (Join-Path $PSScriptRoot 'dump-partition.ps1') -OutFile $file -Port $Port `
            -Path $partition.Node -Size $partition.Size | Select-Object -Last 1 | Write-Host
    }
    if ((Get-Item $file).Length -ne $partition.Size) { throw "wrong size: $file" }
    '{0}  {1}.img' -f (Get-FileHash $file -Algorithm SHA256).Hash.ToLower(), $partition.Name
}
[IO.File]::WriteAllLines((Join-Path $directory 'SHA256SUMS'), [string[]]$manifest)
$manifest
"done: $($partitions.Count) partitions in $directory"
