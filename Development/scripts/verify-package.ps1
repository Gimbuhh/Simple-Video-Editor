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
    $editorProcess = Start-Process -FilePath (Join-Path $editorRelocated 'Simple Video Editor.exe') -ArgumentList ('"' + $editorProject + '"') -WorkingDirectory $editorRelocated -WindowStyle Hidden -PassThru
    $editorLoadedMpv = $false
    for ($editorAttempt = 0; $editorAttempt -lt 75; $editorAttempt++) {
        Start-Sleep -Milliseconds 200
        $editorProcess.Refresh()
        if ($editorProcess.HasExited) { throw 'The relocated editor exited during startup.' }
        $editorLoadedMpv = @($editorProcess.Modules | Where-Object { $_.FileName -eq (Join-Path $editorSupport 'libmpv-2.dll') }).Count -eq 1
        if ($editorLoadedMpv -and $editorProcess.MainWindowHandle -ne 0) { break }
    }
    if (!$editorLoadedMpv -or $editorProcess.MainWindowHandle -eq 0) { throw 'The relocated editor did not open its project and load bundled playback.' }
    # Loading playback can precede the final asynchronous project/recovery work.
    Start-Sleep -Milliseconds 750
    $editorCloseSent = $editorProcess.CloseMainWindow()
    if (!$editorCloseSent -or !$editorProcess.WaitForExit(10000)) { throw "The packaged editor did not close its saved project cleanly (close sent: $editorCloseSent)." }
    @{version=$editorVersion; rootEntries=$editorActual; zipSha256=$editorHash; nativePins=$true; relocation=$true; savedProjectStartup=$true; bundledPlayback=$true; cleanClose=$true} | ConvertTo-Json | Set-Content (Join-Path $editorArtifacts 'results.json')
    Write-Output 'Portable layout, checksum, native pins, Unicode/space relocation, saved-project startup, bundled playback, and clean close passed.'
} finally {
    if ($editorProcess -and !$editorProcess.HasExited) { Stop-Process -Id $editorProcess.Id }
    $editorCleanup = [IO.Path]::GetFullPath($editorRelocated)
    if (!$editorCleanup.StartsWith([IO.Path]::GetFullPath($editorArtifacts) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Package verification cleanup escapes its artifacts folder.' }
    if (Test-Path -LiteralPath $editorCleanup) { Remove-Item -LiteralPath $editorCleanup -Recurse -Force }
}
