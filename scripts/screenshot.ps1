# Saves what the phone's screen shows as a PNG, to check the panel from the computer.
# It reads the framebuffer over the serial console, so it takes about 7 seconds.
param(
    [Parameter(Mandatory)][string]$OutFile,
    [string]$Port = 'COM3',
    [int]$Width = 480,
    [int]$Height = 854,
    # The first color channel in memory. The A32's framebuffer is RGBA.
    [ValidateSet('RGBA', 'BGRA')][string]$Order = 'RGBA'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$raw = [IO.Path]::GetTempFileName()
Remove-Item $raw
try {
    $size = $Width * $Height * 4
    & (Join-Path $PSScriptRoot 'dump-partition.ps1') -OutFile $raw -Port $Port -Path /dev/fb0 -Size $size | Out-Null
    $pixels = [IO.File]::ReadAllBytes($raw)

    # System.Drawing's 32-bit format is BGRA in memory, so swap the channels if needed.
    if ($Order -eq 'RGBA') {
        Add-Type -TypeDefinition @'
public static class SwapChannels {
    public static void Run(byte[] pixels) {
        for (int at = 0; at + 3 < pixels.Length; at += 4) {
            byte red = pixels[at];
            pixels[at] = pixels[at + 2];
            pixels[at + 2] = red;
            pixels[at + 3] = 255;
        }
    }
}
'@
        [SwapChannels]::Run($pixels)
    }

    $bitmap = New-Object System.Drawing.Bitmap($Width, $Height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $area = New-Object System.Drawing.Rectangle(0, 0, $Width, $Height)
        $data = $bitmap.LockBits($area, 'WriteOnly', $bitmap.PixelFormat)
        [Runtime.InteropServices.Marshal]::Copy($pixels, 0, $data.Scan0, $size)
        $bitmap.UnlockBits($data)
        $target = if ([IO.Path]::IsPathRooted($OutFile)) { $OutFile } else { Join-Path (Get-Location) $OutFile }
        $bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
        "saved $target"
    }
    finally { $bitmap.Dispose() }
}
finally { Remove-Item $raw -ErrorAction SilentlyContinue }
