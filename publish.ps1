param (
    [string]$Version = "",
    [string]$Title = "",
    [string]$Notes = "Official production release of SyncDrop with automated LAN sync, audio pitch protection, and in-app updater system.",
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== SyncDrop Terminal Release Publisher ===" -ForegroundColor Cyan

# 1. Determine release version
if (-not $Version) {
    $pkgJson = Get-Content "package.json" -Raw | ConvertFrom-Json
    $Version = $pkgJson.version
}
$Version = $Version.Trim().TrimStart('v')

if (-not $Title) {
    $Title = "SyncDrop v$Version"
}

Write-Host "Target Release Version: v$Version" -ForegroundColor Green

# 2. Check if gh CLI is available
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Error "GitHub CLI ('gh') is not installed. Please install it with: winget install GitHub.cli"
    exit 1
}

# 3. Check GitHub CLI authentication
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

# 4. Sync version across project configuration files
Write-Host "Updating project configuration files to version $Version..." -ForegroundColor Yellow

# Update package.json
(Get-Content "package.json") -replace '"version":\s*"[^"]+"', ('"version": "' + $Version + '"') | Set-Content "package.json"

# Update src-tauri/tauri.conf.json
(Get-Content "src-tauri\tauri.conf.json") -replace '"version":\s*"[^"]+"', ('"version": "' + $Version + '"') | Set-Content "src-tauri\tauri.conf.json"

# Update src-tauri/Cargo.toml (first package version)
$cargoContent = Get-Content "src-tauri\Cargo.toml"
$updatedCargo = @()
$foundPackageVersion = $false
foreach ($line in $cargoContent) {
    if (-not $foundPackageVersion -and $line -match '^version\s*=\s*"[^"]+"') {
        $updatedCargo += ('version = "' + $Version + '"')
        $foundPackageVersion = $true
    } else {
        $updatedCargo += $line
    }
}
$updatedCargo | Set-Content "src-tauri\Cargo.toml"

# 5. Build application if not skipped
if (-not $SkipBuild) {
    Write-Host "Building production installer (npm run tauri build)..." -ForegroundColor Yellow
    npm run tauri build
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Tauri build failed."
        exit 1
    }
} else {
    Write-Host "Skipping build as requested (-SkipBuild)." -ForegroundColor Yellow
}

# Locate the generated NSIS installer dynamically
$builtExe = Get-ChildItem "src-tauri\target\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $builtExe) {
    Write-Error "Could not find any generated NSIS setup executable in src-tauri\target\release\bundle\nsis\"
    exit 1
}

if (-not (Test-Path "release")) {
    New-Item -ItemType Directory -Path "release" -Force | Out-Null
}

$installerPath = "release\SyncDrop_${Version}_x64-setup.exe"
Copy-Item -Path $builtExe.FullName -Destination $installerPath -Force
Write-Host "Installer ready at: $installerPath" -ForegroundColor Green

$fileItem = Get-Item $installerPath
$fileSizeMb = [math]::Round($fileItem.Length / 1MB, 2)
Write-Host "File size: $($fileItem.Name) ($fileSizeMb MB)" -ForegroundColor Green

# 6. Commit version bump if configuration files changed
$status = git status --porcelain package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
if ($status) {
    Write-Host "`nCommitting version bump to Git..." -ForegroundColor Yellow
    git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
    git commit -m "chore(release): bump version to $Version"
}

# 7. Push git commits & tags
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

# 8. Create or update the GitHub Release using gh CLI
Write-Host "`nPublishing release '$tag' to GitHub..." -ForegroundColor Yellow

gh release create $tag $installerPath --title $Title --notes $Notes
if ($LASTEXITCODE -ne 0) {
    Write-Host "Release '$tag' already exists, updating asset with --clobber..." -ForegroundColor Yellow
    gh release upload $tag $installerPath --clobber
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n=======================================================" -ForegroundColor Green
    Write-Host " SUCCESS: Release $tag published successfully to GitHub!" -ForegroundColor Green
    Write-Host " Download URL: https://github.com/trishulreddy3/syncdrop/releases/tag/$tag" -ForegroundColor Cyan
    Write-Host " Users will now receive the update directly in their app UI!" -ForegroundColor Green
    Write-Host "=======================================================`n" -ForegroundColor Green
} else {
    Write-Error "gh release create failed."
}
