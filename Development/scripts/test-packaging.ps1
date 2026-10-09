param([string]$PackageDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
. (Join-Path $PSScriptRoot 'portable-package.ps1')
$editorVersion = ([xml](Get-Content (Join-Path $editorDevelopment 'src/SimpleVideoEditor/SimpleVideoEditor.csproj') -Raw)).Project.PropertyGroup.Version
$editorPackages = if ($PackageDirectory) { [IO.Path]::GetFullPath($PackageDirectory) } else { Join-Path $editorWorkspace 'Releases' }
$editorPackageName = Get-EditorPackageName $editorVersion
$editorSource = Join-Path $editorPackages $editorPackageName
$editorTests = Join-Path $editorDevelopment ('artifacts/packaging-checks-' + [Guid]::NewGuid().ToString('N'))
$editorChecks = 0
function Assert-Packaging($Condition, [string]$Description) {
    if (!$Condition) { throw $Description }
    $script:editorChecks++; Write-Output "PASS $Description"
}
function Expect-PackageRejection([scriptblock]$Action, [string]$Message, [string]$Description) {
    try { & $Action } catch {
        if (!$_.Exception.Message.Contains($Message)) { throw }
        Assert-Packaging $true $Description; return
    }
    throw "Expected rejection: $Description"
}
function Write-TestChecksum([string]$Path) {
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($Path))" | Set-Content -LiteralPath ($Path + '.sha256') -Encoding ascii
}
try {
    Assert-EditorPackage $editorSource $editorVersion
    Assert-Packaging $true 'The current version name, embedded app version and checksum agree'
    $editorOutput = Join-Path $editorTests 'Releases'
    $editorBadDirectory = Join-Path $editorTests 'bad-input'
    New-Item -ItemType Directory -Path $editorOutput,$editorBadDirectory -Force | Out-Null
    foreach ($name in @('SimpleVideoEditor-0.0.1-win-x64.zip','SimpleVideoEditor-0.0.1-win-x64.zip.sha256','SimpleVideoEditor-win-x64.zip','SimpleVideoEditor-win-x64.zip.sha256')) {
        'previous package' | Set-Content -LiteralPath (Join-Path $editorOutput $name)
    }
    'unrelated data' | Set-Content -LiteralPath (Join-Path $editorOutput 'personal-project.zip')
    $editorBadZip = Join-Path $editorBadDirectory $editorPackageName
    'invalid package' | Set-Content -LiteralPath $editorBadZip
    ('0' * 64 + "  $editorPackageName") | Set-Content -LiteralPath ($editorBadZip + '.sha256')
    Expect-PackageRejection { Publish-EditorLocalPackage $editorBadZip $editorOutput $editorVersion } 'Portable checksum mismatch.' 'A damaged replacement is rejected before replacing the previous package'
    Assert-Packaging ((Get-ChildItem -LiteralPath $editorOutput -File).Count -eq 5 -and (Get-Content -LiteralPath (Join-Path $editorOutput 'SimpleVideoEditor-0.0.1-win-x64.zip') -Raw).Trim() -eq 'previous package') 'Failed replacement preserves the previous files and their contents'

    Publish-EditorLocalPackage $editorSource $editorOutput $editorVersion
    Assert-EditorCurrentPackage $editorOutput $editorVersion
    Assert-Packaging ((Get-ChildItem -LiteralPath $editorOutput -File).Count -eq 3) 'Successful replacement removes stale versioned and unversioned package pairs'
    Assert-Packaging ((Get-Content -LiteralPath (Join-Path $editorOutput 'personal-project.zip') -Raw).Trim() -eq 'unrelated data') 'Package cleanup preserves unrelated files'
    $editorStaleZip = Join-Path $editorOutput 'SimpleVideoEditor-0.0.1-win-x64.zip'
    'stale package' | Set-Content -LiteralPath $editorStaleZip
    Expect-PackageRejection { Assert-EditorCurrentPackage $editorOutput $editorVersion } 'only the current version' 'Verification rejects a stale package left beside the current one'
    Remove-Item -LiteralPath $editorStaleZip

    Remove-Item -LiteralPath $editorBadZip
    $editorRealArchive = [IO.Compression.ZipFile]::OpenRead($editorSource)
    $editorWrongArchive = [IO.Compression.ZipFile]::Open($editorBadZip, [IO.Compression.ZipArchiveMode]::Create)
    try {
        # Use a real PE with different version metadata under the correct app filename.
        $inputStream = $editorRealArchive.GetEntry('bin/ffprobe.exe').Open()
        $outputStream = $editorWrongArchive.CreateEntry('Simple Video Editor.exe').Open()
        try { $inputStream.CopyTo($outputStream) } finally { $inputStream.Dispose(); $outputStream.Dispose() }
        foreach ($name in @('README.txt','bin/ffmpeg.exe','bin/ffprobe.exe','docs/guide.txt')) { $editorWrongArchive.CreateEntry($name).Open().Dispose() }
    } finally { $editorRealArchive.Dispose(); $editorWrongArchive.Dispose() }
    Write-TestChecksum $editorBadZip
    Expect-PackageRejection { Assert-EditorPackage $editorBadZip $editorVersion } 'Packaged executable version mismatch.' 'A correct ZIP filename cannot disguise an executable with the wrong version'
    $editorWrongArchive = [IO.Compression.ZipFile]::Open($editorBadZip, [IO.Compression.ZipArchiveMode]::Update)
    try { $editorWrongArchive.CreateEntry('docs/previous-version.zip').Open().Dispose() } finally { $editorWrongArchive.Dispose() }
    Write-TestChecksum $editorBadZip
    Expect-PackageRejection { Assert-EditorPackage $editorBadZip $editorVersion } 'extra app, shortcut or archive' 'Verification rejects a previous-version ZIP nested inside the package'
    $editorWrongArchive = [IO.Compression.ZipFile]::Open($editorBadZip, [IO.Compression.ZipArchiveMode]::Update)
    try {
        $editorWrongArchive.GetEntry('docs/previous-version.zip').Delete()
        $editorWrongArchive.CreateEntry('docs/Previous editor.exe').Open().Dispose()
    } finally { $editorWrongArchive.Dispose() }
    Write-TestChecksum $editorBadZip
    Expect-PackageRejection { Assert-EditorPackage $editorBadZip $editorVersion } 'extra app, shortcut or archive' 'Verification rejects a second app executable even under docs'
    Write-Output "All $editorChecks packaging checks passed."
} finally {
    $editorCheckedTests = [IO.Path]::GetFullPath($editorTests)
    if (!$editorCheckedTests.StartsWith([IO.Path]::GetFullPath((Join-Path $editorDevelopment 'artifacts')) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Packaging test cleanup escapes artifacts.' }
    if (Test-Path -LiteralPath $editorCheckedTests) { Remove-Item -LiteralPath $editorCheckedTests -Recurse -Force }
}
