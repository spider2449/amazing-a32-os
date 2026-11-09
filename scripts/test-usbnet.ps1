# Checks the USB network from the PC after `usbnet on`: the driver, the DHCP lease,
# ping, SSH and throughput, and that the PC's default route was not changed.
# It does not turn the network on or off. Prints PASS or FAIL for each check.
param(
    [int]$LeaseWaitSeconds = 60,
    [int]$SshWaitSeconds = 40,
    [int]$MegaBytes = 32,
    [string]$Phone = '192.168.42.1',
    [string]$Computer = '192.168.42.2',
    [string]$Key = "$HOME\.ssh\a32"
)

$failed = 0
function Report([string]$Name, [bool]$Ok, [string]$Detail) {
    $word = if ($Ok) { 'PASS' } else { 'FAIL' }
    "$word  $Name  $Detail"
    if (-not $Ok) { $script:failed++ }
}

$before = (Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
    Sort-Object ifIndex | ForEach-Object { "$($_.ifIndex)>$($_.NextHop)" }) -join ','

$serial = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -match 'VID_0E8D&PID_2099&MI_02' }
Report 'serial port kept' ([bool]$serial) "$($serial.FriendlyName)"

$adapter = Get-NetAdapter | Where-Object { $_.InterfaceDescription -match 'Remote NDIS' -and $_.Status -eq 'Up' } |
    Select-Object -First 1
Report 'network adapter' ([bool]$adapter) "$($adapter.Name)"
if (-not $adapter) { "failed checks: $failed"; exit 1 }

$deadline = (Get-Date).AddSeconds($LeaseWaitSeconds)
$started = Get-Date
do {
    $address = Get-NetIPAddress -InterfaceIndex $adapter.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object { $_.IPAddress -eq $Computer }
    if ($address) { break }
    Start-Sleep -Seconds 1
} while ((Get-Date) -lt $deadline)
Report 'dhcp lease' ([bool]$address) "$Computer after $([int]((Get-Date) - $started).TotalSeconds) s, origin $($address.PrefixOrigin)"

$replies = @(1..6 | ForEach-Object { Test-Connection -ComputerName $Phone -Count 1 -Quiet -ErrorAction SilentlyContinue }) -eq $true
Report 'ping' ($replies.Count -ge 5) "$($replies.Count) of 6"

$options = "-o HostKeyAlias=a32 -o ConnectTimeout=8 -o BatchMode=yes -i `"$Key`""
# Right after a restart sshd may not be running yet, which refuses the connection.
$started = Get-Date
do {
    $name = cmd /c "ssh $options root@$Phone hostname 2>&1"
    $code = $LASTEXITCODE
    if ($code -eq 0) { break }
    Start-Sleep -Seconds 2
} while ((Get-Date) -lt $started.AddSeconds($SshWaitSeconds))
Report 'ssh' ($code -eq 0) "$name, after $([int]((Get-Date) - $started).TotalSeconds) s"

$watch = [Diagnostics.Stopwatch]::StartNew()
cmd /c "ssh $options root@$Phone dd if=/dev/zero bs=1M count=$MegaBytes 2>NUL >NUL"
$code = $LASTEXITCODE
$watch.Stop()
Report 'download' ($code -eq 0) ("{0} MiB in {1:N1} s, {2:N1} MiB/s" -f $MegaBytes, $watch.Elapsed.TotalSeconds, ($MegaBytes / $watch.Elapsed.TotalSeconds))

$after = (Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
    Sort-Object ifIndex | ForEach-Object { "$($_.ifIndex)>$($_.NextHop)" }) -join ','
Report 'default route unchanged' ($before -eq $after -and $after -notmatch [regex]::Escape($Phone)) $after

"failed checks: $failed"
exit $failed
