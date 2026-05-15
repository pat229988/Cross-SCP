# SPDX-License-Identifier: AGPL-3.0-or-later

param(
    [string]$BuildDir = "build/gui-release",
    [string]$ExePath = "",
    [string]$QtBinDir = "",
    [string]$SignTool = "signtool.exe",
    [string]$CertificateThumbprint = $env:CROSSSCP_SIGN_CERT_THUMBPRINT,
    [string]$PfxPath = $env:CROSSSCP_WINDOWS_PFX_PATH,
    [string]$PfxPassword = $env:WINDOWS_CODE_SIGN_CERT_PASSWORD
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ExePath)) {
    $ExePath = Join-Path $BuildDir "apps/crossscp-gui/Release/CrossSCP.exe"
}

if (!(Test-Path $ExePath)) {
    throw "Missing executable: $ExePath. Build first with scripts/package-gui.sh equivalent."
}

$WinDeployQt = if ([string]::IsNullOrWhiteSpace($QtBinDir)) { "windeployqt.exe" } else { Join-Path $QtBinDir "windeployqt.exe" }
& $WinDeployQt $ExePath

$SignTargets = @($ExePath)
$CliPath = Join-Path (Split-Path $ExePath -Parent) "crossscp-cli.exe"
if (Test-Path $CliPath) {
    $SignTargets += $CliPath
}

& "$PSScriptRoot/sign-windows.ps1" -FilePath $SignTargets -SignTool $SignTool -CertificateThumbprint $CertificateThumbprint -PfxPath $PfxPath -PfxPassword $PfxPassword

Write-Host "Windows deployment prepared for $ExePath"
