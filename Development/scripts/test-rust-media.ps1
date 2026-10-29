param([switch]$Gpu)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
$env:SVE_MEDIA_FIXTURES = Join-Path $editorRustDevelopment 'artifacts/rust-playback'
foreach ($editorFixture in @('sdr','pq','hlg')) { if (!(Test-Path -LiteralPath (Join-Path $env:SVE_MEDIA_FIXTURES "$editorFixture.mkv"))) { throw 'Run test-rust-playback.ps1 to generate the media fixtures first.' } }
if ($Gpu) { $env:SVE_MEDIA_GPU = '1' } else { Remove-Item Env:SVE_MEDIA_GPU -ErrorAction SilentlyContinue }
& $editorCargo test --manifest-path (Join-Path $editorRustWorkspace 'Cargo.toml') -p media-engine --locked -- --include-ignored
if ($LASTEXITCODE -ne 0) { throw 'Rust media integration checks failed.' }
Write-Output 'Rust mixed SDR/PQ/HLG exports, exact frame counts, gaps, audio, source protection, atomic cancellation and project roundtrips passed.'
