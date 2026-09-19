param([Parameter(Mandatory=$true)][string]$OutputPath)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path (Split-Path $OutputPath) | Out-Null
$bitmap = [Drawing.Bitmap]::new(64, 64)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$brush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(82, 127, 245))
$pen = [Drawing.Pen]::new([Drawing.Color]::White, 4)
$pen.StartCap = [Drawing.Drawing2D.LineCap]::Round
$pen.EndCap = [Drawing.Drawing2D.LineCap]::Round
$stream = [IO.MemoryStream]::new()
try {
    $graphics.Clear([Drawing.Color]::Transparent)
    $graphics.FillEllipse($brush, 1, 1, 62, 62)
    $heights = @(12, 25, 38, 25, 17)
    for ($i = 0; $i -lt 5; $i++) {
        $x = 16 + $i * 8
        $graphics.DrawLine($pen, $x, (64 - $heights[$i]) / 2, $x, (64 + $heights[$i]) / 2)
    }
    $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray()
    $writer = [IO.BinaryWriter]::new([IO.File]::Create($OutputPath))
    try {
        $writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]1)
        $writer.Write([byte]64); $writer.Write([byte]64); $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([UInt16]1); $writer.Write([UInt16]32); $writer.Write([UInt32]$bytes.Length); $writer.Write([UInt32]22)
        $writer.Write($bytes)
    } finally { $writer.Dispose() }
} finally {
    $stream.Dispose(); $pen.Dispose(); $brush.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
Write-Host "应用图标已生成: $OutputPath"
