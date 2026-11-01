<#
.SYNOPSIS
Inspect or reversibly exclude this editor from NVIDIA's game overlay.
.DESCRIPTION
Opt-in machine configuration, not part of app startup. Uses the per-application
flag found in NVIDIA's built-in OBS/VLC/Blender profiles. NVIDIA does not document
the flag as a supported API contract; driver updates may change its behavior.
Never changes the global profile, game profiles, recording or replay settings.
#>
[CmdletBinding()]
param(
    [ValidateSet('Inspect','Exclude','Restore')][string]$Action='Inspect',
    [string]$AppPath=(Join-Path $PSScriptRoot '../../App/Simple Video Editor.exe')
)
$ErrorActionPreference='Stop'
$Action=@{inspect='Inspect';exclude='Exclude';restore='Restore'}[$Action]
$editorCaptureExe=(Get-Item -LiteralPath $AppPath -ErrorAction Stop).FullName
$editorCaptureVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($editorCaptureExe)
if($editorCaptureVersion.OriginalFilename -cne 'Simple Video Editor.exe' -or $editorCaptureVersion.ProductName -cne 'Simple Video Editor') {
    throw 'Select a packaged Simple Video Editor executable.'
}
if($Action -ne 'Inspect') {
    foreach($editorCaptureProcess in @(Get-Process -Name 'Simple Video Editor' -ErrorAction SilentlyContinue)) {
        if(!$editorCaptureProcess.Path -or [string]::Equals($editorCaptureProcess.Path,$editorCaptureExe,[StringComparison]::OrdinalIgnoreCase)) {
            throw 'Close this editor before changing its NVIDIA profile. No process was stopped.'
        }
    }
}
$editorCaptureHash=[Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($editorCaptureExe.ToUpperInvariant()))
$editorCaptureProfile='Simple Video Editor - capture exclusion - '+[Convert]::ToHexString($editorCaptureHash).Substring(0,16)
if(!('SVE.NvidiaCaptureProfile' -as [type])) {
    Add-Type -Path (Join-Path $PSScriptRoot 'nvidia-editor-profile.cs')
}
[SVE.NvidiaCaptureProfile]::Run($editorCaptureExe,$editorCaptureProfile,$Action) | ConvertFrom-Json
