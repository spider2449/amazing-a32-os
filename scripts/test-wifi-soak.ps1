# Records whether a joined Wi-Fi network keeps working over time. The phone must
# already be on the network. This never joins, rejoins or restarts anything, so a
# lost connection stays visible in the results. A row is added after each check, so
# an interrupted run keeps what it measured. The network name is not recorded.
#
# USB stays attached for the serial console, so this does not measure battery life.
param(
    [double]$Hours = 4,
    [int]$IntervalSeconds = 60,
    [string]$Port = 'COM3',
    [string]$OutFile = 'target/wifi-soak.csv',
    [string]$TcpTarget = '8.8.8.8:443',
    [string]$PingTarget = '8.8.8.8'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$prompt = 'a32> '
$outPath = if ([IO.Path]::IsPathRooted($OutFile)) { $OutFile } else { Join-Path $root $OutFile }
$cli = 'wpa_cli -p /run/wpa -i wlan0'

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
    throw "no prompt within $Seconds s"
}

function Get-Received([string]$Text) {
    [regex]::Match($Text, '(\d+) received').Groups[1].Value
}

function Measure-Network($Serial) {
    $gateway = Send-Line $Serial "sh ip route | grep default | cut -d' ' -f3"
    $gatewayPing = ''
    if ($gateway) {
        $gatewayPing = Get-Received (Send-Line $Serial "sh ping -c 3 -W 2 $gateway 2>&1 | grep transmitted" 15)
    }
    $publicPing = Send-Line $Serial "sh ping -c 3 -W 2 $PingTarget 2>&1 | grep -E 'transmitted|rtt'" 15
    $tcp = Send-Line $Serial "tcp $TcpTarget" 15
    $state = Send-Line $Serial "sh $cli status | grep wpa_state | cut -d= -f2"
    $signal = Send-Line $Serial "sh $cli signal_poll | grep RSSI | cut -d= -f2"
    [pscustomobject]@{
        Time = Get-Date -Format s
        Console = 'ok'
        Uptime = ((Send-Line $Serial 'cat /proc/uptime') -split ' ')[0]
        WpaState = $state
        Rssi = $signal
        HasAddress = [bool](Send-Line $Serial "sh ip addr show wlan0 | grep 'inet '")
        GatewayReceived = $gatewayPing
        PublicReceived = Get-Received $publicPing
        PublicAverageMs = [regex]::Match($publicPing, 'rtt [^=]*= [\d.]+/([\d.]+)/').Groups[1].Value
        TcpMs = [regex]::Match($tcp, 'connected in (\d+) ms').Groups[1].Value
        Battery = Send-Line $Serial 'cat /sys/class/power_supply/battery/capacity'
        BatteryStatus = Send-Line $Serial 'cat /sys/class/power_supply/battery/status'
        FreeKb = [regex]::Match((Send-Line $Serial 'cat /proc/meminfo'), 'MemFree:\s+(\d+)').Groups[1].Value
    }
}

$end = (Get-Date).AddHours($Hours)
$serial = $null
while ((Get-Date) -lt $end) {
    $started = Get-Date
    try {
        if (-not $serial -or -not $serial.IsOpen) { $serial = Open-Console }
        $row = Measure-Network $serial
    }
    catch {
        $row = [pscustomobject]@{
            Time = Get-Date -Format s
            Console = "lost: $($_.Exception.Message)"
        }
        if ($serial) { try { $serial.Close() } catch {} }
        $serial = $null
    }
    $row | Export-Csv -Path $outPath -Append -NoTypeInformation -Encoding ASCII -Force
    $elapsed = ((Get-Date) - $started).TotalSeconds
    if ($IntervalSeconds -gt $elapsed) { Start-Sleep -Seconds ([int]($IntervalSeconds - $elapsed)) }
}
if ($serial) { try { $serial.Close() } catch {} }
