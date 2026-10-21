param([switch]$BuildTools,[string]$OutputName='App')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
if((Split-Path -Leaf $editorDevelopment) -ne 'Development' -or $OutputName -notmatch '^App(?:-[A-Za-z0-9_-]+)?$'){throw 'Invalid repository/output layout.'}
$editorApp=Join-Path $editorWorkspace $OutputName
$editorVersion=Get-EditorVersion
if([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $editorApp 'Simple Video Editor.exe')).FileVersion -ne "$editorVersion.0"){throw 'Keep build tools until a current Rust App bundle has been verified.'}
$editorDependencies=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorPinned=@{'libmpv-2.dll'=$editorDependencies.MpvLibrarySha256}
foreach($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties){$editorPinned[$editorFile.Name]=$editorFile.Value}
foreach($editorName in $editorPinned.Keys){if((Get-FileHash -LiteralPath (Join-Path $editorApp "bin/$editorName") -Algorithm SHA256).Hash -ine $editorPinned[$editorName]){throw 'App media pins failed; keep development files.'}}
foreach($editorNotice in @('mpv-LGPL.txt','ffmpeg-GPL.txt','rust-dependencies.md')){if(!(Test-Path -LiteralPath (Join-Path $editorApp "docs/licenses/$editorNotice"))){throw 'App license notices are incomplete.'}}
$editorTargets=@('rust/target','vendor','tests/RustParityOracle/bin','tests/RustParityOracle/obj')
if($BuildTools){$editorTargets+='.tools'}
foreach($editorRelative in $editorTargets){
    $editorTarget=[IO.Path]::GetFullPath((Join-Path $editorDevelopment $editorRelative))
    if(!$editorTarget.StartsWith([IO.Path]::GetFullPath($editorDevelopment)+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Cleanup target escapes Development.'}
    if(Test-Path -LiteralPath $editorTarget){
        if((Get-Item -LiteralPath $editorTarget -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Cleanup target is a link; it was left untouched.'}
        Remove-Item -LiteralPath $editorTarget -Recurse -Force
        Write-Output "Removed reproducible build files: $editorRelative"
    }
}
Write-Output 'Source, app bundles, release packages and verification/rollback evidence were preserved. Run setup.ps1 before rebuilding.'
