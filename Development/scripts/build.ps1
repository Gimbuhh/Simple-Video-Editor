param([switch]$Zip,[string]$OutputName='App',[string]$PackageDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
. (Join-Path $PSScriptRoot 'project-paths.ps1')
. (Join-Path $PSScriptRoot 'portable-package.ps1')
if ($OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$') { throw 'Choose App or an App-* output folder.' }
$editorRustDestination=[IO.Path]::GetFullPath((Join-Path $editorWorkspace $OutputName))
if ((Split-Path -Parent $editorRustDestination) -ne [IO.Path]::GetFullPath($editorWorkspace)) { throw 'Output escapes the workspace.' }
function Assert-RustOutputStopped {
    if (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @('Simple Video Editor','simple-video-editor') -and $_.Path -and $_.Path.StartsWith($editorRustDestination+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) }) { throw 'The candidate app is running. Choose another output folder.' }
}
Assert-RustOutputStopped
$editorRustManifest=Join-Path $editorRustWorkspace 'Cargo.toml'
& $editorCargo build --manifest-path $editorRustManifest --release --locked -p simple-video-editor
if ($LASTEXITCODE -ne 0) { throw 'Rust desktop build failed.' }
$editorRustMetadata=(& $editorCargo metadata --manifest-path $editorRustManifest --format-version 1 --locked --filter-platform $editorRustTools.Host | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the locked dependency graph.' }
$editorRustPackage=$editorRustMetadata.packages | Where-Object name -eq 'simple-video-editor'
$editorRustVersion=$editorRustPackage.version
$editorRustStage=Join-Path $editorRustDevelopment ('.tools/rust-bundle-'+[guid]::NewGuid().ToString('N'))
$editorRustBin=Join-Path $editorRustStage 'bin';$editorRustDocs=Join-Path $editorRustStage 'docs';$editorRustLicenses=Join-Path $editorRustDocs 'licenses'
New-Item -ItemType Directory -Path $editorRustBin,$editorRustLicenses -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/simple-video-editor.exe') -Destination (Join-Path $editorRustStage 'Simple Video Editor.exe')
if ([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $editorRustStage 'Simple Video Editor.exe')).FileVersion -ne "$editorRustVersion.0") { throw 'Rust executable version resource mismatch.' }
$editorRustPins=Get-Content (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
foreach ($editorRustFile in $editorRustPins.FfmpegFiles.psobject.Properties) {
    $editorRustSource=Resolve-EditorMediaFile "ffmpeg/$($editorRustFile.Name)" "tools/$($editorRustFile.Name)"
    if ((Get-FileHash -LiteralPath $editorRustSource -Algorithm SHA256).Hash -ine $editorRustFile.Value) { throw "Native checksum mismatch: $($editorRustFile.Name)" }
    Copy-Item -LiteralPath $editorRustSource -Destination (Join-Path $editorRustBin $editorRustFile.Name)
}
$editorRustMpv=Resolve-EditorMediaFile 'mpv/libmpv-2.dll' 'libmpv-2.dll'
if ((Get-FileHash -LiteralPath $editorRustMpv -Algorithm SHA256).Hash -ine $editorRustPins.MpvLibrarySha256) { throw 'libmpv checksum mismatch.' }
Copy-Item -LiteralPath $editorRustMpv -Destination (Join-Path $editorRustBin 'libmpv-2.dll')
Copy-Item -LiteralPath (Resolve-EditorMediaFile 'mpv/LICENSE.LGPL' 'licenses/mpv-LGPL.txt') -Destination (Join-Path $editorRustLicenses 'mpv-LGPL.txt')
Copy-Item -LiteralPath (Resolve-EditorMediaFile 'ffmpeg/LICENSE.txt' 'licenses/ffmpeg-GPL.txt') -Destination (Join-Path $editorRustLicenses 'ffmpeg-GPL.txt')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Destination $editorRustLicenses
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'LICENSE'),(Join-Path $editorWorkspace 'THIRD-PARTY-NOTICES.md'),(Join-Path $editorWorkspace 'docs/rust-rewrite-status.md') -Destination $editorRustDocs
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'README.md'),(Join-Path $editorWorkspace 'CHANGELOG.md'),(Join-Path $editorWorkspace 'CONTRIBUTING.md'),(Join-Path $editorWorkspace 'SECURITY.md') -Destination $editorRustDocs
Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'docs') -Filter '*.md' -File | Copy-Item -Destination $editorRustDocs
New-Item -ItemType Directory -Path (Join-Path $editorRustDocs 'release-notes'),(Join-Path $editorRustDocs 'images') -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'release-notes') -Filter '*.md' -File | Copy-Item -Destination (Join-Path $editorRustDocs 'release-notes')
Copy-Item -LiteralPath (Join-Path $editorWorkspace 'docs/images/editor.png') -Destination (Join-Path $editorRustDocs 'images')
foreach($editorRustDocument in Get-ChildItem -LiteralPath $editorRustDocs -Filter '*.md' -File){
    $editorRustText=[IO.File]::ReadAllText($editorRustDocument.FullName).Replace('](docs/','](').Replace('](../','](')
    [IO.File]::WriteAllText($editorRustDocument.FullName,$editorRustText,[Text.UTF8Encoding]::new($false))
}
foreach($editorRustDocument in Get-ChildItem -LiteralPath (Join-Path $editorRustDocs 'release-notes') -Filter '*.md' -File){
    # Release notes move inside docs; their ../docs links must move with them.
    $editorRustText=[IO.File]::ReadAllText($editorRustDocument.FullName).Replace('](../docs/','](../')
    [IO.File]::WriteAllText($editorRustDocument.FullName,$editorRustText,[Text.UTF8Encoding]::new($false))
}
$editorRustLicenseRows=@()
foreach ($editorRustDependency in $editorRustMetadata.packages | Where-Object source) {
    $editorRustLicenseRows += "| $($editorRustDependency.name) | $($editorRustDependency.version) | $($editorRustDependency.license) |"
    $editorRustDependencyRoot=Split-Path -Parent $editorRustDependency.manifest_path
    $editorRustDependencyLicenses=@(Get-ChildItem -LiteralPath $editorRustDependencyRoot -Recurse -File | Where-Object Name -Match '(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT|^OFL\.txt$|^UFL\.txt$|^Hack-Regular\.txt$)')
    if ($editorRustDependencyLicenses.Count) {
        $editorRustLicenseFolder=Join-Path $editorRustLicenses "rust/$($editorRustDependency.name)-$($editorRustDependency.version)"
        New-Item -ItemType Directory -Path $editorRustLicenseFolder -Force | Out-Null
        foreach($editorRustLicenseFile in $editorRustDependencyLicenses){
            $editorRustRelative=[IO.Path]::GetRelativePath($editorRustDependencyRoot,$editorRustLicenseFile.FullName)
            $editorRustLicenseTarget=Join-Path $editorRustLicenseFolder $editorRustRelative
            New-Item -ItemType Directory -Path (Split-Path -Parent $editorRustLicenseTarget) -Force | Out-Null
            Copy-Item -LiteralPath $editorRustLicenseFile.FullName -Destination $editorRustLicenseTarget
        }
    }
}
@('# Locked Rust dependency licenses','','| Package | Version | SPDX license |','| --- | --- | --- |')+$editorRustLicenseRows | Set-Content (Join-Path $editorRustLicenses 'rust-dependencies.md') -Encoding utf8
$editorRustCompilerNotices=Join-Path $env:RUSTUP_HOME "toolchains/$env:RUSTUP_TOOLCHAIN/share/doc/rust"
foreach ($editorRustNotice in Get-ChildItem -LiteralPath $editorRustCompilerNotices -File | Where-Object Name -Match '^(LICENSE|COPYRIGHT)') {Copy-Item -LiteralPath $editorRustNotice.FullName -Destination (Join-Path $editorRustLicenses ('rust-'+$editorRustNotice.Name))}
@"
SIMPLE VIDEO EDITOR $editorRustVersion

Run Simple Video Editor.exe. Keep bin and docs beside it.
No .NET runtime or installation is required.
This release candidate awaits remaining acceptance; see docs/rust-rewrite-status.md.
Original recordings remain untouched. Save legacy projects to a new filename.
"@ | Set-Content (Join-Path $editorRustStage 'README.txt') -Encoding utf8
Assert-RustOutputStopped
if (Test-Path -LiteralPath $editorRustDestination) {Remove-Item -LiteralPath $editorRustDestination -Recurse -Force}
Move-Item -LiteralPath $editorRustStage -Destination $editorRustDestination
if ($Zip) {
    $editorRustPackages=Join-Path $editorRustDevelopment ('.tools/package-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $editorRustPackages -Force | Out-Null
    $editorRustZip=Join-Path $editorRustPackages (Get-EditorPackageName $editorRustVersion)
    Get-ChildItem -LiteralPath $editorRustDestination -Recurse -File | Where-Object LastWriteTime -LT ([datetime]::new(1980,1,1)) | ForEach-Object { $_.LastWriteTime=[datetime]::new(1980,1,1) }
    Compress-Archive -Path (Join-Path $editorRustDestination '*') -DestinationPath $editorRustZip -Force
    $editorRustHash=(Get-FileHash -LiteralPath $editorRustZip -Algorithm SHA256).Hash.ToLowerInvariant()
    "$editorRustHash  $([IO.Path]::GetFileName($editorRustZip))" | Set-Content -LiteralPath ($editorRustZip+'.sha256') -Encoding ascii
    Assert-EditorPackage $editorRustZip $editorRustVersion
    $editorRustPublished=if($PackageDirectory){[IO.Path]::GetFullPath($PackageDirectory)}else{Join-Path $editorWorkspace 'Releases'}
    Publish-EditorLocalPackage $editorRustZip $editorRustPublished $editorRustVersion
    Assert-EditorCurrentPackage $editorRustPublished $editorRustVersion
}
Write-Output "Rust portable app: $editorRustDestination/Simple Video Editor.exe"
