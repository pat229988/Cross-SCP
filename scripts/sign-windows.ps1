# SPDX-License-Identifier: AGPL-3.0-or-later

param(
    [Parameter(Mandatory = $true)]
    [string[]]$FilePath,
    [string]$SignTool = "signtool.exe",
    [string]$CertificateThumbprint = $env:CROSSSCP_SIGN_CERT_THUMBPRINT,
    [string]$PfxPath = $env:CROSSSCP_WINDOWS_PFX_PATH,
    [string]$PfxPassword = $env:WINDOWS_CODE_SIGN_CERT_PASSWORD
)

$ErrorActionPreference = "Stop"

function Invoke-SignTool {
    param([string]$Path)

    if (!(Test-Path $Path)) {
        throw "Cannot sign missing file: $Path"
    }

    if (![string]::IsNullOrWhiteSpace($PfxPath)) {
        if ([string]::IsNullOrWhiteSpace($PfxPassword)) {
            throw "WINDOWS_CODE_SIGN_CERT_PASSWORD is required when CROSSSCP_WINDOWS_PFX_PATH is set."
        }
        & $SignTool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $PfxPath /p $PfxPassword $Path
    } elseif (![string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
        & $SignTool sign /sha1 $CertificateThumbprint /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $Path
    } else {
        Write-Host "Skipping Authenticode signing for $Path: no Windows signing certificate is configured."
        return
    }

    & $SignTool verify /pa /v $Path
}

foreach ($Path in $FilePath) {
    Invoke-SignTool -Path $Path
}
