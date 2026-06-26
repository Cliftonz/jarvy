# Jarvy Installer Script for Windows
# Usage: irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
#
# Environment variables:
#   JARVY_VERSION     - Version to install (default: latest)
#   JARVY_CHANNEL     - Release channel: stable (default), beta, nightly
#                       beta accepts -rc.N and -beta.N tags
#                       nightly accepts every tag including -alpha.N
#   JARVY_INSTALL_DIR - Installation directory (default: $env:LOCALAPPDATA\Programs\jarvy)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$JarvyVersion = if ($env:JARVY_VERSION) { $env:JARVY_VERSION } else { "latest" }
$JarvyChannel = if ($env:JARVY_CHANNEL) { $env:JARVY_CHANNEL } else { "stable" }
$InstallDir = if ($env:JARVY_INSTALL_DIR) { $env:JARVY_INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\jarvy" }
$JarvyRepo = "Cliftonz/jarvy"

if ($JarvyChannel -notin @('stable', 'beta', 'nightly')) {
    Write-Host "[ERROR] Unknown JARVY_CHANNEL '$JarvyChannel'. Expected: stable, beta, nightly." -ForegroundColor Red
    exit 1
}

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Err {
    param([string]$Message)
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Test-ChannelMatch {
    param([string]$Tag)
    switch ($JarvyChannel) {
        'stable'  { return $Tag -notmatch '-' }
        'beta'    { return ($Tag -notmatch '-') -or ($Tag -match '-rc\.') -or ($Tag -match '-beta\.') }
        'nightly' { return $true }
    }
    return $false
}

function Get-LatestVersion {
    try {
        if ($JarvyChannel -eq 'stable') {
            $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$JarvyRepo/releases/latest" -Method Get
            return $response.tag_name -replace '^v', ''
        }

        # beta or nightly: walk recent releases (newest first) and pick the
        # first one that matches the channel.
        $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$JarvyRepo/releases?per_page=30" -Method Get
        foreach ($release in $releases) {
            if ($release.draft) { continue }
            if (Test-ChannelMatch -Tag $release.tag_name) {
                return $release.tag_name -replace '^v', ''
            }
        }
        Write-Err "No release matching channel '$JarvyChannel' in the most recent 30 releases"
        exit 1
    }
    catch {
        Write-Err "Failed to fetch latest version: $_"
        exit 1
    }
}

function Test-Checksum {
    param(
        [string]$FilePath,
        [string]$ExpectedHash
    )

    $actualHash = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()

    if ($actualHash -ne $ExpectedHash.ToLower()) {
        Write-Err "Checksum verification failed!"
        Write-Err "Expected: $ExpectedHash"
        Write-Err "Actual:   $actualHash"
        return $false
    }

    Write-Info "Checksum verified"
    return $true
}

function Add-ToPath {
    param([string]$Directory)

    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")

    if ($userPath -notlike "*$Directory*") {
        $newPath = "$userPath;$Directory"
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        Write-Info "Added $Directory to user PATH"
        return $true
    }

    return $false
}

function Install-Jarvy {
    Write-Host ""
    Write-Host "Jarvy Installer for Windows" -ForegroundColor Cyan
    Write-Host ""

    # Get version
    $version = $JarvyVersion
    if ($version -eq "latest") {
        Write-Info "Channel: $JarvyChannel"
        Write-Info "Fetching latest version on '$JarvyChannel' channel..."
        $version = Get-LatestVersion
    }
    else {
        $version = $version -replace '^v', ''
    }

    Write-Info "Installing version: v$version"

    # Build download URL
    $platform = "x86_64-pc-windows-msvc"
    $url = "https://github.com/$JarvyRepo/releases/download/v$version/jarvy-v$version-$platform.zip"
    Write-Info "Download URL: $url"

    # Create temporary directory
    $tempDir = Join-Path $env:TEMP "jarvy-install-$(Get-Random)"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

    try {
        # Download
        Write-Info "Downloading..."
        $zipPath = Join-Path $tempDir "jarvy.zip"
        Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

        # Extract
        Write-Info "Extracting..."
        Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

        # Install
        Write-Info "Installing to $InstallDir..."
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

        # Find the executable
        $jarvy = Get-ChildItem -Path $tempDir -Filter "jarvy.exe" -Recurse | Select-Object -First 1
        if (-not $jarvy) {
            Write-Err "jarvy.exe not found in archive"
            exit 1
        }

        Copy-Item -Path $jarvy.FullName -Destination (Join-Path $InstallDir "jarvy.exe") -Force

        Write-Success "Jarvy v$version installed to $InstallDir\jarvy.exe"

        # Add to PATH
        $pathUpdated = Add-ToPath -Directory $InstallDir

        Write-Host ""

        if ($pathUpdated) {
            Write-Warn "PATH updated. Please restart your terminal for changes to take effect."
        }
        elseif (-not (Get-Command jarvy -ErrorAction SilentlyContinue)) {
            Write-Info "Add the following to your PATH:"
            Write-Host "    $InstallDir"
        }

        Write-Host ""
        Write-Success "Installation complete!"
        Write-Host ""
        Write-Host "Get started:"
        Write-Host "    jarvy --help      # Show help"
        Write-Host "    jarvy configure   # Create jarvy.toml"
        Write-Host "    jarvy setup       # Install tools"
        Write-Host ""
    }
    finally {
        # Cleanup
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Run installer
Install-Jarvy
