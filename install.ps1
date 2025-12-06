#!/usr/bin/env pwsh
# wemux Remote Installation Script for Windows
# Usage: irm https://raw.githubusercontent.com/superyngo/wemux/main/install.ps1 | iex

param(
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

# Colors
function Write-Success { Write-Host $args -ForegroundColor Green }
function Write-Info { Write-Host $args -ForegroundColor Cyan }
function Write-Error { Write-Host $args -ForegroundColor Red }
function Write-Warning { Write-Host $args -ForegroundColor Yellow }

# Configuration
$APP_NAME = "wemux"
$REPO = "superyngo/wemux"
$INSTALL_DIR = "$env:LOCALAPPDATA\Programs\$APP_NAME"
$BIN_PATH = "$INSTALL_DIR\$APP_NAME.exe"

function Get-LatestRelease {
    try {
        $apiUrl = "https://api.github.com/repos/$REPO/releases/latest"
        Write-Info "Fetching latest release information..."

        $release = Invoke-RestMethod -Uri $apiUrl -Headers @{
            "User-Agent" = "wemux-installer"
        }

        return $release
    } catch {
        Write-Error "Failed to fetch release information: $_"
        exit 1
    }
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default {
            Write-Warning "Unknown architecture: $arch, defaulting to x86_64"
            return "x86_64"
        }
    }
}

function Install-Wemux {
    Write-Info "=== wemux Installation Script ==="
    Write-Info ""

    # Get latest release
    $release = Get-LatestRelease
    $version = $release.tag_name
    Write-Success "Latest version: $version"

    # Determine architecture
    $arch = Get-Architecture
    Write-Info "Detected architecture: $arch"

    # Find download URL for Windows
    $assetName = "$APP_NAME-windows-$arch.exe"
    $asset = $release.assets | Where-Object { $_.name -eq $assetName }

    if (-not $asset) {
        Write-Error "Could not find Windows release asset"
        Write-Info "Available assets:"
        $release.assets | ForEach-Object { Write-Info "  - $($_.name)" }
        Write-Info ""
        Write-Info "Looking for: $assetName"
        exit 1
    }

    $downloadUrl = $asset.browser_download_url
    Write-Info "Download URL: $downloadUrl"
    Write-Info ""

    # Create installation directory
    if (-not (Test-Path $INSTALL_DIR)) {
        New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
    }

    # Download binary directly
    Write-Info "Downloading $APP_NAME..."

    $ProgressPreference = 'SilentlyContinue'
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $BIN_PATH -UseBasicParsing
        $ProgressPreference = 'Continue'
        Write-Success "Downloaded successfully!"
    } catch {
        $ProgressPreference = 'Continue'
        Write-Error "Download failed: $_"
        exit 1
    }

    Write-Info "Installed to: $INSTALL_DIR"
    Write-Success "Binary installed successfully!"

    # Add to PATH
    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($userPath -notlike "*$INSTALL_DIR*") {
        Write-Info "Adding to user PATH..."
        [Environment]::SetEnvironmentVariable(
            "PATH",
            "$userPath;$INSTALL_DIR",
            "User"
        )
        $env:PATH = "$env:PATH;$INSTALL_DIR"
        Write-Success "Added to PATH"
    } else {
        Write-Info "Already in PATH"
    }

    Write-Info ""
    Write-Success "Installation completed successfully!"
    Write-Info ""
    Write-Info "Installed version: $version"
    Write-Info "Installation path: $BIN_PATH"
    Write-Info ""
    Write-Info "Usage:"
    Write-Info "  $APP_NAME list              - List audio devices"
    Write-Info "  $APP_NAME start             - Start audio sync"
    Write-Info "  $APP_NAME --help            - Show help"
    Write-Info "  $APP_NAME --version         - Show version"
    Write-Info ""
    Write-Warning "Note: You may need to restart your terminal for PATH changes to take effect."
    Write-Info ""
    Write-Info "To uninstall, run:"
    Write-Info "  irm https://raw.githubusercontent.com/$REPO/main/install.ps1 | iex -Uninstall"
}

function Uninstall-Wemux {
    Write-Info "=== wemux Uninstallation Script ==="
    Write-Info ""

    # Remove binary
    if (Test-Path $BIN_PATH) {
        Write-Info "Removing binary..."
        Remove-Item $BIN_PATH -Force
        Write-Success "Binary removed"
    } else {
        Write-Info "Binary not found (already removed?)"
    }

    # Remove installation directory if empty
    if (Test-Path $INSTALL_DIR) {
        $items = Get-ChildItem $INSTALL_DIR -ErrorAction SilentlyContinue
        if ($items.Count -eq 0) {
            Remove-Item $INSTALL_DIR -Force
            Write-Success "Installation directory removed"
        } else {
            Write-Warning "Installation directory not empty, skipping removal"
        }
    }

    # Remove from PATH
    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($userPath -like "*$INSTALL_DIR*") {
        Write-Info "Removing from user PATH..."
        $newPath = ($userPath -split ';' | Where-Object { $_ -ne $INSTALL_DIR }) -join ';'
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        Write-Success "Removed from PATH"
    }

    Write-Info ""
    Write-Success "Uninstallation completed!"
    Write-Warning "Note: You may need to restart your terminal for PATH changes to take effect."
}

# Main
if ($Uninstall) {
    Uninstall-Wemux
} else {
    Install-Wemux
}
