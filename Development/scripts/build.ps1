param([switch]$Zip, [string]$OutputName = 'App')
$ErrorActionPreference = 'Stop'
$editorRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$editorDependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorSdk = Join-Path $editorRoot '.tools/dotnet/dotnet.exe'
if (!(Test-Path -LiteralPath $editorSdk)) { throw 'Run Development/scripts/setup.ps1 first to restore the workspace build tools.' }
$env:DOTNET_CLI_HOME = Join-Path $editorRoot '.tools/cli'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
if ($OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$') { throw 'OutputName must be App or an App-* folder name at the project root.' }
$editorDestination = [IO.Path]::GetFullPath((Join-Path $editorWorkspace $OutputName))
if ((Split-Path -Parent $editorDestination) -ne [IO.Path]::GetFullPath($editorWorkspace)) { throw 'Build output escapes the workspace.' }
function Assert-EditorOutputStopped {
    if (Get-Process -Name SimpleVideoEditor,'Simple Video Editor' -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($editorDestination + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) }) { throw 'This app bundle is running. Choose a different App-* output folder.' }
}
Assert-EditorOutputStopped
# Stage the complete bundle before replacing an older generated output. This also
# keeps the old App's native files available when setup reuses that bundle.
$editorOutput = Join-Path $editorRoot ('.tools/bundle-' + [Guid]::NewGuid().ToString('N'))
& $editorSdk publish (Join-Path $editorRoot 'src/SimpleVideoEditor/SimpleVideoEditor.csproj') -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:DebugType=None -p:DebugSymbols=false -o $editorOutput --nologo
if ($LASTEXITCODE -ne 0) { throw 'Publish failed' }
Move-Item -LiteralPath (Join-Path $editorOutput 'SimpleVideoEditor.exe') -Destination (Join-Path $editorOutput 'Simple Video Editor.exe')
$editorBin = Join-Path $editorOutput 'bin'
$editorDocs = Join-Path $editorOutput 'docs'
New-Item -ItemType Directory -Path $editorBin,(Join-Path $editorDocs 'licenses'),(Join-Path $editorDocs 'images'),(Join-Path $editorDocs 'release-notes') -Force | Out-Null
$editorMpv = Resolve-EditorMediaFile 'mpv/libmpv-2.dll' 'libmpv-2.dll'
if ((Get-FileHash -LiteralPath $editorMpv -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) { throw 'Playback dependency checksum mismatch.' }
Copy-EditorDependency $editorMpv (Join-Path $editorBin 'libmpv-2.dll')
foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
    $editorSourceFile = Resolve-EditorMediaFile "ffmpeg/$($editorFile.Name)" "tools/$($editorFile.Name)"
    if ((Get-FileHash -LiteralPath $editorSourceFile -Algorithm SHA256).Hash -ne $editorFile.Value) { throw "Media dependency checksum mismatch: $($editorFile.Name)" }
    Copy-EditorDependency $editorSourceFile (Join-Path $editorBin $editorFile.Name)
}
Copy-EditorDependency (Resolve-EditorMediaFile 'mpv/LICENSE.LGPL' 'licenses/mpv-LGPL.txt') (Join-Path $editorDocs 'licenses/mpv-LGPL.txt')
Copy-EditorDependency (Resolve-EditorMediaFile 'ffmpeg/LICENSE.txt' 'licenses/ffmpeg-GPL.txt') (Join-Path $editorDocs 'licenses/ffmpeg-GPL.txt')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Destination (Join-Path $editorDocs 'licenses')
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'THIRD-PARTY-NOTICES.md'),(Join-Path $editorWorkspace 'README.md'),(Join-Path $editorWorkspace 'LICENSE'),(Join-Path $editorWorkspace 'CHANGELOG.md'),(Join-Path $editorWorkspace 'CONTRIBUTING.md'),(Join-Path $editorWorkspace 'SECURITY.md') -Destination $editorDocs
Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'docs') -Filter '*.md' -File | Copy-Item -Destination $editorDocs
Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'release-notes') -Filter '*.md' -File | Copy-Item -Destination (Join-Path $editorDocs 'release-notes')
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'docs/images/editor.png') -Destination (Join-Path $editorDocs 'images')
# The packaged guides share a folder with the repository's root documents.
foreach ($editorDocument in Get-ChildItem -LiteralPath $editorDocs -Filter '*.md' -File) {
    $editorDocumentText = [IO.File]::ReadAllText($editorDocument.FullName).Replace('](docs/', '](').Replace('](../', '](')
    [IO.File]::WriteAllText($editorDocument.FullName, $editorDocumentText, [Text.UTF8Encoding]::new($false))
}
$editorAssets = Get-Content -LiteralPath (Join-Path $editorRoot 'src/SimpleVideoEditor/obj/project.assets.json') -Raw | ConvertFrom-Json
$editorFramework = ([xml](Get-Content (Join-Path $editorRoot 'src/SimpleVideoEditor/SimpleVideoEditor.csproj') -Raw)).Project.PropertyGroup.TargetFramework
$editorRuntimeConfig = Get-Content (Join-Path $editorRoot "src/SimpleVideoEditor/bin/Release/$editorFramework/win-x64/SimpleVideoEditor.runtimeconfig.json") -Raw | ConvertFrom-Json
$editorRuntimeVersion = ($editorRuntimeConfig.runtimeOptions.includedFrameworks | Where-Object { $_.name -eq 'Microsoft.NETCore.App' }).version
if (!$editorRuntimeVersion) { throw 'Could not identify the bundled runtime version.' }
$editorPackageRoot = $editorAssets.packageFolders.psobject.Properties.Name | Select-Object -First 1
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.netcore.app.runtime.win-x64/$editorRuntimeVersion/LICENSE.TXT") -Destination (Join-Path $editorDocs 'licenses/dotnet-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.netcore.app.runtime.win-x64/$editorRuntimeVersion/THIRD-PARTY-NOTICES.TXT") -Destination (Join-Path $editorDocs 'licenses/dotnet-THIRD-PARTY-NOTICES.txt')
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.windowsdesktop.app.runtime.win-x64/$editorRuntimeVersion/LICENSE") -Destination (Join-Path $editorDocs 'licenses/WPF-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $editorRoot ".tools/dotnet/sdk/$($editorDependencies.SdkVersion)/Sdks/Microsoft.NET.Sdk.WindowsDesktop/THIRD-PARTY-NOTICES.TXT") -Destination (Join-Path $editorDocs 'licenses/WPF-THIRD-PARTY-NOTICES.txt')
@'
SIMPLE VIDEO EDITOR

Double-click Simple Video Editor.exe to start.
Keep the bin folder beside it. No installation is needed.

Import recordings, drag them onto the timeline, trim, and export.
Your original recordings stay unchanged.
The full guide is docs/user-guide.md. Guides and licenses are in docs.
'@ | Set-Content -LiteralPath (Join-Path $editorOutput 'README.txt') -Encoding utf8
Assert-EditorOutputStopped
if (Test-Path -LiteralPath $editorDestination) { Remove-Item -LiteralPath $editorDestination -Recurse -Force }
Move-Item -LiteralPath $editorOutput -Destination $editorDestination
$editorOutput = $editorDestination
if ($Zip) {
    $editorReleases = Join-Path $editorWorkspace 'Releases'; New-Item -ItemType Directory -Path $editorReleases -Force | Out-Null
    $editorZipPath = Join-Path $editorReleases 'SimpleVideoEditor-win-x64.zip'
    Compress-Archive -Path (Join-Path $editorOutput '*') -DestinationPath $editorZipPath -Force
    $editorZipHash = (Get-FileHash -LiteralPath $editorZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    "$editorZipHash  SimpleVideoEditor-win-x64.zip" | Set-Content -LiteralPath ($editorZipPath + '.sha256') -Encoding ascii
}
if ($OutputName -eq 'App') {
    $editorShell = New-Object -ComObject WScript.Shell
    $editorShortcut = $editorShell.CreateShortcut((Join-Path $editorWorkspace 'Simple Video Editor.lnk'))
    $editorShortcut.TargetPath = Join-Path $editorOutput 'Simple Video Editor.exe'; $editorShortcut.WorkingDirectory = $editorOutput
    $editorShortcut.IconLocation = $editorShortcut.TargetPath + ',0'; $editorShortcut.Description = 'Open Simple Video Editor'; $editorShortcut.Save()
}
Write-Output "Portable app: $editorOutput/Simple Video Editor.exe"
