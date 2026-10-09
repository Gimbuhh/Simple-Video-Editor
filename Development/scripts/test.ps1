param([string[]]$Clips = @(), [switch]$Ui, [switch]$Gpu)
$ErrorActionPreference = 'Stop'
$editorRoot = Split-Path -Parent $PSScriptRoot
if (!(Test-Path -LiteralPath (Join-Path $editorRoot '.tools/dotnet/dotnet.exe'))) { throw 'Run Development/scripts/setup.ps1 first to restore the workspace build tools.' }
$env:DOTNET_CLI_HOME = Join-Path $editorRoot '.tools\cli'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$editorArguments = @()
if ($Ui) { $editorArguments += '--ui' }
if ($Gpu) { $editorArguments += '--gpu' }
foreach ($editorClip in $Clips) {
    if (!(Test-Path -LiteralPath $editorClip -PathType Leaf)) { throw "Test recording not found: $editorClip" }
    $editorArguments += (Resolve-Path -LiteralPath $editorClip).Path
}
if ($Ui -and $Clips.Count -eq 1) { throw 'The optional UI gameplay checks require two recordings. See docs/development.md.' }
# Exercise local directory-link support without requiring symbolic-link privileges.
$editorFixtures = Join-Path $editorRoot 'artifacts/verification'
New-Item -ItemType Directory -Path $editorFixtures -Force | Out-Null
$editorLocalLink = Join-Path $editorFixtures 'local-recording-link'
if (!(Test-Path -LiteralPath $editorLocalLink)) { New-Item -ItemType Junction -Path $editorLocalLink -Target $editorFixtures | Out-Null }
& (Join-Path $editorRoot '.tools\dotnet\dotnet.exe') run --project (Join-Path $editorRoot 'tests\SimpleVideoEditor.Tests\SimpleVideoEditor.Tests.csproj') -c Release -- @editorArguments
if ($LASTEXITCODE -ne 0) { throw 'Verification failed' }
