param([switch]$LegacyParity, [switch]$Playback)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
if (!(Test-Path -LiteralPath $editorCargo)) { throw 'Run Development/scripts/setup-rust.ps1 first.' }
$editorRustManifest = Join-Path $editorRustWorkspace 'Cargo.toml'
& $editorCargo fmt --manifest-path $editorRustManifest --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
& $editorCargo clippy --manifest-path $editorRustManifest --workspace --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Rust lint checks failed.' }
$editorPathFixtureRoot = Join-Path $editorRustDevelopment 'artifacts/verification'
if (Test-Path -LiteralPath (Join-Path $editorPathFixtureRoot 'local-recording-link/source B.mp4')) {
    $env:SVE_PATH_FIXTURES = $editorPathFixtureRoot
} else { Remove-Item Env:SVE_PATH_FIXTURES -ErrorAction SilentlyContinue }
& $editorCargo test --manifest-path $editorRustManifest --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
& $editorCargo build --manifest-path $editorRustManifest --release --locked -p sve-inspect
if ($LASTEXITCODE -ne 0) { throw 'Rust inspector build failed.' }
$editorRustFixture = Join-Path $editorRustWorkspace 'crates/project-store/tests/fixtures/gaps-library.json'
$editorRustBefore = (Get-FileHash -LiteralPath $editorRustFixture -Algorithm SHA256).Hash
$editorRustSummary = & (Join-Path $env:CARGO_TARGET_DIR 'release/sve-inspect.exe') $editorRustFixture
if ($LASTEXITCODE -ne 0) { throw 'Built Rust inspector failed.' }
$editorRustResult = $editorRustSummary | ConvertFrom-Json
if ($editorRustResult.clipCount -ne 2 -or $editorRustResult.sourceCount -ne 2 -or $editorRustResult.durationSeconds -ne 5) { throw 'Built inspector returned an incorrect project summary.' }
if ((Get-FileHash -LiteralPath $editorRustFixture -Algorithm SHA256).Hash -ne $editorRustBefore) { throw 'Inspector changed its input project.' }
if ($LegacyParity) {
    $editorOracleProject = Join-Path $editorRustDevelopment 'tests/RustParityOracle/RustParityOracle.csproj'
    $editorOracleSdk = Join-Path $editorRustDevelopment '.tools/dotnet/dotnet.exe'
    if (!(Test-Path -LiteralPath $editorOracleSdk)) { throw 'Run setup.ps1 before the legacy parity checks.' }
    $env:DOTNET_CLI_HOME = Join-Path $editorRustDevelopment '.tools/cli'
    $env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
    & $editorOracleSdk build $editorOracleProject -c Release --nologo
    if ($LASTEXITCODE -ne 0) { throw 'C# parity oracle build failed.' }
    $editorOracleDll = Join-Path $editorRustDevelopment 'tests/RustParityOracle/bin/Release/net10.0-windows/RustParityOracle.dll'
    foreach ($editorFixture in Get-ChildItem -LiteralPath (Join-Path $editorRustWorkspace 'crates/project-store/tests/fixtures') -Filter '*.json' -File) {
        $editorFixtureHash = (Get-FileHash -LiteralPath $editorFixture.FullName -Algorithm SHA256).Hash
        $editorLegacyJson = & $editorOracleSdk $editorOracleDll $editorFixture.FullName
        if ($LASTEXITCODE -ne 0) { throw "Legacy reader failed: $($editorFixture.Name)" }
        $editorNewJson = & (Join-Path $env:CARGO_TARGET_DIR 'release/sve-inspect.exe') $editorFixture.FullName
        if ($LASTEXITCODE -ne 0) { throw "Rust reader failed: $($editorFixture.Name)" }
        $editorLegacy = $editorLegacyJson | ConvertFrom-Json
        $editorNew = $editorNewJson | ConvertFrom-Json
        foreach ($editorField in @('schemaVersion','sourceCount','clipCount','durationSeconds')) {
            if ($editorLegacy.$editorField -cne $editorNew.$editorField) { throw "Project parity mismatch: $($editorFixture.Name) $editorField" }
        }
        for ($editorIndex = 0; $editorIndex -lt $editorLegacy.clipCount; $editorIndex++) {
            foreach ($editorField in @('clipId','sourceId','sourceIn','sourceOut','timelineStart')) {
                if ($editorLegacy.clips[$editorIndex].$editorField -cne $editorNew.clips[$editorIndex].$editorField) { throw "Clip parity mismatch: $($editorFixture.Name) clip $editorIndex $editorField" }
            }
        }
        if ((Get-FileHash -LiteralPath $editorFixture.FullName -Algorithm SHA256).Hash -ne $editorFixtureHash) { throw 'Parity check changed a fixture.' }
        Write-Output "C#/Rust import parity passed: $($editorFixture.Name)"
    }
}
Write-Output 'Rust formatting, lint, contracts, executable tests, and release inspector smoke check passed.'
if ($Playback) { & (Join-Path $PSScriptRoot 'test-rust-playback.ps1') }
