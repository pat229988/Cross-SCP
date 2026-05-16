# SPDX-License-Identifier: AGPL-3.0-or-later

param(
    [string]$BuildDir = "build/gui-release",
    [string]$ExePath = "",
    [string]$QmlDir = "apps/crossscp-gui/qml",
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

$DeployDir = Split-Path $ExePath -Parent
$WinDeployQt = if ([string]::IsNullOrWhiteSpace($QtBinDir)) { "windeployqt.exe" } else { Join-Path $QtBinDir "windeployqt.exe" }
if (!(Get-Command $WinDeployQt -ErrorAction SilentlyContinue) -and !(Test-Path $WinDeployQt)) {
    throw "Unable to find windeployqt at '$WinDeployQt'. Pass -QtBinDir or add Qt bin to PATH."
}

$ResolvedQmlDir = if ([System.IO.Path]::IsPathRooted($QmlDir)) { $QmlDir } else { Join-Path (Get-Location) $QmlDir }
if (!(Test-Path $ResolvedQmlDir)) {
    throw "Missing QML source directory: $ResolvedQmlDir"
}

& $WinDeployQt --release --compiler-runtime --qmldir $ResolvedQmlDir --verbose 1 $ExePath
if ($LASTEXITCODE -ne 0) {
    throw "windeployqt failed with exit code $LASTEXITCODE"
}

$LegalDir = Join-Path $DeployDir "Legal"
New-Item -ItemType Directory -Force -Path $LegalDir | Out-Null
Copy-Item -Path "LICENSE", "THIRD_PARTY_NOTICES.md" -Destination $LegalDir -Force
Copy-Item -Path "LICENSES" -Destination $LegalDir -Recurse -Force

function Copy-IfPresent {
    param([string]$Source, [string]$DestinationDirectory)
    if (Test-Path $Source) {
        Copy-Item -Path $Source -Destination $DestinationDirectory -Force
    }
}

function Copy-MsvcRuntimeFallbacks {
    param([string]$DestinationDirectory)

    $runtimeNames = @(
        "vcruntime140.dll",
        "vcruntime140_1.dll",
        "msvcp140.dll",
        "msvcp140_1.dll",
        "msvcp140_2.dll",
        "concrt140.dll"
    )

    $searchRoots = @()
    if ($env:VCToolsRedistDir) { $searchRoots += $env:VCToolsRedistDir }
    if ($env:VCINSTALLDIR) { $searchRoots += (Join-Path $env:VCINSTALLDIR "Redist") }
    if ($env:ProgramFiles) { $searchRoots += (Join-Path $env:ProgramFiles "Microsoft Visual Studio") }
    if (${env:ProgramFiles(x86)}) { $searchRoots += (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio") }

    foreach ($name in $runtimeNames) {
        if (Test-Path (Join-Path $DestinationDirectory $name)) {
            continue
        }
        $candidate = $null
        foreach ($root in $searchRoots) {
            if ([string]::IsNullOrWhiteSpace($root) -or !(Test-Path $root)) { continue }
            $candidate = Get-ChildItem -Path $root -Recurse -Filter $name -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match "\\x64\\|\\x64$|\\Hostx64\\" } |
                Select-Object -First 1
            if ($candidate) { break }
        }
        if ($candidate) {
            Copy-IfPresent -Source $candidate.FullName -DestinationDirectory $DestinationDirectory
            Write-Host "Copied MSVC runtime fallback: $name"
        } else {
            Write-Host "MSVC runtime not found for fallback copy: $name"
        }
    }
}

Copy-MsvcRuntimeFallbacks -DestinationDirectory $DeployDir

function Assert-DeployedFile {
    param([string]$RelativePath)
    $path = Join-Path $DeployDir $RelativePath
    if (!(Test-Path $path)) {
        throw "Missing deployed Windows runtime file: $RelativePath in $DeployDir"
    }
}

@(
    "CrossSCP.exe",
    "Qt6Core.dll",
    "Qt6Gui.dll",
    "Qt6Qml.dll",
    "Qt6Quick.dll",
    "vcruntime140.dll",
    "msvcp140.dll",
    "Legal\THIRD_PARTY_NOTICES.md",
    "Legal\LICENSES\QT-LGPL-COMPLIANCE.md",
    "platforms\qwindows.dll"
) | ForEach-Object { Assert-DeployedFile $_ }

if (!(Test-Path (Join-Path $DeployDir "Qt6QmlModels.dll"))) {
    Write-Host "Qt6QmlModels.dll not found at top level; this may be okay depending on Qt deployment output."
}

$SignTargets = @($ExePath)
$CliPath = Join-Path $DeployDir "crossscp-cli.exe"
if (Test-Path $CliPath) {
    $SignTargets += $CliPath
} else {
    throw "Missing deployed CLI bridge: $CliPath"
}

& "$PSScriptRoot/sign-windows.ps1" -FilePath $SignTargets -SignTool $SignTool -CertificateThumbprint $CertificateThumbprint -PfxPath $PfxPath -PfxPassword $PfxPassword

Write-Host "Windows deployment prepared for $ExePath"
