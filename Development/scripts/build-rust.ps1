param([switch]$Zip,[string]$OutputName='App-Rust-Candidate')
# Compatibility entry point for isolated candidate builds. build.ps1 owns packaging.
& (Join-Path $PSScriptRoot 'build.ps1') -Zip:$Zip -OutputName $OutputName -PackageDirectory (Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts/rust-packages')
