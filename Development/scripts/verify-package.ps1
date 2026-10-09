param([string]$OutputName = 'App')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
if ($OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$') { throw 'Invalid app output name.' }
$editorBundle = Join-Path $editorWorkspace $OutputName
$editorExpected = @('README.txt','Simple Video Editor.exe','Support')
$editorActual = @(Get-ChildItem -LiteralPath $editorBundle | Sort-Object Name | Select-Object -ExpandProperty Name)
if (@(Compare-Object $editorExpected $editorActual).Count) { throw 'The portable folder must expose only the executable, README.txt, and Support.' }
$editorVersion = ([xml](Get-Content (Join-Path $editorDevelopment 'src/SimpleVideoEditor/SimpleVideoEditor.csproj') -Raw)).Project.PropertyGroup.Version
if ([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $editorBundle 'Simple Video Editor.exe')).FileVersion -ne "$editorVersion.0") { throw 'Packaged executable version mismatch.' }
$editorZip = Join-Path $editorWorkspace 'Releases/SimpleVideoEditor-win-x64.zip'
$editorHash = (Get-FileHash -LiteralPath $editorZip -Algorithm SHA256).Hash.ToLowerInvariant()
if ((Get-Content -LiteralPath ($editorZip + '.sha256') -Raw).Trim() -ne "$editorHash  SimpleVideoEditor-win-x64.zip") { throw 'Portable checksum mismatch.' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$editorArchive = [IO.Compression.ZipFile]::OpenRead($editorZip)
try {
    $editorZipRoot = @($editorArchive.Entries | ForEach-Object { ($_.FullName.Replace('\','/') -split '/')[0] } | Sort-Object -Unique)
    if (@(Compare-Object $editorExpected $editorZipRoot).Count) { throw 'The release ZIP has unexpected top-level entries.' }
} finally { $editorArchive.Dispose() }
$editorArtifacts = Join-Path $editorDevelopment 'artifacts/package-verification'
$editorRelocated = Join-Path $editorArtifacts ("relocated café's editor " + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorRelocated -Force | Out-Null
$editorProcess = $null
try {
    [IO.Compression.ZipFile]::ExtractToDirectory($editorZip, $editorRelocated)
    $editorSupport = Join-Path $editorRelocated 'Support'
    $editorDependencies = Get-Content (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
    if ((Get-FileHash (Join-Path $editorSupport 'libmpv-2.dll') -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) { throw 'Relocated playback dependency mismatch.' }
    foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
        if ((Get-FileHash (Join-Path $editorSupport "tools/$($editorFile.Name)") -Algorithm SHA256).Hash -ne $editorFile.Value) { throw "Relocated dependency mismatch: $($editorFile.Name)" }
    }
    foreach ($editorLicense in @('mpv-LGPL.txt','ffmpeg-GPL.txt','dotnet-LICENSE.txt','dotnet-THIRD-PARTY-NOTICES.txt','WPF-LICENSE.txt','WPF-THIRD-PARTY-NOTICES.txt')) {
        if (!(Test-Path -LiteralPath (Join-Path $editorSupport "licenses/$editorLicense"))) { throw "Missing packaged license: $editorLicense" }
    }
    $editorProject = Join-Path $editorDevelopment 'artifacts/verification/roundtrip.sveproject'
    if (!(Test-Path -LiteralPath $editorProject)) { throw 'Run the core/media checks first to generate the portable startup fixture.' }
    $editorReadyFile = Join-Path $editorRelocated 'verification.json'
    $editorProcess = Start-Process -FilePath (Join-Path $editorRelocated 'Simple Video Editor.exe') -ArgumentList ('--verify-package "' + $editorProject + '" "' + $editorReadyFile + '"') -WorkingDirectory $editorRelocated -WindowStyle Hidden -PassThru
    if (!$editorProcess.WaitForExit(25000) -or $editorProcess.ExitCode -ne 0 -or !(Test-Path -LiteralPath $editorReadyFile)) { throw 'The relocated editor did not render its saved project and close cleanly.' }
    $editorReady = Get-Content -LiteralPath $editorReadyFile -Raw | ConvertFrom-Json
    if ($editorReady.PreviewWidth -le 0 -or $editorReady.PreviewHeight -le 0 -or $editorReady.PlaybackLibrary -ne (Join-Path $editorSupport 'libmpv-2.dll') -or $editorReady.ProbeTool -ne (Join-Path $editorSupport 'tools/ffprobe.exe')) { throw 'The relocated preview did not use its own bundled video tools.' }
    @{version=$editorVersion; rootEntries=$editorActual; zipSha256=$editorHash; nativePins=$true; relocation=$true; savedProjectStartup=$true; bundledPlayback=$true; cleanClose=$true} | ConvertTo-Json | Set-Content (Join-Path $editorArtifacts 'results.json')
    Write-Output 'Portable layout, checksum, native pins, Unicode/space relocation, saved-project preview rendering, bundled playback, and clean close passed.'
} finally {
    if ($editorProcess -and !$editorProcess.HasExited) { Stop-Process -Id $editorProcess.Id }
    $editorCleanup = [IO.Path]::GetFullPath($editorRelocated)
    if (!$editorCleanup.StartsWith([IO.Path]::GetFullPath($editorArtifacts) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Package verification cleanup escapes its artifacts folder.' }
    if (Test-Path -LiteralPath $editorCleanup) { Remove-Item -LiteralPath $editorCleanup -Recurse -Force }
}
