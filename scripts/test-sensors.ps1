# Prints the phone's own sensors from the status page while the phone is moved by
# hand. Each reading takes about 2 s. Stand it up, lay it on a long edge and cover the
# sensors next to the earpiece, then compare with the "Phone sensors" notes in the README.
param(
    # An IP address: .NET does not resolve a32.local on Windows, though ping and ssh do.
    [Parameter(Mandatory)][string]$Address,
    [int]$Readings = 15
)

$ErrorActionPreference = 'Stop'
$started = Get-Date
foreach ($number in 1..$Readings) {
    $reading = Invoke-RestMethod -Uri "http://$Address/sensors.json" -TimeoutSec 10
    '{0,5:N1} s  x {1,7:N2}  y {2,7:N2} m/s2  light {3,4}  proximity {4,4}  near {5}' -f `
        ((Get-Date) - $started).TotalSeconds, $reading.acceleration.x, $reading.acceleration.y,
        $reading.light_raw, $reading.proximity_raw, $reading.near
}
