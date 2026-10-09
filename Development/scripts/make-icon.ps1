$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$editorRoot = Split-Path -Parent $PSScriptRoot
$editorAssets = Join-Path $editorRoot 'src\SimpleVideoEditor\Assets'
$editorImage = [Drawing.Bitmap]::FromFile((Join-Path $editorAssets 'app-icon.png'))
$editorSizes = @(16,20,24,32,40,48,64,96,128,256)
$editorFrames = @()
try {
    foreach ($editorSize in $editorSizes) {
        $editorBitmap = New-Object Drawing.Bitmap($editorSize,$editorSize,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $editorGraphics = [Drawing.Graphics]::FromImage($editorBitmap)
        try {
            $editorGraphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
            $editorGraphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $editorGraphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $editorGraphics.Clear([Drawing.Color]::Transparent)
            $editorGraphics.DrawImage($editorImage, (New-Object Drawing.Rectangle(0,0,$editorSize,$editorSize)))
            $editorBuffer = New-Object IO.MemoryStream
            try { $editorBitmap.Save($editorBuffer,[Drawing.Imaging.ImageFormat]::Png); $editorFrames += ,$editorBuffer.ToArray() }
            finally { $editorBuffer.Dispose() }
        }
        finally { $editorGraphics.Dispose(); $editorBitmap.Dispose() }
    }
    $editorIconPath = Join-Path $editorAssets 'app.ico'
    $editorStream = [IO.File]::Create($editorIconPath)
    $editorWriter = New-Object IO.BinaryWriter($editorStream)
    try {
        $editorWriter.Write([uint16]0); $editorWriter.Write([uint16]1); $editorWriter.Write([uint16]$editorSizes.Count)
        $editorOffset = 6 + 16 * $editorSizes.Count
        for ($editorIndex=0; $editorIndex -lt $editorSizes.Count; $editorIndex++) {
            $editorDimension = $editorSizes[$editorIndex] % 256
            $editorWriter.Write([byte]$editorDimension); $editorWriter.Write([byte]$editorDimension)
            $editorWriter.Write([byte]0); $editorWriter.Write([byte]0)
            $editorWriter.Write([uint16]1); $editorWriter.Write([uint16]32)
            $editorWriter.Write([uint32]$editorFrames[$editorIndex].Length); $editorWriter.Write([uint32]$editorOffset)
            $editorOffset += $editorFrames[$editorIndex].Length
        }
        foreach ($editorFrame in $editorFrames) { $editorWriter.Write([byte[]]$editorFrame) }
    }
    finally { $editorWriter.Dispose(); $editorStream.Dispose() }
    Write-Output "Created icon with sizes: $($editorSizes -join ', ')"
}
finally { $editorImage.Dispose() }
