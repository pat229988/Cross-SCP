# SPDX-License-Identifier: AGPL-3.0-or-later

param(
    [string]$BuildDir = "build/gui-release",
    [string]$ExePath = "",
    [string]$QtBinDir = "",
    [string]$SignTool = "signtool.exe",
    [string]$CertificateThumbprint = $env:CROSSSCP_SIGN_CERT_THUMBPRINT
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

if (![string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
    & $SignTool sign /sha1 $CertificateThumbprint /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $ExePath
} else {
    Write-Host "Skipping Authenticode signing: CROSSSCP_SIGN_CERT_THUMBPRINT is not set."
}

Write-Host "Windows deployment prepared for $ExePath"
