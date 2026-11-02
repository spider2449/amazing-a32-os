# Copies a file or partition from the phone to the PC over the serial console.
# It only sends the read-only `dump` command. An interrupted copy resumes from
# the length of the output file.
param(
    [Parameter(Mandatory)][string]$OutFile,
    [string]$Port = 'COM3',
    [string]$Path = '/dev/boot',
    [long]$Size = 16MB,
    [int]$ChunkSize = 64KB,
    [int]$Retries = 5
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
public static class Crc32 {
    public static uint Compute(byte[] data) {
        uint crc = 0xffffffff;
        foreach (byte value in data) {
            crc ^= value;
            for (int bit = 0; bit < 8; bit++) {
                crc = (crc >> 1) ^ (0xedb88320 & (uint)(-(int)(crc & 1)));
            }
        }
        return ~crc;
    }
}
'@

function Read-Chunk($serial, [long]$offset, [int]$length) {
    [void]$serial.ReadExisting()
    $serial.WriteLine("dump $Path $offset $length")

    # Skip the terminal's echo of the command and any older output.
    do {
        $line = $serial.ReadLine().Trim()
        if ($line -cmatch '^error:') { throw "phone: $line" }
    } until ($line -cmatch '^DUMP (\d+) ([0-9a-f]{8})$')
    $expectedLength = [int]$Matches[1]
    $expectedCrc = [Convert]::ToUInt32($Matches[2], 16)

    $data = [Convert]::FromBase64String($serial.ReadLine().Trim())
    if ($serial.ReadLine().Trim() -cne 'END') { throw 'missing end marker' }
    if ($data.Length -ne $expectedLength) { throw "got $($data.Length) of $expectedLength bytes" }
    if ([Crc32]::Compute($data) -ne $expectedCrc) { throw 'CRC mismatch' }
    if ($data.Length -ne $length) { throw "short read: $($data.Length) of $length bytes" }
    return , $data
}

$serial = New-Object System.IO.Ports.SerialPort $Port, 115200, 'None', 8, 'One'
$serial.DtrEnable = $true
$serial.RtsEnable = $true
$serial.NewLine = "`n"
$serial.ReadTimeout = 120000
$serial.WriteTimeout = 3000
$serial.ReadBufferSize = 1MB
$serial.Open()

$output = [System.IO.File]::Open($OutFile, 'OpenOrCreate', 'Write')
try {
    [void]$output.Seek(0, 'End')
    $start = $output.Position
    $clock = [System.Diagnostics.Stopwatch]::StartNew()

    while ($output.Position -lt $Size) {
        $length = [int][Math]::Min($ChunkSize, $Size - $output.Position)
        for ($attempt = 1; ; $attempt++) {
            try {
                $data = Read-Chunk $serial $output.Position $length
                break
            } catch {
                if ($attempt -ge $Retries) { throw }
                Write-Warning "offset $($output.Position), attempt ${attempt}: $_"
                Start-Sleep -Seconds 2
            }
        }
        $output.Write($data, 0, $data.Length)
        $output.Flush()

        $copied = $output.Position - $start
        $rate = $copied / 1KB / [Math]::Max($clock.Elapsed.TotalSeconds, 0.001)
        '{0} {1}/{2} bytes, {3:N1} KiB/s' -f (Get-Date -Format s), $output.Position, $Size, $rate
    }
    "DONE: $($output.Position) bytes"
} finally {
    $output.Close()
    $serial.Close()
}
