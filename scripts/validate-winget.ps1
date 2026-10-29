param([Parameter(Mandatory = $true)][string]$Directory)

$ErrorActionPreference = 'Stop'
$schemaCommit = '5b62860167520b1503b3880d5a026809eb07c6f4'
if (!(Get-Module -ListAvailable -Name powershell-yaml | Where-Object Version -eq '0.4.12')) {
    Install-Module -Name powershell-yaml -RequiredVersion 0.4.12 -Scope CurrentUser -Force -Repository PSGallery
}
Import-Module powershell-yaml -RequiredVersion 0.4.12
$files = @(Get-ChildItem -LiteralPath $Directory -Filter '*.yaml' -Recurse -File)
if ($files.Count -ne 3) { throw 'Expected exactly three WinGet manifest files.' }
foreach ($file in $files) {
    $lines = Get-Content -LiteralPath $file.FullName
    if ($lines[0] -notmatch '^# yaml-language-server: \$schema=') { throw 'Missing manifest schema header.' }
    # The catalog expects block YAML, not JSON-style flow collections or quoted keys.
    $body = @($lines | Where-Object { $_ -notmatch '^#' -and $_ -ne '' })
    if ($body | Where-Object { $_ -match '[{}\[\]]|^\s*"' }) { throw "Use block YAML style: $($file.Name)" }
    $manifest = ($body -join "`n") | ConvertFrom-Yaml -Ordered
    $type = $manifest.ManifestType
    if ($type -notin @('version', 'installer', 'defaultLocale')) { throw 'Unexpected manifest type.' }
    $uri = "https://raw.githubusercontent.com/microsoft/winget-cli/$schemaCommit/schemas/JSON/manifests/v1.12.0/manifest.$type.1.12.0.json"
    $schema = (Invoke-WebRequest -Uri $uri).Content
    $json = $manifest | ConvertTo-Json -Depth 10
    if (!(Test-Json -Json $json -Schema $schema)) { throw "Invalid manifest: $($file.Name)" }
    Write-Output "Validated $($file.Name)"
}
