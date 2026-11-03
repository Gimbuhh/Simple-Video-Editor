param(
    [switch]$LiveCapture,
    [Parameter(Mandatory)][string]$RecordingDirectory,
    [Parameter(Mandatory)][int]$ExpectedGameProcessId,
    [ValidateRange(5,60)][int]$Seconds=12,
    [switch]$RequireHdr
)
$ErrorActionPreference='Stop'
if(!$LiveCapture){throw 'Live recording requires -LiveCapture. No NVIDIA settings are changed.'}
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$editorCaptureDirectory=(Get-Item -LiteralPath $RecordingDirectory).FullName
$editorCaptureOutput=Join-Path $editorDevelopment ('artifacts/shadowplay/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorCaptureOutput -Force | Out-Null
$editorCaptureExisting=@(Get-ChildItem -LiteralPath $editorCaptureDirectory -Filter '*.mp4' -File -Recurse | ForEach-Object FullName)
$editorCaptureReport=@{passed=$false;seconds=$Seconds;startedUtc=[datetime]::UtcNow.ToString('o')}
$editorCaptureOwned=$false
$editorCaptureFailure=$null
function Invoke-CaptureTool([string]$Name,[hashtable]$Arguments=@{}) {
    $editorCaptureBody=@{jsonrpc='2.0';id=2;method='tools/call';params=@{name=$Name;arguments=$Arguments}} | ConvertTo-Json -Depth 8 -Compress
    foreach($editorCaptureAttempt in 1..3){
        $editorCaptureReply=Invoke-RestMethod -Uri $editorCaptureUrl -Method Post -ContentType 'application/json' -Headers $editorCaptureHeaders -Body $editorCaptureBody -TimeoutSec 10
        if($editorCaptureReply.error.code -eq -32029){
            $editorCaptureDelay=[int]$editorCaptureReply.error.data.retryAfter
            if($editorCaptureDelay -lt 1 -or $editorCaptureDelay -gt 60){throw 'Unexpected NVIDIA rate-limit delay.'}
            Write-Output "NVIDIA rate limit: waiting $editorCaptureDelay seconds." | Out-Host
            Start-Sleep -Seconds $editorCaptureDelay
            continue
        }
        if($editorCaptureReply.error -or $editorCaptureReply.result.isError){throw "NVIDIA capture API error in $Name."}
        return $editorCaptureReply.result
    }
    throw 'NVIDIA capture API remained rate limited.'
}
try {
    # This server accepts stateless tool calls. Avoid accumulating persistent MCP sessions.
    # Use only an already-enabled server; never enable access or change settings.
    $editorCaptureUrl='http://127.0.0.1:13508/mcp'
    $editorCaptureHeaders=@{Accept='application/json, text/event-stream'}
    # Reserve room in NVIDIA's per-minute API budget before checking ownership/target.
    Write-Output 'Waiting for the NVIDIA API capture budget before the live test.' | Out-Host
    Start-Sleep -Seconds 60
    $editorCaptureReport.before=(Invoke-CaptureTool nvapp_overlay_get_status).structuredContent
    if($editorCaptureReport.before.recordingActive){throw 'A recording is already running; it was left untouched.'}
    $editorCaptureReport.target=(Invoke-CaptureTool nvapp_overlay_get_current_game_info).structuredContent
    if(!$editorCaptureReport.target.running -or $editorCaptureReport.target.processId -ne $ExpectedGameProcessId){throw 'NVIDIA is not targeting the expected game process.'}
    $editorCaptureOwned=$true
    $editorCaptureReport.startReply=Invoke-CaptureTool nvapp_overlay_capture @{action='toggle_recording';enable=$true}
    # The API acknowledges before its asynchronous state changes. Check actual state, not reply text.
    Start-Sleep -Seconds 1
    $editorCaptureStatus=(Invoke-CaptureTool nvapp_overlay_get_status).structuredContent
    if(!$editorCaptureStatus.recordingActive){throw 'NVIDIA did not start recording.'}
    Start-Sleep -Seconds ($Seconds-1)
    if(!(Invoke-CaptureTool nvapp_overlay_get_status).structuredContent.recordingActive){throw 'NVIDIA stopped recording early; check its capture notification/log.'}
} catch {
    $editorCaptureFailure=$_.Exception.Message
} finally {
    if($editorCaptureOwned){
        try {
            $editorCaptureReport.stopReply=Invoke-CaptureTool nvapp_overlay_capture @{action='toggle_recording';enable=$false}
            Start-Sleep -Seconds 2
            $editorCaptureReport.after=(Invoke-CaptureTool nvapp_overlay_get_status).structuredContent
            if($editorCaptureReport.after.recordingActive){throw 'NVIDIA did not stop the owned test recording.'}
        } catch { $editorCaptureFailure=$_.Exception.Message }
    }
}
try {
    if($editorCaptureFailure){throw $editorCaptureFailure}
    # Wait for the new file to finish being saved. Never modify or delete any recording.
    $editorCaptureDeadline=[datetime]::UtcNow.AddSeconds(10)
    $editorCapturePreviousSize=-1
    $editorCaptureStable=$false
    do {
        Start-Sleep -Milliseconds 500
        $editorCaptureNew=@(Get-ChildItem -LiteralPath $editorCaptureDirectory -Filter '*.mp4' -File -Recurse | Where-Object FullName -NotIn $editorCaptureExisting)
        if($editorCaptureNew.Count -eq 1 -and $editorCaptureNew[0].Length -gt 0 -and $editorCaptureNew[0].Length -eq $editorCapturePreviousSize){$editorCaptureStable=$true;break}
        if($editorCaptureNew.Count -eq 1){$editorCapturePreviousSize=$editorCaptureNew[0].Length}
    } while([datetime]::UtcNow -lt $editorCaptureDeadline)
    if($editorCaptureNew.Count -ne 1){throw 'Expected exactly one new game recording in the specified directory.'}
    if(!$editorCaptureStable){throw 'New game recording did not finish being saved.'}
    $editorCaptureFile=$editorCaptureNew[0].FullName
    $editorCaptureProbe=Resolve-EditorMediaFile 'ffmpeg/ffprobe.exe' 'tools/ffprobe.exe'
    $editorCaptureMetadata=& $editorCaptureProbe -v error -show_streams -show_format -of json $editorCaptureFile
    if($LASTEXITCODE -ne 0){throw 'Saved recording failed FFprobe validation.'}
    $editorCaptureMetadata | Set-Content -LiteralPath (Join-Path $editorCaptureOutput 'probe.json') -Encoding utf8
    $editorCaptureParsed=$editorCaptureMetadata | ConvertFrom-Json
    $editorCaptureVideo=$editorCaptureParsed.streams | Where-Object codec_type -eq 'video' | Select-Object -First 1
    if(!$editorCaptureVideo -or [double]$editorCaptureParsed.format.duration -lt $Seconds-2){throw 'Saved recording has no video or is shorter than the test.'}
    if($RequireHdr -and ($editorCaptureVideo.color_primaries -ne 'bt2020' -or $editorCaptureVideo.color_transfer -notin @('smpte2084','arib-std-b67'))){throw 'Saved recording does not contain the expected HDR metadata.'}
    $editorCaptureReport.file=$editorCaptureFile
    $editorCaptureReport.sha256=(Get-FileHash -LiteralPath $editorCaptureFile -Algorithm SHA256).Hash
    $editorCaptureReport.passed=$true
} catch { $editorCaptureFailure=$_.Exception.Message }
$editorCaptureReport.failure=$editorCaptureFailure
$editorCaptureReport | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $editorCaptureOutput 'capture.json') -Encoding utf8
Write-Output "Capture report: $editorCaptureOutput"
if($editorCaptureFailure){throw $editorCaptureFailure}
Write-Output "Saved recording: $editorCaptureFile (target, duration and metadata checked; inspect decoded frames for color/target correctness)."
