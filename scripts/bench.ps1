# Runs the console's performance measurements and saves the results. The phone must
# be running an image with the bench command. Storage is only written with
# -WriteFile, which names a new file that the phone removes afterwards.
#
# -Net needs the phone on a network that this computer can reach. For its duration
# the phone listens for one connection on -NetPort. Results hold no network names.
param(
    [Parameter(Mandatory)][string]$Label,
    [string]$Port = 'COM3',
    [string]$OutDir = 'benchmarks/amazing-a32',
    [string]$DiskPath = '/dev/system',
    [string]$WriteFile = '',
    [int]$Runs = 3,
    [int]$LoadSeconds = 0,
    [switch]$Net,
    # Only the network measurement, for comparing settings that affect Wi-Fi.
    [switch]$NetOnly,
    [int]$NetPort = 5201,
    [int]$NetSeconds = 10
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$prompt = 'a32> '

function Send-Line($Serial, [string]$Line, [int]$Seconds) {
    $Serial.WriteLine($Line)
    Read-Output $Serial $Seconds
}

# Returns output up to the next prompt, without the echoed line.
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

# The phone receives first and sends second, so that only an outgoing connection
# from this computer is needed, and its firewall does not have to change.
function Measure-Network($Serial, [string]$Address) {
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
    Read-Output $Serial ($NetSeconds + 30)
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
    $output = New-Object System.Collections.ArrayList
    [void]$output.Add("# label=$Label date=$(Get-Date -Format s)")
    [void]$output.Add("# " + ((Send-Line $serial 'cat /proc/version' 5) -split ' ' | Select-Object -First 3) -join ' ')
    [void]$output.Add("# uptime=" + ((Send-Line $serial 'cat /proc/uptime' 5) -split ' ')[0])

    for ($run = 1; $run -le $Runs; $run++) {
        [void]$output.Add("# run=$run")
        if (-not $NetOnly) {
            [void]$output.Add((Send-Line $serial 'bench cpu' 60))
            [void]$output.Add((Send-Line $serial 'bench mem' 120))
            [void]$output.Add((Send-Line $serial "bench disk $DiskPath" 40))
            if ($WriteFile) { [void]$output.Add((Send-Line $serial "bench write $WriteFile" 60)) }
        }
        if ($Net -or $NetOnly) {
            $status = Send-Line $serial 'wifi' 5
            $address = [regex]::Match($status, 'ip_address=(\S+)').Groups[1].Value
            if (-not $address) { throw 'the phone has no address, join a network first' }
            [void]$output.Add((Measure-Network $serial $address))
        }
    }
    if ($LoadSeconds -gt 0) {
        [void]$output.Add((Send-Line $serial "bench load $LoadSeconds" ($LoadSeconds + 60)))
    }
}
finally { $serial.Close() }

$directory = Join-Path $root $OutDir
New-Item -ItemType Directory -Force -Path $directory | Out-Null
$path = Join-Path $directory "$Label.txt"
$text = (($output -join "`n") -replace "`r", '') + "`n"
[IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false))
$text
"saved $path"
