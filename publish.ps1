param (
    [string]$Version = "0.1.0",
    [string]$Title = "SyncDrop v0.1.0",
    [string]$Notes = "Official production release of SyncDrop: LAN file transfer, synchronized watch rooms, and offline Windows setup.",
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== SyncDrop Terminal Release Publisher ===" -ForegroundColor Cyan

# 1. Check if gh CLI is available
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Error "GitHub CLI ('gh') is not installed. Please install it with: winget install GitHub.cli"
    exit 1
}

# 2. Check GitHub CLI authentication
Write-Host "Checking GitHub authentication..." -ForegroundColor Yellow
$authCheck = gh auth status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nYou are not logged into GitHub CLI yet." -ForegroundColor Yellow
    Write-Host "Opening GitHub login in your browser..." -ForegroundColor Green
    gh auth login --web --hostname github.com
    if ($LASTEXITCODE -ne 0) {
        Write-Error "GitHub authentication cancelled or failed."
        exit 1
    }
}
Write-Host "GitHub CLI authenticated successfully.`n" -ForegroundColor Green

$installerPath = "release\SyncDrop_${Version}_x64-setup.exe"
$buildArtifact = "src-tauri\target\release\bundle\nsis\SyncDrop_${Version}_x64-setup.exe"

# 3. Build application if not skipped
if (-not $SkipBuild) {
    Write-Host "Building production installer (npm run tauri build)..." -ForegroundColor Yellow
    npm run tauri build
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Tauri build failed."
        exit 1
    }

    if (Test-Path $buildArtifact) {
        if (-not (Test-Path "release")) {
            New-Item -ItemType Directory -Path "release" -Force | Out-Null
        }
        Copy-Item -Path $buildArtifact -Destination $installerPath -Force
        Write-Host "Copied installer to: $installerPath" -ForegroundColor Green
    } else {
        Write-Error "Build finished but could not locate: $buildArtifact"
        exit 1
    }
} else {
    Write-Host "Skipping build as requested (-SkipBuild)." -ForegroundColor Yellow
    if (-not (Test-Path $installerPath)) {
        if (Test-Path $buildArtifact) {
            Copy-Item -Path $buildArtifact -Destination $installerPath -Force
        } else {
            Write-Error "Installer not found at $installerPath. Run without -SkipBuild."
            exit 1
        }
    }
}

$fileItem = Get-Item $installerPath
$fileSizeMb = [math]::Round($fileItem.Length / 1MB, 2)
Write-Host "Ready to publish: $($fileItem.Name) ($fileSizeMb MB)" -ForegroundColor Green

# 4. Push git commits & tags
Write-Host "`nSyncing Git commits & tags with GitHub..." -ForegroundColor Yellow
$tag = "v$Version"
$existingTag = git tag -l $tag
if (-not $existingTag) {
    git tag $tag
    Write-Host "Created Git tag: $tag" -ForegroundColor Green
}

git push origin main --tags
if ($LASTEXITCODE -ne 0) {
    Write-Host "Warning: git push returned non-zero code. Continuing to release upload..." -ForegroundColor DarkYellow
}

# 5. Create or update the GitHub Release using gh CLI
Write-Host "`nPublishing release '$tag' to GitHub..." -ForegroundColor Yellow
gh release create $tag $installerPath --title $Title --notes $Notes --clobber

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n=======================================================" -ForegroundColor Green
    Write-Host " SUCCESS: Release $tag published successfully to GitHub!" -ForegroundColor Green
    Write-Host " View your release at: https://github.com/trishulreddy3/syncdrop/releases/tag/$tag" -ForegroundColor Cyan
    Write-Host "=======================================================`n" -ForegroundColor Green
} else {
    Write-Error "gh release create failed."
}
