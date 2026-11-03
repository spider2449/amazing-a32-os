# Boots target/a32-boot.img from RAM again and again, and records whether the init
# started Wi-Fi, and rejoined the saved network if there is one. Flash is not written,
# and only -JoinFailure joins by itself: it tries a network that should not exist,
# with a made-up passphrase.
#
# The console must be running on the phone before this starts. Scan results are
# only counted, so the names of nearby networks do not end up in the records.
param(
    [int]$Count = 1,
    [string]$Port = 'COM3',
    [string]$OutFile = 'target/wifi-boots.csv',
    [int]$StartupSeconds = 90,
    [int]$JoinSeconds = 45,
    [switch]$JoinFailure
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fastboot = Join-Path $root 'phone-tools/platform-tools/fastboot.exe'
$image = Join-Path $root 'target/a32-boot.img'
$serialId = 'USB\VID_0E8D&PID_2007*'
$prompt = 'a32> '
$fakeSsid = 'a32-no-such-network'
$fakePassphrase = 'a32-made-up-passphrase'

function Test-Serial {
    [bool](Get-PnpDevice -PresentOnly -InstanceId $serialId -ErrorAction SilentlyContinue)
}

function Wait-Until([scriptblock]$Condition, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        if (& $Condition) { return $true }
        Start-Sleep -Seconds 1
    }
    return $false
}

function Open-Console {
    $serial = New-Object System.IO.Ports.SerialPort $Port, 115200, 'None', 8, 'One'
    $serial.DtrEnable = $true
    $serial.RtsEnable = $true
    $serial.NewLine = "`n"
    $serial.WriteTimeout = 5000
    $serial.ReadBufferSize = 2MB
    $serial.Open()
    Start-Sleep -Milliseconds 300
    [void]$serial.ReadExisting()
    return $serial
}

# Sends a line and returns its output, without the echoed line and the prompt.
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
    throw "no prompt within $Seconds s after: $($Line.Split(' ')[0])"
}

function Restart-FromRam {
    $serial = Open-Console
    try { $serial.WriteLine('reboot bootloader'); Start-Sleep -Seconds 1 }
    finally { try { $serial.Close() } catch {} }

    if (-not (Wait-Until { & $fastboot devices } 80)) { throw 'fastboot was not reached' }
    & cmd /c "`"$fastboot`" boot `"$image`" 2>&1" | Out-Null
    Start-Sleep -Seconds 8
    if (-not (Wait-Until { Test-Serial } 80)) { throw 'the console did not return' }
    Start-Sleep -Seconds 3
}

function Measure-Boot($Serial) {
    $state = ''
    $deadline = (Get-Date).AddSeconds($StartupSeconds)
    while ((Get-Date) -lt $deadline) {
        $state = Send-Line $Serial 'cat /run/wifi-state'
        if ($state -notmatch 'starting') { break }
        Start-Sleep -Seconds 2
    }
    $joined = $false
    if ($state -eq 'ready' -and (Send-Line $Serial 'ls /data/a32') -match 'wifi-network') {
        $joined = Wait-Until { (Send-Line $Serial 'wifi') -match 'ip_address=' -and (Send-Line $Serial 'cat /run/init.log') -match 'time: set' } $JoinSeconds
    }
    $init = Send-Line $Serial 'cat /run/init.log'
    $status = Send-Line $Serial 'wifi'
    $started = [regex]::Match($init, 'wifi: started in (\d+) s').Groups[1].Value
    $attempt = [regex]::Match($init, 'after power on attempt (\d+)').Groups[1].Value
    $uptime = ((Send-Line $Serial 'cat /proc/uptime') -split ' ')[0]
    $touch = (Send-Line $Serial 'ls /dev') -match '(?m)^touch\r?$'
    $networks = ''
    if ($state -eq 'ready') {
        $cli = 'wpa_cli -p /run/wpa -i wlan0'
        $networks = Send-Line $Serial "sh $cli scan > /dev/null; sleep 8; $cli scan_results | grep -c '^[0-9a-f][0-9a-f]:'" 20
    }
    [pscustomobject]@{
        Time = Get-Date -Format s
        State = $state
        StartSeconds = $started
        PowerAttempt = $attempt
        Uptime = $uptime
        Networks = $networks
        Joined = $joined
        JoinedAt = [regex]::Match($init, '\[([\d.]+)\] wifi: connected').Groups[1].Value
        ClockSetAt = [regex]::Match($init, '\[([\d.]+)\] time: set').Groups[1].Value
        Mac = [regex]::Match($status, '(?m)^address=(\S+)').Groups[1].Value
        Address = [regex]::Match($status, 'ip_address=(\S+)').Groups[1].Value
        Data = $init -match 'data: mounted'
        Touch = $touch
    }
}

function Test-JoinFailure($Serial) {
    $cli = 'wpa_cli -p /run/wpa -i wlan0'
    $answer = Send-Line $Serial "wifi join $fakeSsid $fakePassphrase" 60
    "join answered: $answer"
    "networks left: " + (Send-Line $Serial "sh $cli list_networks | tail -n +2 | wc -l")
    "wpa_state: " + (Send-Line $Serial "sh $cli status | grep wpa_state")
    # The pattern is split, so that this command cannot match itself anywhere.
    $pattern = $fakePassphrase.Substring(0, 8) + '[-]' + $fakePassphrase.Substring(9)
    "passphrase in /run files: [" + (Send-Line $Serial "sh grep -l -e '$pattern' /run/* 2>/dev/null") + "]"
    "passphrase in kernel log: " + (Send-Line $Serial "sh dmesg | grep -c -e '$pattern'")
    "console still answers: " + (Send-Line $Serial 'cat /run/wifi-state')
}

if (-not (Test-Path $image)) { throw "missing $image" }
if (-not (Test-Serial)) { throw 'the console is not present' }
$outPath = if ([IO.Path]::IsPathRooted($OutFile)) { $OutFile } else { Join-Path $root $OutFile }

for ($boot = 1; $boot -le $Count; $boot++) {
    Restart-FromRam
    $serial = Open-Console
    try {
        $result = Measure-Boot $serial
        $result | Add-Member -NotePropertyName Boot -NotePropertyValue $boot
        $result | Export-Csv -Path $outPath -Append -NoTypeInformation -Encoding ASCII
        $result | Format-List | Out-String | Write-Output
        if ($JoinFailure -and $boot -eq $Count) { Test-JoinFailure $serial }
    }
    finally { try { $serial.Close() } catch {} }
}
