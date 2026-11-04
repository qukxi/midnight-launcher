$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$runId = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$productName = "Midnight Installer Check $runId"
$bundleId = "dev.midnight.installer-check-$runId"
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
$runRoot = Join-Path $projectRoot "artifacts/installer verification/$runId"
$installPath = Join-Path $runRoot 'app with spaces'
$configPath = Join-Path $runRoot 'tauri-check.json'
$source = Join-Path $projectRoot 'src-tauri/target/x86_64-pc-windows-msvc/release/midnight-launcher.exe'
$installer = Join-Path $projectRoot "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/${productName}_${version}_x64-setup.exe"
$registryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$productName"

if (-not (Test-Path -LiteralPath $source)) { throw 'Run npm run build:win before checking the installer.' }
if (Test-Path -LiteralPath $registryPath) { throw 'The isolated verification registry key already exists.' }
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
$config = @{
  productName = $productName
  identifier = $bundleId
  bundle = @{ windows = @{ nsis = @{ languages = @('Russian') } } }
}
$config | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $configPath -Encoding UTF8
Push-Location $projectRoot
try {
  & npm.cmd run tauri -- bundle --target x86_64-pc-windows-msvc --bundles nsis --ci --no-binary-patching --config $configPath
  if ($LASTEXITCODE -ne 0) { throw "Verification bundle failed: $LASTEXITCODE" }
} finally {
  Pop-Location
}

function Get-ContentHash([string]$file) {
  $algorithm = [Security.Cryptography.SHA256]::Create()
  $stream = [IO.File]::OpenRead($file)
  try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '') }
  finally { $stream.Dispose(); $algorithm.Dispose() }
}

function Install-Check {
  # NSIS requires /D to be last and its path to be unquoted, even with spaces.
  $process = Start-Process -FilePath $installer -ArgumentList "/S /NS /D=$installPath" -WindowStyle Hidden -PassThru -Wait
  if ($process.ExitCode -ne 0) { throw "Silent installation failed: $($process.ExitCode)" }
  $installed = Join-Path $installPath 'midnight-launcher.exe'
  if (-not (Test-Path -LiteralPath $installed)) { throw 'The installed launcher is missing.' }
  if ((Get-ContentHash $installed) -ne (Get-ContentHash $source)) {
    throw 'The installed executable differs from the release executable.'
  }
  $registration = Get-ItemProperty -LiteralPath $registryPath
  if ($registration.DisplayName -ne $productName) { throw 'The isolated uninstall registration is missing.' }
}

$uninstaller = Join-Path $installPath 'uninstall.exe'
try {
  Install-Check
  $sentinel = Join-Path $installPath 'keep-existing-file.txt'
  [IO.File]::WriteAllText($sentinel, 'Keep unrelated files during reinstall and uninstall.')
  Install-Check
  if (-not (Test-Path -LiteralPath $sentinel)) { throw 'Reinstallation removed an unrelated file.' }
} finally {
  # Also remove the isolated registration when an assertion above fails.
  if (Test-Path -LiteralPath $uninstaller) {
    $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "Silent uninstallation failed: $($process.ExitCode)" }
  }
}
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath $registryPath) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
if (Test-Path -LiteralPath $registryPath) { throw 'Uninstallation left its isolated registry key.' }
if (Test-Path -LiteralPath (Join-Path $installPath 'midnight-launcher.exe')) { throw 'Uninstallation left the launcher executable.' }
if (-not (Test-Path -LiteralPath $sentinel)) { throw 'Uninstallation removed an unrelated file.' }

$report = @{
  checkedAt = [DateTime]::UtcNow.ToString('o')
  productName = $productName
  bundleId = $bundleId
  installPath = $installPath
  sourceSha256 = Get-ContentHash $source
  isolatedInstaller = $installer
  install = 'passed'
  reinstall = 'passed'
  uninstall = 'passed'
  unrelatedFilePreserved = $true
  authenticatedGameplay = 'not tested'
}
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runRoot 'result.json') -Encoding UTF8
Write-Output "Installation, reinstallation, removal and unrelated-file preservation passed. Report: $runRoot/result.json"
