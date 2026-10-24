$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
# The old application is a frozen test oracle restored from Git, never a
# production source tree or a dependency of the Rust application/package.
$editorBaseline='dd65df13a41490e8168a624b65fcff6a27956940'
& git -C $editorWorkspace cat-file -e "$($editorBaseline)^{commit}"
if($LASTEXITCODE -ne 0){throw 'Legacy parity needs the v1.0.4 Git commit. Fetch repository history before running -LegacyParity.'}
$editorOracleRoot=Join-Path $editorDevelopment 'artifacts/rewrite-baseline/source'
New-Item -ItemType Directory -Path $editorOracleRoot -Force | Out-Null
$editorOracleArchive=Join-Path $editorDevelopment 'artifacts/rewrite-baseline/oracle-source.tar'
try{
    & git -C $editorWorkspace archive --format=tar --output=$editorOracleArchive $editorBaseline -- Development/src/SimpleVideoEditor
    if($LASTEXITCODE -ne 0){throw 'Could not restore the pinned legacy oracle from Git.'}
    & tar -xf $editorOracleArchive -C $editorOracleRoot
    if($LASTEXITCODE -ne 0){throw 'Could not extract the test-only legacy oracle.'}
}finally{if(Test-Path -LiteralPath $editorOracleArchive){Remove-Item -LiteralPath $editorOracleArchive -Force}}
Write-Output "Legacy test oracle restored from $editorBaseline."
