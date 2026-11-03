param([string]$OutputName='App',[string]$PackageDirectory,[switch]$Desktop,[switch]$Extended)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
. (Join-Path $PSScriptRoot 'portable-package.ps1')
if($OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$'){throw 'Invalid app output name.'}
$editorVersion=Get-EditorVersion
$editorBundle=Join-Path $editorWorkspace $OutputName
$editorEntries=@(Get-ChildItem -LiteralPath $editorBundle | Select-Object -ExpandProperty Name)
if(@(Compare-Object @('bin','docs','README.txt','Simple Video Editor.exe') $editorEntries).Count){throw 'Unexpected portable app layout.'}
if([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $editorBundle 'Simple Video Editor.exe')).FileVersion -ne "$editorVersion.0"){throw 'App version differs from the Rust manifest.'}
$editorPackages=if($PackageDirectory){[IO.Path]::GetFullPath($PackageDirectory)}else{Join-Path $editorWorkspace 'Releases'}
Assert-EditorCurrentPackage $editorPackages $editorVersion
& (Join-Path $PSScriptRoot 'verify-rust-package.ps1') -PackagePath (Join-Path $editorPackages (Get-EditorPackageName $editorVersion)) -Desktop:$Desktop -Extended:$Extended
