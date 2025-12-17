# MSIX Build Script for wemux
# Creates a packaged MSIX file ready for Microsoft Store submission or local testing
#
# Usage:
#   .\scripts\build_msix.ps1                    # Build with default version from Cargo.toml
#   .\scripts\build_msix.ps1 -Version 0.3.1.0   # Build with specific version
#   .\scripts\build_msix.ps1 -SkipBuild         # Skip Rust build (use existing executable)
#   .\scripts\build_msix.ps1 -Sign              # Sign the package (requires certificate)
#
# Requirements:
#   - Windows 10 SDK (for makeappx.exe and signtool.exe)
#   - Rust toolchain
#   - Icon assets in packaging/assets/ (run generate_icons.ps1 first)

param(
    [string]$Version = "",
    [string]$Configuration = "Release",
    [switch]$SkipBuild = $false,
    [switch]$Sign = $false,
    [string]$CertPath = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

Write-Host ""
Write-Host "=== wemux MSIX Package Builder ===" -ForegroundColor Cyan
Write-Host ""

# Paths
$projectRoot = Split-Path -Parent $PSScriptRoot
$packagingDir = Join-Path $projectRoot "packaging"
$stagingDir = Join-Path $packagingDir "staging"
$outputDir = Join-Path $packagingDir "output"
$manifestTemplate = Join-Path $packagingDir "AppxManifest.xml"

# Detect version from Cargo.toml if not specified
if ([string]::IsNullOrEmpty($Version)) {
    Write-Host "Detecting version from Cargo.toml..." -ForegroundColor Yellow
    $cargoToml = Join-Path $projectRoot "Cargo.toml"
    $versionMatch = Select-String -Path $cargoToml -Pattern 'version\s*=\s*"([^"]+)"' | Select-Object -First 1

    if ($versionMatch) {
        $cargoVersion = $versionMatch.Matches[0].Groups[1].Value
        $Version = "$cargoVersion.0"  # Convert 0.3.0 to 0.3.0.0
        Write-Host "  ✓ Detected version: $Version" -ForegroundColor Green
    } else {
        Write-Host "  ✗ Could not detect version, using default: 0.3.0.0" -ForegroundColor Yellow
        $Version = "0.3.0.0"
    }
}

Write-Host ""
Write-Host "Build Configuration:" -ForegroundColor Cyan
Write-Host "  Version: $Version" -ForegroundColor White
Write-Host "  Configuration: $Configuration" -ForegroundColor White
Write-Host "  Skip Rust Build: $SkipBuild" -ForegroundColor White
Write-Host "  Sign Package: $Sign" -ForegroundColor White
Write-Host ""

# Validate prerequisites
Write-Host "Checking prerequisites..." -ForegroundColor Yellow

# Check for makeappx.exe
try {
    $makeappx = Get-Command makeappx.exe -ErrorAction Stop
    Write-Host "  ✓ makeappx.exe found: $($makeappx.Source)" -ForegroundColor Green
} catch {
    Write-Host "  ✗ makeappx.exe not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install Windows 10 SDK:" -ForegroundColor Yellow
    Write-Host "  https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/" -ForegroundColor White
    exit 1
}

# Check for manifest template
if (-not (Test-Path $manifestTemplate)) {
    Write-Host "  ✗ AppxManifest.xml not found at: $manifestTemplate" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please ensure packaging/AppxManifest.xml exists." -ForegroundColor Yellow
    exit 1
}
Write-Host "  ✓ AppxManifest.xml found" -ForegroundColor Green

# Check for icon assets
$requiredIcons = @(
    "Square44x44Logo.png",
    "Square71x71Logo.png",
    "Square150x150Logo.png",
    "Square310x310Logo.png",
    "Wide310x150Logo.png",
    "StoreLogo.png",
    "SplashScreen.png"
)

$missingIcons = @()
foreach ($icon in $requiredIcons) {
    $iconPath = Join-Path $packagingDir "assets\$icon"
    if (-not (Test-Path $iconPath)) {
        $missingIcons += $icon
    }
}

if ($missingIcons.Count -gt 0) {
    Write-Host "  ✗ Missing icon assets:" -ForegroundColor Red
    $missingIcons | ForEach-Object { Write-Host "    - $_" -ForegroundColor Yellow }
    Write-Host ""
    Write-Host "Please run generate_icons.ps1 first:" -ForegroundColor Yellow
    Write-Host "  .\scripts\generate_icons.ps1" -ForegroundColor White
    exit 1
}
Write-Host "  ✓ All icon assets found" -ForegroundColor Green

Write-Host ""

# Clean and create staging directory
Write-Host "Preparing staging directory..." -ForegroundColor Yellow
if (Test-Path $stagingDir) {
    Remove-Item -Recurse -Force $stagingDir
}
New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
Write-Host "  ✓ Staging directory ready: $stagingDir" -ForegroundColor Green

# Step 1: Build Rust application
if (-not $SkipBuild) {
    Write-Host ""
    Write-Host "Step 1/6: Building Rust application..." -ForegroundColor Cyan
    Push-Location $projectRoot

    $buildCmd = if ($Configuration -eq "Release") { "cargo build --release" } else { "cargo build" }
    Write-Host "  Running: $buildCmd" -ForegroundColor Gray

    Invoke-Expression $buildCmd
    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        Write-Host "  ✗ Cargo build failed" -ForegroundColor Red
        exit 1
    }

    Pop-Location
    Write-Host "  ✓ Build complete" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "Step 1/6: Skipping Rust build (using existing executable)" -ForegroundColor Cyan
}

# Step 2: Copy executable
Write-Host ""
Write-Host "Step 2/6: Copying executable..." -ForegroundColor Cyan
$exePath = if ($Configuration -eq "Release") {
    Join-Path $projectRoot "target\release\wemux.exe"
} else {
    Join-Path $projectRoot "target\debug\wemux.exe"
}

if (-not (Test-Path $exePath)) {
    Write-Host "  ✗ Executable not found: $exePath" -ForegroundColor Red
    exit 1
}

Copy-Item $exePath $stagingDir
$exeSize = [math]::Round((Get-Item $exePath).Length / 1MB, 2)
Write-Host "  ✓ Copied wemux.exe ($exeSize MB)" -ForegroundColor Green

# Step 3: Copy assets
Write-Host ""
Write-Host "Step 3/6: Copying assets..." -ForegroundColor Cyan
$assetsDir = Join-Path $stagingDir "assets"
New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null

# Copy tray icons (if they exist)
$trayIconsDir = Join-Path $projectRoot "assets\icons\tray"
if (Test-Path $trayIconsDir) {
    Copy-Item "$trayIconsDir\*.png" $assetsDir -ErrorAction SilentlyContinue
}

# Copy MSIX-specific icons
$msixIconsDir = Join-Path $packagingDir "assets"
Copy-Item "$msixIconsDir\*.png" $assetsDir -Force

$iconCount = (Get-ChildItem -Path $assetsDir -Filter "*.png").Count
Write-Host "  ✓ Copied $iconCount icon files" -ForegroundColor Green

# Step 4: Update manifest version
Write-Host ""
Write-Host "Step 4/6: Updating manifest version..." -ForegroundColor Cyan
$manifestContent = Get-Content $manifestTemplate -Raw
# Match only the Version attribute in the Identity element (case-sensitive, with surrounding context)
$manifestContent = $manifestContent -replace '(<Identity[^>]+Version=)"[\d\.]+"', "`$1`"$Version`""
$manifestPath = Join-Path $stagingDir "AppxManifest.xml"
Set-Content -Path $manifestPath -Value $manifestContent -Encoding UTF8
Write-Host "  ✓ Manifest version set to: $Version" -ForegroundColor Green

# Step 5: Create MSIX package
Write-Host ""
Write-Host "Step 5/6: Creating MSIX package..." -ForegroundColor Cyan
$versionShort = $Version -replace '\.0$', ''  # Remove trailing .0 for filename
$msixPath = Join-Path $outputDir "wemux_${versionShort}_x64.msix"

Write-Host "  Running makeappx.exe..." -ForegroundColor Gray
& makeappx.exe pack /d $stagingDir /p $msixPath /o

if ($LASTEXITCODE -ne 0) {
    Write-Host "  ✗ makeappx.exe failed with exit code $LASTEXITCODE" -ForegroundColor Red
    exit 1
}

$msixSize = [math]::Round((Get-Item $msixPath).Length / 1MB, 2)
Write-Host "  ✓ MSIX package created: $msixPath ($msixSize MB)" -ForegroundColor Green

# Step 6: Sign package (optional)
Write-Host ""
if ($Sign) {
    Write-Host "Step 6/6: Signing package..." -ForegroundColor Cyan

    # Default certificate path
    if ([string]::IsNullOrEmpty($CertPath)) {
        $CertPath = Join-Path $packagingDir "wemux_cert.pfx"
    }

    if (-not (Test-Path $CertPath)) {
        Write-Host "  ✗ Certificate not found: $CertPath" -ForegroundColor Red
        Write-Host ""
        Write-Host "For local testing, generate a self-signed certificate:" -ForegroundColor Yellow
        Write-Host '  $cert = New-SelfSignedCertificate -Type Custom -Subject "CN=wemux Dev" `' -ForegroundColor White
        Write-Host '      -KeyUsage DigitalSignature -FriendlyName "wemux Dev Certificate" `' -ForegroundColor White
        Write-Host '      -CertStoreLocation "Cert:\CurrentUser\My"' -ForegroundColor White
        Write-Host '  Export-PfxCertificate -Cert $cert -FilePath "packaging\wemux_cert.pfx" -Password (ConvertTo-SecureString -String "password" -Force -AsPlainText)' -ForegroundColor White
        exit 1
    }

    try {
        $signtool = Get-Command signtool.exe -ErrorAction Stop
        Write-Host "  Running signtool.exe..." -ForegroundColor Gray

        # Prompt for certificate password
        $certPassword = Read-Host "Enter certificate password" -AsSecureString
        $certPasswordPlain = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($certPassword))

        & signtool.exe sign /fd SHA256 /a /f $CertPath /p $certPasswordPlain $msixPath

        if ($LASTEXITCODE -ne 0) {
            Write-Host "  ✗ Package signing failed with exit code $LASTEXITCODE" -ForegroundColor Red
            exit 1
        }

        Write-Host "  ✓ Package signed successfully" -ForegroundColor Green
    } catch {
        Write-Host "  ✗ signtool.exe not found (part of Windows SDK)" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "Step 6/6: Skipping signing" -ForegroundColor Cyan
    Write-Host "  For Microsoft Store submission, signing will be done automatically by Microsoft" -ForegroundColor Gray
}

# Validate package
Write-Host ""
Write-Host "Validating package..." -ForegroundColor Yellow
& makeappx.exe validate /p $msixPath

if ($LASTEXITCODE -ne 0) {
    Write-Host "  ⚠ Package validation reported warnings (may still be usable)" -ForegroundColor Yellow
} else {
    Write-Host "  ✓ Package validation passed" -ForegroundColor Green
}

# Summary
Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Package Details:" -ForegroundColor Cyan
Write-Host "  File: $msixPath" -ForegroundColor White
Write-Host "  Size: $msixSize MB" -ForegroundColor White
Write-Host "  Version: $Version" -ForegroundColor White
Write-Host "  Signed: $(if ($Sign) { 'Yes' } else { 'No' })" -ForegroundColor White
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  • For local testing:" -ForegroundColor White
Write-Host "      Add-AppxPackage -Path `"$msixPath`"" -ForegroundColor Gray
Write-Host "  • For Microsoft Store:" -ForegroundColor White
Write-Host "      Upload to Partner Center at https://partner.microsoft.com/dashboard" -ForegroundColor Gray
Write-Host ""
