param([switch]$BuildTools)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
if ((Split-Path -Leaf $editorDevelopment) -ne 'Development' -or !(Test-Path -LiteralPath (Join-Path $editorDevelopment 'src/SimpleVideoEditor/SimpleVideoEditor.csproj'))) { throw 'Run this script from the organized project.' }
$editorDependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorApp = Join-Path $editorWorkspace 'App'
if (Test-Path -LiteralPath (Join-Path $editorApp 'Support')) { $editorApp = Join-Path $editorApp 'Support' }
if ((Get-FileHash -LiteralPath (Join-Path $editorApp 'libmpv-2.dll') -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) { throw 'Keep vendor dependencies until the App bundle is complete.' }
foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
    if ((Get-FileHash -LiteralPath (Join-Path $editorApp "tools/$($editorFile.Name)") -Algorithm SHA256).Hash -ne $editorFile.Value) { throw 'Keep vendor dependencies until the App bundle is verified.' }
}
foreach ($editorLicense in @('mpv-LGPL.txt','ffmpeg-GPL.txt')) { if (!(Test-Path -LiteralPath (Join-Path $editorApp "licenses/$editorLicense"))) { throw 'App licenses are missing.' } }
$editorSdk = Join-Path $editorDevelopment '.tools/dotnet/dotnet.exe'
if (Test-Path -LiteralPath $editorSdk) { & $editorSdk build-server shutdown; if ($LASTEXITCODE -ne 0) { throw 'Could not stop workspace build servers.' } }
$editorTargets = @('src/SimpleVideoEditor/bin','src/SimpleVideoEditor/obj','tests/SimpleVideoEditor.Tests/bin','tests/SimpleVideoEditor.Tests/obj','vendor')
if ($BuildTools) { $editorTargets += '.tools' }
foreach ($editorRelative in $editorTargets) {
    $editorTarget = [IO.Path]::GetFullPath((Join-Path $editorDevelopment $editorRelative))
    if (!$editorTarget.StartsWith($editorDevelopment + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Cleanup target escapes Development.' }
    if (Test-Path -LiteralPath $editorTarget) { Remove-Item -LiteralPath $editorTarget -Recurse -Force; Write-Output "Removed reproducible development files: $editorRelative" }
}
if ($BuildTools) { Write-Output 'The app still runs without build tools. Run Development/scripts/setup.ps1 before rebuilding.' }
