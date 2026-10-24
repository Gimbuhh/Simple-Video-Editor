param([switch]$Gpu,[string]$ReuseFixturesFrom)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
$editorBenchmarkPins=Get-Content (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
$editorBenchmarkFfmpeg=Join-Path $editorRustDevelopment 'vendor/ffmpeg/ffmpeg.exe'
$editorBenchmarkProbe=Join-Path $editorRustDevelopment 'vendor/ffmpeg/ffprobe.exe'
$editorBenchmarkMpv=Join-Path $editorRustDevelopment 'vendor/mpv/libmpv-2.dll'
foreach($editorBenchmarkBinary in @(@($editorBenchmarkFfmpeg,$editorBenchmarkPins.FfmpegSha256),@($editorBenchmarkProbe,$editorBenchmarkPins.FfmpegFiles.'ffprobe.exe'),@($editorBenchmarkMpv,$editorBenchmarkPins.MpvLibrarySha256))){
    if((Get-FileHash -LiteralPath $editorBenchmarkBinary[0] -Algorithm SHA256).Hash -ine $editorBenchmarkBinary[1]){throw 'Benchmark native pin mismatch.'}
}
$editorBenchmarkRoot=Join-Path $editorRustDevelopment ('artifacts/playback-benchmark/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorBenchmarkRoot -Force | Out-Null
@{cpu=@(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors);gpu=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion);ffmpegSha256=$editorBenchmarkPins.FfmpegSha256;mpvSha256=$editorBenchmarkPins.MpvLibrarySha256;fixtureEncoder=$(if($ReuseFixturesFrom){'existing generated fixtures'}elseif($Gpu){'hevc_nvenc'}else{'libx265'});filesystemCache='uncontrolled';otherDesktopVerificationsAtStart=@(Get-CimInstance Win32_Process -Filter "name='simple-video-editor.exe' OR name='Simple Video Editor.exe'" | Where-Object CommandLine -Like '*--verify-desktop*').Count} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $editorBenchmarkRoot 'environment.json') -Encoding utf8
& $editorCargo build --manifest-path (Join-Path $editorRustWorkspace 'Cargo.toml') --release --locked -p sve-playback-lab
if($LASTEXITCODE -ne 0){throw 'Playback lab build failed.'}
$editorBenchmarkLab=Join-Path $env:CARGO_TARGET_DIR 'release/sve-playback-lab.exe'
$editorBenchmarkRows=@()
foreach($editorBenchmarkProfile in @('sdr','pq')){
    $editorBenchmarkSource=Join-Path $(if($ReuseFixturesFrom){[IO.Path]::GetFullPath($ReuseFixturesFrom)}else{$editorBenchmarkRoot}) "4k-$editorBenchmarkProfile.mp4"
    $editorBenchmarkFilter=if($editorBenchmarkProfile -eq 'sdr'){'format=yuv420p'}else{'zscale=pin=bt709:tin=bt709:min=bt709:rin=limited:t=linear:p=bt709:m=gbr:r=full:npl=100,format=gbrpf32le,zscale=pin=bt709:tin=linear:min=gbr:rin=full:p=bt2020:t=smpte2084:m=bt2020nc:r=limited,format=yuv420p10le'}
    $editorBenchmarkEncoder=if($Gpu){'hevc_nvenc'}else{'libx265'}
    $editorBenchmarkArguments=@('-v','error','-y','-f','lavfi','-i','testsrc2=size=3840x2160:rate=30:duration=12','-vf',$editorBenchmarkFilter,'-c:v',$editorBenchmarkEncoder,'-g','300')
    if($Gpu){$editorBenchmarkArguments+=@('-preset','p1','-rc','constqp','-qp','28','-no-scenecut','1')}else{$editorBenchmarkArguments+=@('-preset','ultrafast','-crf','28','-x265-params','keyint=300:min-keyint=300:scenecut=0:pools=4')}
    if($editorBenchmarkProfile -eq 'pq'){$editorBenchmarkArguments+=@('-color_trc','smpte2084','-color_primaries','bt2020','-colorspace','bt2020nc')}
    if(!$ReuseFixturesFrom){
        & $editorBenchmarkFfmpeg @editorBenchmarkArguments $editorBenchmarkSource
        if($LASTEXITCODE -ne 0){throw '4K benchmark fixture failed.'}
    }
    $editorBenchmarkHash=(Get-FileHash -LiteralPath $editorBenchmarkSource -Algorithm SHA256).Hash
    $editorBenchmarkFrames=& $editorBenchmarkProbe -v error -select_streams v:0 -show_entries frame=best_effort_timestamp_time,key_frame -of json $editorBenchmarkSource | ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Frame analysis failed.'}
    $editorBenchmarkKeys=@($editorBenchmarkFrames.frames | Where-Object key_frame -EQ 1 | ForEach-Object {[double]$_.best_effort_timestamp_time})
    if($editorBenchmarkKeys.Count -ne 2 -or [math]::Abs($editorBenchmarkKeys[1]-10) -gt 0.05){throw 'Expected actual keyframes at 0 and 10 seconds.'}
    $editorBenchmarkModes=if($editorBenchmarkProfile -eq 'sdr'){@('software','native-sdr')}else{@('native-sdr','native-auto')}
    foreach($editorBenchmarkMode in $editorBenchmarkModes){
        $editorBenchmarkJson=& $editorBenchmarkLab $editorBenchmarkMode $editorBenchmarkMpv $editorBenchmarkSource --benchmark
        if($LASTEXITCODE -ne 0){throw "4K benchmark failed: $editorBenchmarkMode $editorBenchmarkProfile"}
        $editorBenchmarkJson | Set-Content -LiteralPath (Join-Path $editorBenchmarkRoot "$editorBenchmarkMode-$editorBenchmarkProfile.json") -Encoding utf8
        $editorBenchmarkReport=$editorBenchmarkJson | ConvertFrom-Json
        $editorBenchmarkSamples=@($editorBenchmarkReport.cycles | ForEach-Object {$_.seekMs} | Sort-Object)
        if($editorBenchmarkSamples.Count -ne 24){throw 'Expected three cycles with eight completed seeks each.'}
        $editorBenchmarkMedian=($editorBenchmarkSamples[11]+$editorBenchmarkSamples[12])/2
        $editorBenchmarkRow=@{profile=$editorBenchmarkProfile;mode=$editorBenchmarkMode;sampleCount=$editorBenchmarkSamples.Count;medianMs=$editorBenchmarkMedian;p95Ms=$editorBenchmarkSamples[[int][math]::Ceiling($editorBenchmarkSamples.Count*0.95)-1];maximumMs=$editorBenchmarkSamples[-1];dimensions=@(3840,2160);codec='HEVC';keyframesSeconds=$editorBenchmarkKeys;sourceSha256=$editorBenchmarkHash;decoder='CPU';filesystemCache='uncontrolled';scope='native backend acknowledgement; excludes desktop input and shared project filter graph'}
        $editorBenchmarkRows+=$editorBenchmarkRow
        Write-Output "$editorBenchmarkMode $editorBenchmarkProfile · median=$($editorBenchmarkRow.medianMs) ms · p95=$($editorBenchmarkRow.p95Ms) ms"
    }
    if((Get-FileHash -LiteralPath $editorBenchmarkSource -Algorithm SHA256).Hash -ne $editorBenchmarkHash){throw 'Benchmark changed its recording.'}
}
$editorBenchmarkRows | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $editorBenchmarkRoot 'results.json') -Encoding utf8
Write-Output "Generated 4K/10-second-GOP diagnostic reports: $editorBenchmarkRoot"
