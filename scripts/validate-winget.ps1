param([Parameter(Mandatory = $true)][string]$Directory)

$ErrorActionPreference = 'Stop'
$schemaCommit = '5b62860167520b1503b3880d5a026809eb07c6f4'
$files = @(Get-ChildItem -LiteralPath $Directory -Filter '*.yaml' -Recurse -File)
if ($files.Count -ne 3) { throw 'Expected exactly three WinGet manifest files.' }
foreach ($file in $files) {
    $content = Get-Content -LiteralPath $file.FullName -Raw
    $manifest = $content | ConvertFrom-Json
    $type = $manifest.ManifestType
    if ($type -notin @('version', 'installer', 'defaultLocale')) { throw 'Unexpected manifest type.' }
    $uri = "https://raw.githubusercontent.com/microsoft/winget-cli/$schemaCommit/schemas/JSON/manifests/v1.9.0/manifest.$type.1.9.0.json"
    $schema = (Invoke-WebRequest -Uri $uri).Content
    if (!(Test-Json -Json $content -Schema $schema)) { throw "Invalid manifest: $($file.Name)" }
    Write-Output "Validated $($file.Name)"
}
