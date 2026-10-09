param([string]$FfmpegDirectory, [string]$MediaBundlePath)
$ErrorActionPreference = 'Stop'
$editorRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'project-paths.ps1')
. (Join-Path $PSScriptRoot 'portable-package.ps1')
$editorDependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorTools = Join-Path $editorRoot '.tools'
$editorVendor = Join-Path $editorRoot 'vendor'
New-Item -ItemType Directory -Path $editorTools,$editorVendor -Force | Out-Null
function Get-VerifiedArchive($Url, $Destination, $Algorithm, $ExpectedHash) {
    if (!(Test-Path -LiteralPath $Destination) -or (Get-FileHash -LiteralPath $Destination -Algorithm $Algorithm).Hash -ne $ExpectedHash) {
        Write-Output "Downloading $([IO.Path]::GetFileName($Destination))"
        try { Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing }
        catch { throw "Dependency download failed: $Url. $($_.Exception.Message)" }
    }
    if ((Get-FileHash -LiteralPath $Destination -Algorithm $Algorithm).Hash -ne $ExpectedHash) { throw "Checksum mismatch: $Destination" }
}
function Restore-EditorMediaBundle([string]$BundlePath) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $editorRestoreRoot = Join-Path $editorTools ('media-restore-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $editorRestoreRoot -Force | Out-Null
    $editorBundleFiles = @(@{Entry='libmpv-2.dll'; Vendor='mpv/libmpv-2.dll'; Hash=$editorDependencies.MpvLibrarySha256}, @{Entry='licenses/mpv-LGPL.txt'; Vendor='mpv/LICENSE.LGPL'}, @{Entry='licenses/ffmpeg-GPL.txt'; Vendor='ffmpeg/LICENSE.txt'})
    foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) { $editorBundleFiles += @{Entry="tools/$($editorFile.Name)"; Vendor="ffmpeg/$($editorFile.Name)"; Hash=$editorFile.Value} }
    $editorBundle = $null
    try {
        $editorBundle = [IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($BundlePath))
        foreach ($editorFile in $editorBundleFiles) {
            $editorEntry = $editorBundle.GetEntry($editorFile.Entry)
            $editorPortableEntry = if ($editorFile.Entry.StartsWith('licenses/')) { "docs/$($editorFile.Entry)" } else { "bin/$([IO.Path]::GetFileName($editorFile.Entry))" }
            if (!$editorEntry) { $editorEntry = $editorBundle.GetEntry($editorPortableEntry) }
            if (!$editorEntry) { throw "Missing media bundle entry: $($editorFile.Entry)" }
            $editorExtracted = Join-Path $editorRestoreRoot $editorFile.Vendor
            New-Item -ItemType Directory -Path (Split-Path -Parent $editorExtracted) -Force | Out-Null
            $editorEntryStream = $editorEntry.Open(); $editorDestinationStream = [IO.File]::Create($editorExtracted)
            try { $editorEntryStream.CopyTo($editorDestinationStream) } finally { $editorEntryStream.Dispose(); $editorDestinationStream.Dispose() }
            if ($editorFile.Hash -and (Get-FileHash -LiteralPath $editorExtracted -Algorithm SHA256).Hash -ne $editorFile.Hash) { throw "Media bundle checksum mismatch: $($editorFile.Entry)" }
        }
        # Validate every binary before replacing any existing vendor file.
        foreach ($editorFile in $editorBundleFiles) {
            $editorDestination = Join-Path $editorVendor $editorFile.Vendor
            New-Item -ItemType Directory -Path (Split-Path -Parent $editorDestination) -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $editorRestoreRoot $editorFile.Vendor) -Destination $editorDestination -Force
        }
        $editorDependencies | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $editorVendor 'dependencies.json')
        Write-Output 'Media dependencies restored from a checksum-verified portable release bundle.'
    } finally {
        if ($editorBundle) { $editorBundle.Dispose() }
        $editorVerifiedRestore = [IO.Path]::GetFullPath($editorRestoreRoot)
        if (!$editorVerifiedRestore.StartsWith([IO.Path]::GetFullPath($editorTools) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Media restore cleanup safety check failed.' }
        Remove-Item -LiteralPath $editorVerifiedRestore -Recurse -Force
    }
}
function Restore-EditorReleaseMirror([string]$DownloadError) {
    if (!$env:GH_REPO -or !(Get-Command gh -ErrorAction SilentlyContinue)) { throw "$DownloadError Restore the matching portable ZIP with -MediaBundlePath, or set GH_REPO and authenticate gh to use the private release mirror." }
    Write-Output "Upstream media unavailable; trying the pinned release mirror $($editorDependencies.MediaMirrorTag)."
    $editorMirrorAssets = & gh release view $editorDependencies.MediaMirrorTag --repo $env:GH_REPO --json assets
    if ($LASTEXITCODE -ne 0) { throw "$DownloadError The matching release mirror is unavailable." }
    $editorMirrorNames = @(($editorMirrorAssets | ConvertFrom-Json).assets.name)
    $editorMirrorName = Get-EditorPackageName ($editorDependencies.MediaMirrorTag.Substring(1))
    if ($editorMirrorName -notin $editorMirrorNames) { $editorMirrorName = 'SimpleVideoEditor-win-x64.zip' }
    if ($editorMirrorName -notin $editorMirrorNames) { throw "$DownloadError The matching release mirror has no portable ZIP." }
    & gh release download $editorDependencies.MediaMirrorTag --repo $env:GH_REPO --pattern $editorMirrorName --dir $editorTools --clobber
    if ($LASTEXITCODE -ne 0) { throw "$DownloadError The matching release mirror is also unavailable." }
    $editorMirrorZip = Join-Path $editorTools $editorMirrorName
    try { Restore-EditorMediaBundle $editorMirrorZip } finally { Remove-Item -LiteralPath $editorMirrorZip -Force }
}
$editorSdkArchive = Join-Path $editorTools 'dotnet-sdk.zip'
if (!(Test-Path -LiteralPath (Join-Path $editorTools 'dotnet\dotnet.exe'))) {
    Get-VerifiedArchive $editorDependencies.SdkUrl $editorSdkArchive 'SHA512' $editorDependencies.SdkSha512
    Expand-Archive -LiteralPath $editorSdkArchive -DestinationPath (Join-Path $editorTools 'dotnet') -Force
    Remove-Item -LiteralPath $editorSdkArchive -Force
}
if ($MediaBundlePath) {
    if ($FfmpegDirectory) { throw 'Use either MediaBundlePath or FfmpegDirectory, not both.' }
    Restore-EditorMediaBundle $MediaBundlePath
    return
}
$editorReuseApp = $false
if (!$FfmpegDirectory) {
    try {
        if ((Get-FileHash -LiteralPath (Resolve-EditorMediaFile 'mpv/libmpv-2.dll' 'libmpv-2.dll') -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) { throw 'Playback checksum mismatch.' }
        foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
            if ((Get-FileHash -LiteralPath (Resolve-EditorMediaFile "ffmpeg/$($editorFile.Name)" "tools/$($editorFile.Name)") -Algorithm SHA256).Hash -ne $editorFile.Value) { throw 'Media checksum mismatch.' }
        }
        $null = Resolve-EditorMediaFile 'mpv/LICENSE.LGPL' 'licenses/mpv-LGPL.txt'
        $null = Resolve-EditorMediaFile 'ffmpeg/LICENSE.txt' 'licenses/ffmpeg-GPL.txt'
        $editorReuseApp = $true
    } catch { $editorReuseApp = $false }
}
if ($editorReuseApp) { Write-Output 'Build tools are ready; media dependencies reuse the verified App bundle.'; return }
$editorVendorMpv = Join-Path $editorVendor 'mpv\libmpv-2.dll'
if (!(Test-Path -LiteralPath $editorVendorMpv) -or (Get-FileHash -LiteralPath $editorVendorMpv -Algorithm SHA256).Hash -ne $editorDependencies.MpvLibrarySha256) {
    $editorMpvArchive = Join-Path $editorTools 'libmpv.zip'
    try { Get-VerifiedArchive $editorDependencies.MpvUrl $editorMpvArchive 'SHA256' $editorDependencies.MpvSha256 }
    catch { Restore-EditorReleaseMirror $_.Exception.Message; return }
    Expand-Archive -LiteralPath $editorMpvArchive -DestinationPath (Join-Path $editorVendor 'mpv') -Force
    Remove-Item -LiteralPath $editorMpvArchive -Force
}
if (!$FfmpegDirectory) {
    $editorFfmpegArchive = Join-Path $editorTools 'ffmpeg-shared.zip'
    try { Get-VerifiedArchive $editorDependencies.FfmpegUrl $editorFfmpegArchive 'SHA256' $editorDependencies.FfmpegArchiveSha256 }
    catch { Restore-EditorReleaseMirror $_.Exception.Message; return }
    $editorFfmpegExtract = Join-Path $editorTools 'ffmpeg-shared'
    $editorFfmpegFolder = [IO.Path]::GetFileNameWithoutExtension($editorDependencies.FfmpegUrl)
    $FfmpegDirectory = Join-Path $editorFfmpegExtract "$editorFfmpegFolder\bin"
    if (!(Test-Path -LiteralPath (Join-Path $FfmpegDirectory 'ffmpeg.exe'))) { Expand-Archive -LiteralPath $editorFfmpegArchive -DestinationPath $editorFfmpegExtract -Force }
}
New-Item -ItemType Directory -Path (Join-Path $editorVendor 'ffmpeg') -Force | Out-Null
foreach ($editorFile in $editorDependencies.FfmpegFiles.psobject.Properties) {
    $editorSourceFile = Join-Path $FfmpegDirectory $editorFile.Name
    if (!(Test-Path -LiteralPath $editorSourceFile) -or (Get-FileHash -LiteralPath $editorSourceFile -Algorithm SHA256).Hash -ne $editorFile.Value) { throw "Media dependency checksum mismatch: $($editorFile.Name)" }
    Copy-Item -LiteralPath $editorSourceFile -Destination (Join-Path $editorVendor 'ffmpeg')
}
$editorFfmpegLicense = Join-Path (Split-Path -Parent $FfmpegDirectory) 'LICENSE.txt'
if (!(Test-Path -LiteralPath $editorFfmpegLicense)) { $editorFfmpegLicense = Join-Path $FfmpegDirectory 'LICENSE.txt' }
if (!(Test-Path -LiteralPath $editorFfmpegLicense)) { throw 'The FFmpeg license is missing. Provide it with the verified binaries.' }
Copy-Item -LiteralPath $editorFfmpegLicense -Destination (Join-Path $editorVendor 'ffmpeg\LICENSE.txt')
if ($editorFfmpegArchive -and (Test-Path -LiteralPath $editorFfmpegArchive)) { Remove-Item -LiteralPath $editorFfmpegArchive -Force }
if ($editorFfmpegExtract -and (Test-Path -LiteralPath $editorFfmpegExtract)) {
    $editorVerifiedExtract = [IO.Path]::GetFullPath($editorFfmpegExtract)
    if ($editorVerifiedExtract -ne [IO.Path]::GetFullPath((Join-Path $editorTools 'ffmpeg-shared'))) { throw 'Extraction cleanup safety check failed.' }
    Remove-Item -LiteralPath $editorVerifiedExtract -Recurse -Force
}
$editorDependencies | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $editorVendor 'dependencies.json')
Write-Output 'Workspace dependencies are ready.'
