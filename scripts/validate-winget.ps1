param([Parameter(Mandatory = $true)][string]$Directory)

$ErrorActionPreference = 'Stop'
$schemaCommit = 'dafe5e7795c6b36e2a8cea86aa9fddec898df795'
# The catalog rejects deprecated manifest versions; keep this equal to
# WINGET_MANIFEST_VERSION in scripts/package.py.
$manifestVersion = '1.28.0'
if (!(Get-Module -ListAvailable -Name powershell-yaml | Where-Object Version -eq '0.4.12')) {
    Install-Module -Name powershell-yaml -RequiredVersion 0.4.12 -Scope CurrentUser -Force -Repository PSGallery
}
Import-Module powershell-yaml -RequiredVersion 0.4.12
$files = @(Get-ChildItem -LiteralPath $Directory -Filter '*.yaml' -Recurse -File)
if ($files.Count -ne 3) { throw 'Expected exactly three WinGet manifest files.' }
foreach ($file in $files) {
    $lines = Get-Content -LiteralPath $file.FullName
    if ($lines[0] -notmatch "^# yaml-language-server: \`$schema=https://aka.ms/winget-manifest\.\w+\.$([regex]::Escape($manifestVersion))\.schema\.json$") { throw "Missing manifest schema header for $manifestVersion." }
    # The catalog expects block YAML, not JSON-style flow collections or quoted keys.
    $body = @($lines | Where-Object { $_ -notmatch '^#' -and $_ -ne '' })
    if ($body | Where-Object { $_ -match '[{}\[\]]|^\s*"' }) { throw "Use block YAML style: $($file.Name)" }
    $manifest = ($body -join "`n") | ConvertFrom-Yaml -Ordered
    $type = $manifest.ManifestType
    if ($type -notin @('version', 'installer', 'defaultLocale')) { throw 'Unexpected manifest type.' }
    if ($manifest.ManifestVersion -ne $manifestVersion) { throw "Use ManifestVersion ${manifestVersion}: $($file.Name)" }
    $uri = "https://raw.githubusercontent.com/microsoft/winget-cli/$schemaCommit/schemas/JSON/manifests/v$manifestVersion/manifest.$type.$manifestVersion.json"
    $schema = (Invoke-WebRequest -Uri $uri).Content
    $json = $manifest | ConvertTo-Json -Depth 10
    if (!(Test-Json -Json $json -Schema $schema)) { throw "Invalid manifest: $($file.Name)" }
    Write-Output "Validated $($file.Name)"
}
