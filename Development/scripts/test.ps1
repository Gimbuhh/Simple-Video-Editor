param([switch]$Ui,[switch]$Gpu,[switch]$LegacyParity,[switch]$Extended)
$ErrorActionPreference='Stop'
& (Join-Path $PSScriptRoot 'test-rust.ps1') -LegacyParity:$LegacyParity -Playback
& (Join-Path $PSScriptRoot 'test-rust-media.ps1') -Gpu:$Gpu
if($Ui){& (Join-Path $PSScriptRoot 'test-rust-desktop.ps1') -Extended:$Extended}
