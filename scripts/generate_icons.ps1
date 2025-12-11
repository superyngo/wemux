# PowerShell Script to Generate MSIX Icon Assets
# Requires ImageMagick (https://imagemagick.org/script/download.php)
#
# Usage:
#   .\scripts\generate_icons.ps1
#
# This script generates all required MSIX icon sizes from the existing tray icon

param(
    [string]$SourceIcon = "assets\icons\tray\idle.png",
    [string]$OutputDir = "packaging\assets"
)

$ErrorActionPreference = "Stop"

Write-Host "=== wemux MSIX Icon Generator ===" -ForegroundColor Cyan
Write-Host ""

# Check if ImageMagick is installed
try {
    $magickVersion = & magick --version 2>&1 | Select-Object -First 1
    Write-Host "✓ ImageMagick found: $magickVersion" -ForegroundColor Green
} catch {
    Write-Host "✗ ImageMagick not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install ImageMagick from:" -ForegroundColor Yellow
    Write-Host "  https://imagemagick.org/script/download.php" -ForegroundColor White
    Write-Host ""
    Write-Host "Or install via winget:" -ForegroundColor Yellow
    Write-Host "  winget install ImageMagick.ImageMagick" -ForegroundColor White
    exit 1
}

# Check if source icon exists
if (-not (Test-Path $SourceIcon)) {
    Write-Host "✗ Source icon not found: $SourceIcon" -ForegroundColor Red
    exit 1
}

Write-Host "✓ Source icon: $SourceIcon" -ForegroundColor Green

# Create output directory
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
Write-Host "✓ Output directory: $OutputDir" -ForegroundColor Green
Write-Host ""

# Define required icon sizes
$iconSizes = @{
    "Square44x44Logo.png" = @{ Width = 44; Height = 44; Description = "App list, taskbar" }
    "Square71x71Logo.png" = @{ Width = 71; Height = 71; Description = "Small tile" }
    "Square150x150Logo.png" = @{ Width = 150; Height = 150; Description = "Medium tile" }
    "Square310x310Logo.png" = @{ Width = 310; Height = 310; Description = "Large tile" }
    "StoreLogo.png" = @{ Width = 50; Height = 50; Description = "Microsoft Store" }
}

# Generate square icons
Write-Host "Generating square icons..." -ForegroundColor Yellow
foreach ($filename in $iconSizes.Keys | Sort-Object) {
    $size = $iconSizes[$filename]
    $outputPath = Join-Path $OutputDir $filename

    Write-Host "  → $filename ($($size.Width)x$($size.Height)) - $($size.Description)" -ForegroundColor White

    # Use ImageMagick to resize with proper scaling
    & magick convert $SourceIcon `
        -resize "$($size.Width)x$($size.Height)" `
        -background transparent `
        -gravity center `
        -extent "$($size.Width)x$($size.Height)" `
        $outputPath

    if ($LASTEXITCODE -ne 0) {
        Write-Host "    ✗ Failed to generate $filename" -ForegroundColor Red
        exit 1
    }

    Write-Host "    ✓ Created $filename" -ForegroundColor Green
}

# Generate wide tile (different aspect ratio)
Write-Host ""
Write-Host "Generating wide tile..." -ForegroundColor Yellow
$wideOutput = Join-Path $OutputDir "Wide310x150Logo.png"
Write-Host "  → Wide310x150Logo.png (310x150) - Wide tile" -ForegroundColor White

& magick convert $SourceIcon `
    -resize "150x150" `
    -background transparent `
    -gravity center `
    -extent "310x150" `
    $wideOutput

if ($LASTEXITCODE -ne 0) {
    Write-Host "    ✗ Failed to generate wide tile" -ForegroundColor Red
    exit 1
}

Write-Host "    ✓ Created Wide310x150Logo.png" -ForegroundColor Green

# Generate splash screen (optional)
Write-Host ""
Write-Host "Generating splash screen..." -ForegroundColor Yellow
$splashOutput = Join-Path $OutputDir "SplashScreen.png"
Write-Host "  → SplashScreen.png (620x300) - Launch screen" -ForegroundColor White

& magick convert $SourceIcon `
    -resize "300x300" `
    -background transparent `
    -gravity center `
    -extent "620x300" `
    $splashOutput

if ($LASTEXITCODE -ne 0) {
    Write-Host "    ✗ Failed to generate splash screen" -ForegroundColor Red
    exit 1
}

Write-Host "    ✓ Created SplashScreen.png" -ForegroundColor Green

# Summary
Write-Host ""
Write-Host "=== Icon Generation Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Generated icons:" -ForegroundColor Cyan
Get-ChildItem -Path $OutputDir -Filter "*.png" | ForEach-Object {
    $sizeKB = [math]::Round($_.Length / 1KB, 2)
    Write-Host "  • $($_.Name) ($sizeKB KB)" -ForegroundColor White
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Review generated icons in: $OutputDir" -ForegroundColor White
Write-Host "  2. Optionally edit icons for better appearance" -ForegroundColor White
Write-Host "  3. Run build_msix.ps1 to create MSIX package" -ForegroundColor White
Write-Host ""
