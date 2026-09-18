param([Parameter(Mandatory = $true)][string]$Directory)

$ErrorActionPreference = 'Stop'
$schemaCommit = '5b62860167520b1503b3880d5a026809eb07c6f4'
$files = @(Get-ChildItem -LiteralPath $Directory -Filter '*.yaml' -Recurse -File)
if ($files.Count -ne 3) { throw 'Expected exactly three WinGet manifest files.' }
foreach ($file in $files) {
    $lines = Get-Content -LiteralPath $file.FullName
    if ($lines[0] -notmatch '^# yaml-language-server: \$schema=') { throw 'Missing manifest schema header.' }
    $content = ($lines | Where-Object { $_ -notmatch '^#' }) -join "`n"
    $manifest = $content | ConvertFrom-Json
    $type = $manifest.ManifestType
    if ($type -notin @('version', 'installer', 'defaultLocale')) { throw 'Unexpected manifest type.' }
    $uri = "https://raw.githubusercontent.com/microsoft/winget-cli/$schemaCommit/schemas/JSON/manifests/v1.12.0/manifest.$type.1.12.0.json"
    $schema = (Invoke-WebRequest -Uri $uri).Content
    if (!(Test-Json -Json $content -Schema $schema)) { throw "Invalid manifest: $($file.Name)" }
    Write-Output "Validated $($file.Name)"
}
