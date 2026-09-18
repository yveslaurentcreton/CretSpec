param([Parameter(Mandatory = $true)][string]$Directory)

# Run on a disposable Windows runner with WinGet and Git installed.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$id = 'YvesLaurentCreton.CretSpec'
$manifest = Get-ChildItem -LiteralPath $Directory -Filter '*.installer.yaml' -Recurse -File
if (@($manifest).Count -ne 1) { throw 'Expected one installer manifest.' }
$data = (Get-Content -LiteralPath $manifest.FullName | Where-Object { $_ -notmatch '^#' }) -join "`n" | ConvertFrom-Json
$version = $data.PackageVersion
winget settings --enable LocalManifestFiles
try {
    winget validate --manifest $manifest.DirectoryName --disable-interactivity
    winget install --manifest $manifest.DirectoryName --scope user --accept-source-agreements --accept-package-agreements --disable-interactivity
    $env:PATH = [Environment]::GetEnvironmentVariable('PATH', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH', 'User')
    $command = Get-Command cspec -ErrorAction Stop
    Write-Output "Command registered at $($command.Source)"
    if ((cspec --version) -ne "cspec $version") { throw 'Installed version mismatch.' }
    cspec --help
    git --version
    winget list --id $id --exact --accept-source-agreements --disable-interactivity
    winget uninstall --id $id --exact --silent --disable-interactivity
    if (Get-Command cspec -ErrorAction SilentlyContinue) { throw 'Command alias survived uninstall.' }
    winget install --manifest $manifest.DirectoryName --scope user --accept-source-agreements --accept-package-agreements --disable-interactivity
    if ((cspec --version) -ne "cspec $version") { throw 'Reinstalled version mismatch.' }
    winget uninstall --id $id --exact --silent --disable-interactivity
} finally {
    winget settings --disable LocalManifestFiles
}
