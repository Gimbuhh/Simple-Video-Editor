param([string]$ReleaseTag)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$editorProject = [xml](Get-Content -LiteralPath (Join-Path $editorDevelopment 'src/SimpleVideoEditor/SimpleVideoEditor.csproj') -Raw)
$editorVersion = [string]$editorProject.Project.PropertyGroup.Version
if ($editorVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'The application must have a three-part version.' }
if ($ReleaseTag -and $ReleaseTag -cne "v$editorVersion") { throw "Release tag must match the application version: v$editorVersion" }
$editorSdk = Get-Content -LiteralPath (Join-Path $editorWorkspace 'global.json') -Raw | ConvertFrom-Json
$editorDependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
if ($editorSdk.sdk.version -ne $editorDependencies.SdkVersion -or $editorSdk.sdk.rollForward -ne 'disable') { throw 'global.json must match the pinned SDK with rollForward disabled.' }
if ($editorDependencies.MediaMirrorTag -notmatch '^v\d+\.\d+\.\d+$') { throw 'The media mirror must name a versioned release tag.' }
$editorRequired = @('README.md','LICENSE','THIRD-PARTY-NOTICES.md','CHANGELOG.md','CONTRIBUTING.md','SECURITY.md','.gitignore','.gitattributes','docs/user-guide.md','docs/development.md','docs/releasing.md','docs/images/editor.png',"release-notes/$editorVersion.md",'.github/workflows/check.yml','.github/workflows/release.yml')
foreach ($editorRelative in $editorRequired) {
    if (!(Test-Path -LiteralPath (Join-Path $editorWorkspace $editorRelative) -PathType Leaf)) { throw "Missing repository file: $editorRelative" }
}
$editorChangelog = Get-Content -LiteralPath (Join-Path $editorWorkspace 'CHANGELOG.md') -Raw
if ($editorChangelog -notmatch "(?m)^## $([regex]::Escape($editorVersion))\s") { throw 'The changelog must contain the application version.' }
$editorReadme = Get-Content -LiteralPath (Join-Path $editorWorkspace 'README.md') -Raw
if (!$editorReadme.Contains("Version $editorVersion")) { throw 'Update the README version to match the app.' }
$editorMarkdown = @(Get-Item -LiteralPath (Join-Path $editorWorkspace 'README.md'),(Join-Path $editorWorkspace 'CONTRIBUTING.md'),(Join-Path $editorWorkspace 'SECURITY.md'),(Join-Path $editorDevelopment 'README.md'))
$editorMarkdown += Get-ChildItem -LiteralPath (Join-Path $editorWorkspace 'docs'),(Join-Path $editorWorkspace 'release-notes') -Filter '*.md' -File -Recurse
foreach ($editorFile in $editorMarkdown) {
    foreach ($editorMatch in [regex]::Matches((Get-Content -LiteralPath $editorFile.FullName -Raw), '\]\(([^)]+)\)')) {
        $editorLink = $editorMatch.Groups[1].Value
        if ($editorLink -match '^[a-zA-Z][a-zA-Z0-9+.-]*:' -or $editorLink.StartsWith('#')) { continue }
        $editorLink = ($editorLink -split '#',2)[0]
        if (!(Test-Path -LiteralPath (Join-Path $editorFile.DirectoryName $editorLink))) { throw "Broken documentation link in $($editorFile.Name): $editorLink" }
    }
}
$editorScripts = Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1' -File
foreach ($editorScript in $editorScripts) {
    $editorParseTokens = $null; $editorParseErrors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile($editorScript.FullName, [ref]$editorParseTokens, [ref]$editorParseErrors)
    if ($editorParseErrors.Count) { throw "PowerShell syntax errors in $($editorScript.Name): $editorParseErrors" }
}
Write-Output "Repository documentation, scripts, SDK pin, and version $editorVersion are consistent."
