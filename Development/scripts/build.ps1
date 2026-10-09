param([switch]$Zip, [string]$OutputName = 'App')
$ErrorActionPreference = 'Stop'
$editorRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$editorDependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorSdk = Join-Path $editorRoot '.tools\dotnet\dotnet.exe'
if (!(Test-Path -LiteralPath $editorSdk)) { throw 'Run Development/scripts/setup.ps1 first to restore the workspace build tools.' }
$env:DOTNET_CLI_HOME = Join-Path $editorRoot '.tools\cli'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
if ($OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$') { throw 'OutputName must be App or an App-* folder name at the project root.' }
$editorOutput = Join-Path $editorWorkspace $OutputName
& $editorSdk publish (Join-Path $editorRoot 'src\SimpleVideoEditor\SimpleVideoEditor.csproj') -c Release -r win-x64 --self-contained true -o $editorOutput --nologo
if ($LASTEXITCODE -ne 0) { throw 'Publish failed' }
New-Item -ItemType Directory -Path (Join-Path $editorOutput 'tools'),(Join-Path $editorOutput 'licenses') -Force | Out-Null
$editorMpv = Resolve-EditorMediaFile 'mpv/libmpv-2.dll' 'libmpv-2.dll'
if ((Get-FileHash -LiteralPath $editorMpv -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) { throw 'Playback dependency checksum mismatch.' }
Copy-EditorDependency $editorMpv (Join-Path $editorOutput 'libmpv-2.dll')
foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
    $editorSourceFile = Resolve-EditorMediaFile "ffmpeg/$($editorFile.Name)" "tools/$($editorFile.Name)"
    if ((Get-FileHash -LiteralPath $editorSourceFile -Algorithm SHA256).Hash -ne $editorFile.Value) { throw "Media dependency checksum mismatch: $($editorFile.Name)" }
    Copy-EditorDependency $editorSourceFile (Join-Path (Join-Path $editorOutput 'tools') $editorFile.Name)
}
Copy-EditorDependency (Resolve-EditorMediaFile 'mpv/LICENSE.LGPL' 'licenses/mpv-LGPL.txt') (Join-Path $editorOutput 'licenses/mpv-LGPL.txt')
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'THIRD-PARTY-NOTICES.md'),(Join-Path $editorWorkspace 'README.md'),(Join-Path $editorWorkspace 'LICENSE'),(Join-Path $editorWorkspace 'CHANGELOG.md'),(Join-Path $editorWorkspace 'CONTRIBUTING.md'),(Join-Path $editorWorkspace 'SECURITY.md') -Destination $editorOutput
New-Item -ItemType Directory -Path (Join-Path $editorOutput 'docs/images') -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'docs') -Filter '*.md' -File | Copy-Item -Destination (Join-Path $editorOutput 'docs')
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'docs/images/editor.png') -Destination (Join-Path $editorOutput 'docs/images')
Copy-EditorDependency (Resolve-EditorMediaFile 'ffmpeg/LICENSE.txt' 'licenses/ffmpeg-GPL.txt') (Join-Path $editorOutput 'licenses/ffmpeg-GPL.txt')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Destination (Join-Path $editorOutput 'licenses')
$editorRuntimeConfig = Get-Content -LiteralPath (Join-Path $editorOutput 'SimpleVideoEditor.runtimeconfig.json') -Raw | ConvertFrom-Json
$editorRuntimeVersion = ($editorRuntimeConfig.runtimeOptions.includedFrameworks | Where-Object { $_.name -eq 'Microsoft.NETCore.App' }).version
$editorAssets = Get-Content -LiteralPath (Join-Path $editorRoot 'src\SimpleVideoEditor\obj\project.assets.json') -Raw | ConvertFrom-Json
$editorPackageRoot = $editorAssets.packageFolders.psobject.Properties.Name | Select-Object -First 1
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.netcore.app.runtime.win-x64\$editorRuntimeVersion\LICENSE.TXT") -Destination (Join-Path $editorOutput 'licenses\dotnet-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.netcore.app.runtime.win-x64\$editorRuntimeVersion\THIRD-PARTY-NOTICES.TXT") -Destination (Join-Path $editorOutput 'licenses\dotnet-THIRD-PARTY-NOTICES.txt')
Copy-Item -LiteralPath (Join-Path $editorPackageRoot "microsoft.windowsdesktop.app.runtime.win-x64\$editorRuntimeVersion\LICENSE") -Destination (Join-Path $editorOutput 'licenses\WPF-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $editorRoot ".tools\dotnet\sdk\$($editorDependencies.SdkVersion)\Sdks\Microsoft.NET.Sdk.WindowsDesktop\THIRD-PARTY-NOTICES.TXT") -Destination (Join-Path $editorOutput 'licenses\WPF-THIRD-PARTY-NOTICES.txt')
if ($Zip) {
    $editorReleases = Join-Path $editorWorkspace 'Releases'; New-Item -ItemType Directory -Path $editorReleases -Force | Out-Null
    Compress-Archive -Path (Join-Path $editorOutput '*') -DestinationPath (Join-Path $editorReleases 'SimpleVideoEditor-win-x64.zip') -Force
    $editorZipPath = Join-Path $editorReleases 'SimpleVideoEditor-win-x64.zip'
    $editorZipHash = (Get-FileHash -LiteralPath $editorZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    "$editorZipHash  SimpleVideoEditor-win-x64.zip" | Set-Content -LiteralPath ($editorZipPath + '.sha256') -Encoding ascii
}
if ($OutputName -eq 'App') {
    $editorShell = New-Object -ComObject WScript.Shell
    $editorShortcut = $editorShell.CreateShortcut((Join-Path $editorWorkspace 'Simple Video Editor.lnk'))
    $editorShortcut.TargetPath = Join-Path $editorOutput 'SimpleVideoEditor.exe'; $editorShortcut.WorkingDirectory = $editorOutput
    $editorShortcut.IconLocation = $editorShortcut.TargetPath + ',0'; $editorShortcut.Description = 'Open Simple Video Editor'; $editorShortcut.Save()
}
Write-Output "Portable app: $editorOutput\SimpleVideoEditor.exe"
