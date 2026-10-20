param([switch]$Gpu)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
$editorPlaybackPins = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorPlaybackLibrary = Join-Path $editorRustDevelopment 'vendor/mpv/libmpv-2.dll'
$editorPlaybackFfmpeg = Join-Path $editorRustDevelopment 'vendor/ffmpeg/ffmpeg.exe'
if (!(Test-Path -LiteralPath $editorPlaybackLibrary) -or !(Test-Path -LiteralPath $editorPlaybackFfmpeg)) { throw 'Run setup.ps1 before native playback checks.' }
if ((Get-FileHash -LiteralPath $editorPlaybackFfmpeg -Algorithm SHA256).Hash -ine $editorPlaybackPins.FfmpegSha256) { throw 'FFmpeg checksum mismatch.' }
$editorPlaybackRoot = Join-Path $editorRustDevelopment 'artifacts/rust-playback'
New-Item -ItemType Directory -Path $editorPlaybackRoot -Force | Out-Null
& $editorCargo build --manifest-path (Join-Path $editorRustWorkspace 'Cargo.toml') --release --locked -p sve-playback-lab
if ($LASTEXITCODE -ne 0) { throw 'Rust playback lab build failed.' }
$editorPlaybackLab = Join-Path $env:CARGO_TARGET_DIR 'release/sve-playback-lab.exe'
$editorPlaybackChecks = 0
function Assert-RustPlayback($Condition, [string]$Message) {
    if (!$Condition) { throw $Message }
    $script:editorPlaybackChecks++; Write-Host "PASS $Message"
}
function Invoke-RustPlayback([string]$Mode, [string]$Source) {
    $editorOriginalHash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
    $editorReport = & $editorPlaybackLab $Mode $editorPlaybackLibrary $Source
    if ($LASTEXITCODE -ne 0) { throw "Playback lab failed: $Mode $([IO.Path]::GetFileName($Source))" }
    Assert-RustPlayback ((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -eq $editorOriginalHash) 'Native playback keeps the original fixture unchanged'
    $editorReport | Set-Content -LiteralPath (Join-Path $editorPlaybackRoot ("$Mode-" + [IO.Path]::GetFileNameWithoutExtension($Source) + '.json'))
    $editorParsed = $editorReport | ConvertFrom-Json
    Assert-RustPlayback ($editorParsed.cycles.Count -eq 3) 'Three native create/play/seek/shutdown cycles complete'
    Assert-RustPlayback (!$editorParsed.hdrPresentationVerified -and !$editorParsed.toolkitSelected) 'Prototype evidence never claims certified HDR presentation or a selected toolkit'
    return $editorParsed
}
# Generated fixtures only. Transfer conversion changes actual sample values;
# HDR fixtures are not SDR pixels with new tags attached.
foreach ($editorTransfer in @('sdr','pq','hlg')) {
    $editorSource = Join-Path $editorPlaybackRoot "$editorTransfer.mkv"
    $editorFilter = if ($editorTransfer -eq 'sdr') { 'format=yuv420p' } else {
        $editorTargetTrc = if ($editorTransfer -eq 'pq') { 'smpte2084' } else { 'arib-std-b67' }
        "zscale=pin=bt709:tin=bt709:min=bt709:rin=limited:t=linear:p=bt709:m=gbr:r=full:npl=100,format=gbrpf32le,zscale=pin=bt709:tin=linear:min=gbr:rin=full:p=bt2020:t=${editorTargetTrc}:m=bt2020nc:r=limited,format=yuv420p10le"
    }
    $editorArguments = @('-v','error','-y','-f','lavfi','-i','testsrc2=size=320x180:rate=30:duration=3','-vf',$editorFilter,'-c:v','ffv1')
    if ($editorTransfer -ne 'sdr') { $editorArguments += @('-color_trc',$editorTargetTrc,'-color_primaries','bt2020','-colorspace','bt2020nc') }
    & $editorPlaybackFfmpeg @editorArguments $editorSource
    if ($LASTEXITCODE -ne 0) { throw "Fixture generation failed: $editorTransfer" }
}
$editorSoftware = Invoke-RustPlayback 'software' (Join-Path $editorPlaybackRoot 'sdr.mkv')
foreach ($editorCycle in $editorSoftware.cycles) {
    Assert-RustPlayback ($editorCycle.properties.'current-vo' -eq 'libmpv' -and $editorCycle.properties.'hwdec-current' -eq 'no' -and $editorCycle.softwareFrame.nonBlackPixels -gt 0) 'Software API presents real CPU-decoded frames without a GPU output backend'
}
foreach ($editorTransfer in @('pq','hlg')) {
    $editorFailure = & $editorPlaybackLab software $editorPlaybackLibrary (Join-Path $editorPlaybackRoot "$editorTransfer.mkv") 2>&1
    Assert-RustPlayback ($LASTEXITCODE -ne 0 -and "$editorFailure".Contains('cannot tone-map HDR')) "Software SDR reference explicitly rejects $editorTransfer rather than displaying incorrect HDR colors"
}
$editorMissing = Join-Path $editorPlaybackRoot 'does-not-exist.mkv'
$editorFailure = & $editorPlaybackLab software $editorPlaybackLibrary $editorMissing 2>&1
Assert-RustPlayback ($LASTEXITCODE -ne 0 -and "$editorFailure".Contains('libmpv error')) 'A missing recording fails through the native end-file error without hanging'
if ($Gpu) {
    foreach ($editorTransfer in @('sdr','pq','hlg')) {
        $editorNative = Invoke-RustPlayback 'native-sdr' (Join-Path $editorPlaybackRoot "$editorTransfer.mkv")
        foreach ($editorCycle in $editorNative.cycles) {
            $editorTarget = $editorCycle.properties.'video-target-params' | ConvertFrom-Json
            $editorInput = $editorCycle.properties.'video-params' | ConvertFrom-Json
            $editorExpectedGamma = if ($editorTransfer -eq 'pq') { 'pq' } elseif ($editorTransfer -eq 'hlg') { 'hlg' } else { 'bt.1886' }
            Assert-RustPlayback ($editorInput.gamma -eq $editorExpectedGamma) "The decoder identifies the generated $editorTransfer transfer function"
            Assert-RustPlayback ($editorCycle.properties.'current-vo' -eq 'gpu-next' -and $editorCycle.properties.'current-gpu-context' -eq 'd3d11' -and $editorTarget.gamma -eq 'srgb' -and $editorTarget.primaries -eq 'bt.709') "Native $editorTransfer preview uses D3D11 with the deliberate SDR target"
        }
    }
    foreach ($editorTransfer in @('pq','hlg')) {
        $editorAuto = Invoke-RustPlayback 'native-auto' (Join-Path $editorPlaybackRoot "$editorTransfer.mkv")
        foreach ($editorCycle in $editorAuto.cycles) {
            Assert-RustPlayback ($editorCycle.properties.'current-gpu-context' -eq 'd3d11') 'Automatic native target reports its actual D3D11 context'
        }
    }
}
@{passed=$editorPlaybackChecks; gpu=[bool]$Gpu; hdrPresentationVerified=$false; toolkitSelected=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $editorPlaybackRoot 'results.json')
Write-Output "All $editorPlaybackChecks Rust playback checks passed. Physical HDR, visual accuracy, pointer input, toolkit embedding, and game capture remain unverified."
