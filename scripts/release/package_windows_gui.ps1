param(
    [Parameter(Mandatory = $true)]
    [string] $BinDir,

    [Parameter(Mandatory = $true)]
    [string] $OutputDir,

    [Parameter(Mandatory = $true)]
    [string] $Version,

    [Parameter(Mandatory = $true)]
    [ValidateSet("windows-x86_64")]
    [string] $Platform
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$') {
    throw "Invalid semantic version: $Version"
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ReleaseRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
$BinDir = (Resolve-Path $BinDir).Path
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$OutputDir = (Resolve-Path $OutputDir).Path

foreach ($Binary in @("parano1d-gui.exe", "parano1d.exe")) {
    $Path = Join-Path $BinDir $Binary
    if (-not (Test-Path -Path $Path -PathType Leaf)) {
        throw "Release binary is missing: $Path"
    }
}

$CompilerCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe")
)
$Compiler = $CompilerCandidates |
    Where-Object { $_ -and (Test-Path -Path $_ -PathType Leaf) } |
    Select-Object -First 1
if (-not $Compiler) {
    throw "Inno Setup 6 compiler was not found"
}

$Definition = Join-Path $ScriptDir "gui\windows\Parano1d.iss"
$Icon = Join-Path $ReleaseRoot "noid_gui\assets\app-icons\Parano1d.ico"
$License = Join-Path $ReleaseRoot "LICENSE"
$Notice = Join-Path $ReleaseRoot "NOTICE"
$UserGuide = Join-Path $ScriptDir "README.txt"
$ContractGuide = Join-Path $ReleaseRoot "docs\reference\contracts.md"
$OutputBaseFilename = "parano1d-gui-v$Version-$Platform-setup"
$NumericVersion = [regex]::Match($Version, '^([0-9]+\.[0-9]+\.[0-9]+)').Groups[1].Value

& $Compiler `
    "/DMyAppVersion=$Version" `
    "/DNumericVersion=$NumericVersion" `
    "/DSourceDir=$BinDir" `
    "/DOutputDir=$OutputDir" `
    "/DOutputBaseFilename=$OutputBaseFilename" `
    "/DIconFile=$Icon" `
    "/DLicenseFile=$License" `
    "/DNoticeFile=$Notice" `
    "/DUserGuideFile=$UserGuide" `
    "/DContractGuideFile=$ContractGuide" `
    $Definition
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup exited with code $LASTEXITCODE"
}

$Artifact = Join-Path $OutputDir "$OutputBaseFilename.exe"
if (-not (Test-Path -Path $Artifact -PathType Leaf)) {
    throw "Windows GUI installer was not created: $Artifact"
}

$InstallDir = Join-Path ([System.IO.Path]::GetTempPath()) (
    "Parano1d-release-smoke-" + [guid]::NewGuid().ToString("N")
)
try {
    $Install = Start-Process `
        -FilePath $Artifact `
        -ArgumentList @(
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/NOICONS",
            "/CURRENTUSER",
            "/DIR=$InstallDir"
        ) `
        -Wait `
        -PassThru
    if ($Install.ExitCode -ne 0) {
        throw "GUI installer smoke-test exited with code $($Install.ExitCode)"
    }

    $Wallet = Join-Path $InstallDir "Parano1d.exe"
    $Node = Join-Path $InstallDir "parano1d-node.exe"
    $InstalledLicense = Join-Path $InstallDir "LICENSE.txt"
    $InstalledNotice = Join-Path $InstallDir "NOTICE.txt"
    $InstalledGuide = Join-Path $InstallDir "README.txt"
    $InstalledContracts = Join-Path $InstallDir "CONTRACTS.md"
    if (-not (Test-Path -Path $Wallet -PathType Leaf) -or
        -not (Test-Path -Path $Node -PathType Leaf) -or
        -not (Test-Path -Path $InstalledLicense -PathType Leaf) -or
        -not (Test-Path -Path $InstalledNotice -PathType Leaf) -or
        -not (Test-Path -Path $InstalledGuide -PathType Leaf) -or
        -not (Test-Path -Path $InstalledContracts -PathType Leaf)) {
        throw "GUI installer payload is incomplete"
    }
    foreach ($ForbiddenBinary in @("parano1d-cli.exe", "parano1d-miner.exe")) {
        if (Test-Path -Path (Join-Path $InstallDir $ForbiddenBinary)) {
            throw "GUI installer contains forbidden operator tool: $ForbiddenBinary"
        }
    }
    $Check = Start-Process `
        -FilePath $Wallet `
        -ArgumentList "--release-self-check" `
        -Wait `
        -PassThru
    if ($Check.ExitCode -ne 0) {
        throw "Installed GUI wallet self-check exited with code $($Check.ExitCode)"
    }
    $NodeCheck = Start-Process `
        -FilePath $Node `
        -ArgumentList "--check-hardware" `
        -Wait `
        -PassThru
    if ($NodeCheck.ExitCode -ne 0) {
        throw "Installed node self-check exited with code $($NodeCheck.ExitCode)"
    }

    $Uninstaller = Join-Path $InstallDir "unins000.exe"
    $Uninstall = Start-Process `
        -FilePath $Uninstaller `
        -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART") `
        -Wait `
        -PassThru
    if ($Uninstall.ExitCode -ne 0) {
        throw "GUI uninstaller smoke-test exited with code $($Uninstall.ExitCode)"
    }
}
finally {
    if (Test-Path $InstallDir) {
        Remove-Item -Recurse -Force $InstallDir
    }
}

Write-Output $Artifact
