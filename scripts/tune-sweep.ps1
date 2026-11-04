# Measures one or more runtime settings against each other, and appends to a CSV. The
# settings are only made through /sys and /proc, so a restart undoes them. The phone
# must be on a network this computer can reach. Each measurement is repeated, because
# eMMC writes vary a lot from run to run.
param(
    [string]$Port = 'COM3',
    [string]$OutFile = 'benchmarks/amazing-a32/tuning.csv',
    [int]$Repeats = 3,
    [int]$NetPort = 5201,
    [int]$NetSeconds = 8,
    [int]$PingCount = 15,
    [int]$IdleSeconds = 20,
    [string[]]$Only = @(),
    # Runs the settings in turn this many times, so that their order cannot favor one.
    [int]$Rounds = 1
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outPath = if ([IO.Path]::IsPathRooted($OutFile)) { $OutFile } else { Join-Path $root $OutFile }
$prompt = 'a32> '

$governor = '/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor'
$baseCpus = '/proc/hps/num_base_perf_serv'
$scheduler = '/sys/block/mmcblk0/queue/scheduler'
$ioBusy = '/sys/devices/system/cpu/cpufreq/interactive/io_is_busy'
# The first setting of each group is the system's default, which restores the group.
$settings = [ordered]@{
    'default'             = ''
    'interactive-io-busy' = "echo interactive > $governor; echo 1 > $ioBusy"
    'interactive-io-idle' = "echo interactive > $governor; echo 0 > $ioBusy"
    'governor-performance' = "echo performance > $governor"
    'governor-ondemand'    = "echo ondemand > $governor"
    'governor-interactive' = "echo interactive > $governor"
    'cpus-4-online'        = "echo 4 > $baseCpus"
    'cpus-1-online'        = "echo 1 > $baseCpus"
    'io-deadline'          = "echo deadline > $scheduler"
    'io-noop'              = "echo noop > $scheduler"
    'io-cfq'               = "echo cfq > $scheduler"
}

function Send-Line($Serial, [string]$Line, [int]$Seconds = 30) {
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

function Add-Row([string]$Setting, [string]$Metric, [string]$Value, [string]$Unit) {
    [pscustomobject]@{
        Time = Get-Date -Format s; Setting = $Setting; Metric = $Metric; Value = $Value; Unit = $Unit
    } | Export-Csv -Path $outPath -Append -NoTypeInformation -Encoding ASCII
    "  $Metric = $Value $Unit"
}

function Get-Value([string]$Text, [string]$Name) {
    [regex]::Match($Text, "(?m)^$([regex]::Escape($Name)) .*?value=([\d.]+)").Groups[1].Value
}

function Measure-Net($Serial, [string]$Address, [string]$Setting) {
    $Serial.WriteLine("bench net $NetPort $NetSeconds")
    Start-Sleep -Seconds 1
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $client.Connect($Address, $NetPort)
        $stream = $client.GetStream()
        $buffer = New-Object byte[] 65536
        (New-Object Random 1).NextBytes($buffer)
        $clock = [Diagnostics.Stopwatch]::StartNew()
        while ($clock.Elapsed.TotalSeconds -lt $NetSeconds) { $stream.Write($buffer, 0, $buffer.Length) }
        $client.Client.Shutdown([Net.Sockets.SocketShutdown]::Send)
        while ($stream.Read($buffer, 0, $buffer.Length) -gt 0) {}
    }
    finally { $client.Close() }
    $text = Read-Output $Serial ($NetSeconds + 30)
    Add-Row $Setting 'net.receive' (Get-Value $text 'net.receive') 'Mbit/s'
    Add-Row $Setting 'net.send' (Get-Value $text 'net.send') 'Mbit/s'
}

function Measure-Ping([string]$Address, [string]$Setting) {
    $times = foreach ($line in (& ping.exe -n $PingCount $Address)) {
        if ($line -match 'time[=<](\d+)ms') { [int]$Matches[1] }
    }
    $sorted = @($times | Sort-Object)
    if ($sorted.Count -eq 0) { Add-Row $Setting 'ping.answered' 0 "of $PingCount"; return }
    Add-Row $Setting 'ping.answered' $sorted.Count "of $PingCount"
    Add-Row $Setting 'ping.median' $sorted[[int]($sorted.Count / 2)] 'ms'
    Add-Row $Setting 'ping.max' $sorted[-1] 'ms'
}

# The share of CPU time that was not idle, from /proc/stat.
function Measure-Idle($Serial, [string]$Setting) {
    $read = {
        $fields = ((Send-Line $Serial 'sh head -n 1 /proc/stat') -split '\s+')[1..7] | ForEach-Object { [long]$_ }
        , $fields
    }
    $before = & $read
    Start-Sleep -Seconds $IdleSeconds
    $after = & $read
    $total = 0; $busy = 0
    for ($i = 0; $i -lt 7; $i++) {
        $change = $after[$i] - $before[$i]
        $total += $change
        if ($i -ne 3 -and $i -ne 4) { $busy += $change }
    }
    Add-Row $Setting 'idle.busy' ([math]::Round(100 * $busy / [math]::Max($total, 1), 1)) '%'
    $online = Send-Line $Serial 'cat /sys/devices/system/cpu/online'
    Add-Row $Setting 'idle.online' $online 'cpus'
    $thermal = Send-Line $Serial 'cat /run/thermal-state'
    Add-Row $Setting 'idle.cpu_temperature' ([regex]::Match($thermal, 'cpu: Some\((\d+)\)').Groups[1].Value) 'millidegrees C'
}

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
    $address = [regex]::Match((Send-Line $serial 'wifi'), 'ip_address=(\S+)').Groups[1].Value
    if (-not $address) { throw 'the phone has no address' }

    # A comma list arrives as one string through powershell -File, so split it. A name
    # that is not known must stop the run: it would measure the wrong thing in silence.
    $names = if ($Only.Count) { @($Only | ForEach-Object { $_ -split ',' }) } else { @($settings.Keys) }
    foreach ($name in $names) {
        if (-not $settings.Contains($name)) { throw "unknown setting: $name" }
    }
    foreach ($name in (1..$Rounds | ForEach-Object { $names })) {
        "=== $name"
        if ($settings[$name]) {
            $failed = Send-Line $serial "sh $($settings[$name]); echo rc=`$?"
            if ($failed -notmatch 'rc=0') { "  could not apply: $failed"; continue }
        }
        $state = Send-Line $serial "sh cat $governor; cat $ioBusy 2>/dev/null; cat $baseCpus; cat $scheduler"
        Add-Row $name 'state' (($state -split "`n") -join ' / ') ''
        Measure-Idle $serial $name
        Measure-Ping $address $name
        # The default also restores a group, so it gets a shorter pass where nothing changed.
        for ($run = 1; $run -le $Repeats; $run++) {
            Measure-Net $serial $address $name
            $text = Send-Line $serial 'bench disk /dev/system' 40
            Add-Row $name 'disk.read_sequential' (Get-Value $text 'disk.read_sequential') 'MB/s'
            Add-Row $name 'disk.read_random' (Get-Value $text 'disk.read_random') 'IOPS'
            $text = Send-Line $serial 'bench write /data/a32/bench.tmp' 60
            Add-Row $name 'disk.write_sequential' (Get-Value $text 'disk.write_sequential') 'MB/s'
            Add-Row $name 'disk.write_random' (Get-Value $text 'disk.write_random') 'IOPS'
        }
    }
    "--- state afterwards: " + (Send-Line $serial "sh cat $governor $baseCpus $scheduler | tr '\n' ' '")
}
finally { $serial.Close() }
