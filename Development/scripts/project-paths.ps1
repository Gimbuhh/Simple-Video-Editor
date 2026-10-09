$editorDevelopment = Split-Path -Parent $PSScriptRoot
$editorWorkspace = if ((Split-Path -Leaf $editorDevelopment) -eq 'Development') { Split-Path -Parent $editorDevelopment } else { $editorDevelopment }
function Resolve-EditorMediaFile([string]$VendorPath, [string]$AppPath) {
    $editorCandidate = Join-Path (Join-Path $editorDevelopment 'vendor') $VendorPath
    if (Test-Path -LiteralPath $editorCandidate -PathType Leaf) { return $editorCandidate }
    $editorCandidate = Join-Path (Join-Path $editorWorkspace 'App') $AppPath
    if (Test-Path -LiteralPath $editorCandidate -PathType Leaf) { return $editorCandidate }
    $editorCandidate = Join-Path (Join-Path $editorWorkspace 'App/Support') $AppPath
    if (Test-Path -LiteralPath $editorCandidate -PathType Leaf) { return $editorCandidate }
    throw "Missing dependency: $VendorPath. Run Development/scripts/setup.ps1 first."
}
function Copy-EditorDependency([string]$Source, [string]$Destination) {
    if ([IO.Path]::GetFullPath($Source) -ne [IO.Path]::GetFullPath($Destination)) { Copy-Item -LiteralPath $Source -Destination $Destination }
}
