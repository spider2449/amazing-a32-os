# Sends lines to the phone's serial console and prints what each one returned.
# Each output is headed by "### LINE". A line that does not return the prompt
# within -Seconds is reported and the next line is still sent.
param(
    [Parameter(Mandatory)][string[]]$Lines,
    [int]$Seconds = 20,
    [string]$Port = 'COM3'
)

$ErrorActionPreference = 'Stop'
$prompt = 'a32> '
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
    foreach ($line in $Lines) {
        $serial.WriteLine($line)
        $text = ''
        $done = $false
        $deadline = (Get-Date).AddSeconds($Seconds)
        while ((Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $text += $serial.ReadExisting()
            if ($text.EndsWith($prompt)) { $done = $true; break }
        }
        "### $line"
        # Drop the echoed line and the prompt.
        $body = ($text -split "`r?`n" | Select-Object -Skip 1) -join "`n"
        if ($done) { $body.Substring(0, $body.Length - $prompt.Length).TrimEnd() }
        else { $body.TrimEnd(); "(no prompt within $Seconds s)" }
    }
}
finally { $serial.Close() }
