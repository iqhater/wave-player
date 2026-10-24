#requires -Version 5.1
<#
.SYNOPSIS
    Runs the in-app updater end to end without publishing a GitHub release.

.DESCRIPTION
    Builds two portable Windows builds (the running one and a newer one),
    serves a fake GitHub Releases response from a local HTTP server and starts
    the older build with WAVE_PLAYER_UPDATE_API pointing at it.

    After the app restarts, the window title shows the newer build and
    assets/update-test-marker.txt appears in the portable folder.

.PARAMETER CurrentBuild
    WAVE_PLAYER_BUILD compiled into the build that is started first.

.PARAMETER UpdateBuild
    Build number offered by the fake release. Must be greater than CurrentBuild.

.PARAMETER SkipBuild
    Reuses the artifacts of a previous run instead of rebuilding.

.PARAMETER NoLaunch
    Only prepares the server and the portable build.

.PARAMETER Port
    Local server port. When 0, the first free port is used.

.PARAMETER StopServer
    Stops the local server started by a previous run.

.EXAMPLE
    pwsh scripts/test-update.ps1

.EXAMPLE
    pwsh scripts/test-update.ps1 -StopServer
#>
[CmdletBinding()]
param(
    [int]$CurrentBuild = 900,
    [int]$UpdateBuild = 901,
    [int]$Port = 0,
    [switch]$SkipBuild,
    [switch]$NoLaunch,
    [switch]$StopServer
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$WorkRoot = Join-Path $RepoRoot "target/update-test"
$AppDir = Join-Path $WorkRoot "app"
$StagingDir = Join-Path $WorkRoot "new"
$ServerDir = Join-Path $WorkRoot "server"
$PidFile = Join-Path $WorkRoot "server.pid"

function Find-FreePort {
    for ($candidate = 9899; $candidate -lt 10000; $candidate++) {
        try {
            $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $candidate)
            $listener.Start()
            $listener.Stop()
            return $candidate
        } catch {
            continue
        }
    }

    throw "No free local port found in the 9899-9999 range"
}

function Stop-TestServer {
    if (-not (Test-Path $PidFile)) {
        return
    }

    $serverPid = Get-Content $PidFile | Select-Object -First 1
    $process = Get-Process -Id $serverPid -ErrorAction SilentlyContinue

    if ($process) {
        Stop-Process -Id $process.Id -Force
    }

    Remove-Item $PidFile -Force -ErrorAction SilentlyContinue
}

function Build-App([int]$Build) {
    Write-Host "Building with WAVE_PLAYER_BUILD=$Build..." -ForegroundColor Cyan

    $env:WAVE_PLAYER_BUILD = "$Build"

    try {
        Push-Location $RepoRoot
        try {
            & dx build --release --platform desktop 2>&1 | Out-Host
            if ($LASTEXITCODE -ne 0) {
                throw "dx build failed with exit code $LASTEXITCODE"
            }
        } finally {
            Pop-Location
        }
    } finally {
        Remove-Item Env:WAVE_PLAYER_BUILD -ErrorAction SilentlyContinue
    }

    $source = Join-Path $RepoRoot "target/dx/wave_player/release/windows/app"
    $exe = Join-Path $source "wave_player.exe"

    if (-not (Test-Path $exe)) {
        throw "Build output not found: $exe"
    }

    return $source
}

function Copy-PortableApp([string]$Source, [string]$Destination) {
    if (Test-Path $Destination) {
        Remove-Item -Recurse -Force $Destination
    }

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item (Join-Path $Source "wave_player.exe") $Destination -Force

    if (Test-Path (Join-Path $Source "assets")) {
        Copy-Item (Join-Path $Source "assets") $Destination -Recurse -Force
    }
}

if ($StopServer) {
    Stop-TestServer
    Write-Host "Local update server stopped." -ForegroundColor Green
    return
}

if ($UpdateBuild -le $CurrentBuild) {
    throw "UpdateBuild ($UpdateBuild) must be greater than CurrentBuild ($CurrentBuild)"
}

$versionLine = Select-String -Path (Join-Path $RepoRoot "Cargo.toml") -Pattern '^version = "(.+)"' |
    Select-Object -First 1
$Version = $versionLine.Matches[0].Groups[1].Value
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
$ZipName = "WavePlayer_${Version}_windows_${Arch}_portable.zip"
$Tag = "v${Version}-build.${UpdateBuild}"

if ($Port -le 0) {
    $Port = Find-FreePort
}

$BaseUrl = "http://127.0.0.1:$Port"
$ReleasesUrl = "$BaseUrl/releases.json"

New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

if (-not $SkipBuild -and -not (Get-Command dx -ErrorAction SilentlyContinue)) {
    throw "dx (Dioxus CLI) is required to build the test binaries"
}

if (-not $SkipBuild) {
    $currentSource = Build-App $CurrentBuild
    Copy-PortableApp $currentSource $AppDir
    $currentHash = (Get-FileHash (Join-Path $AppDir "wave_player.exe") -Algorithm SHA256).Hash

    $updateSource = Build-App $UpdateBuild
    $updateExe = Join-Path $updateSource "wave_player.exe"
    $updateHash = (Get-FileHash $updateExe -Algorithm SHA256).Hash

    if ($currentHash -eq $updateHash) {
        throw "Both builds produced the same executable. dx did not pick up WAVE_PLAYER_BUILD."
    }

    Copy-PortableApp $updateSource $StagingDir
    Copy-Item (Join-Path $RepoRoot "README.md") $StagingDir -Force
    Copy-Item (Join-Path $RepoRoot "LICENSE") $StagingDir -Force
    Set-Content -Path (Join-Path $StagingDir "assets/update-test-marker.txt") `
        -Value "Local updater test build $UpdateBuild" -Encoding Ascii
}

if (-not (Test-Path (Join-Path $AppDir "wave_player.exe"))) {
    throw "The portable build is missing. Run the script without -SkipBuild first."
}

if (-not (Test-Path (Join-Path $StagingDir "wave_player.exe"))) {
    throw "The staged update is missing. Run the script without -SkipBuild first."
}

if ($SkipBuild) {
    $appHash = (Get-FileHash (Join-Path $AppDir "wave_player.exe") -Algorithm SHA256).Hash
    $stagedHash = (Get-FileHash (Join-Path $StagingDir "wave_player.exe") -Algorithm SHA256).Hash

    if ($appHash -eq $stagedHash) {
        throw "The portable build already contains the update. Run the script without -SkipBuild."
    }
}

New-Item -ItemType Directory -Force -Path $ServerDir | Out-Null

$zipPath = Join-Path $ServerDir $ZipName
Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $StagingDir "*") -DestinationPath $zipPath -Force

$zipHash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$zipSize = (Get-Item $zipPath).Length

$sumsPath = Join-Path $ServerDir "SHA256SUMS"
Set-Content -Path $sumsPath -Value "$zipHash  $ZipName" -Encoding Ascii
$sumsSize = (Get-Item $sumsPath).Length

$releases = @"
[
  {
    "tag_name": "$Tag",
    "name": "Local updater test",
    "draft": false,
    "prerelease": true,
    "assets": [
      {
        "name": "$ZipName",
        "browser_download_url": "$BaseUrl/$ZipName",
        "size": $zipSize
      },
      {
        "name": "SHA256SUMS",
        "browser_download_url": "$BaseUrl/SHA256SUMS",
        "size": $sumsSize
      }
    ]
  }
]
"@

Set-Content -Path (Join-Path $ServerDir "releases.json") -Value $releases -Encoding Ascii

Stop-TestServer

$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command py -ErrorAction SilentlyContinue
}

if (-not $python) {
    throw "Python is required to serve the fake release. Install Python or serve $ServerDir manually on port $Port."
}

$serverLog = Join-Path $WorkRoot "server.log"
$server = Start-Process -FilePath $python.Source `
    -ArgumentList @("-m", "http.server", "$Port", "--bind", "127.0.0.1", "--directory", "`"$ServerDir`"") `
    -RedirectStandardOutput $serverLog -RedirectStandardError "$serverLog.err" `
    -PassThru -WindowStyle Hidden

Set-Content -Path $PidFile -Value $server.Id -Encoding Ascii

$ready = $false
for ($attempt = 0; $attempt -lt 40; $attempt++) {
    try {
        Invoke-RestMethod -Uri $ReleasesUrl -TimeoutSec 2 | Out-Null
        $ready = $true
        break
    } catch {
        Start-Sleep -Milliseconds 250
    }
}

if (-not $ready) {
    Stop-TestServer
    $details = @()
    foreach ($log in @($serverLog, "$serverLog.err")) {
        if (Test-Path $log) {
            $details += Get-Content $log -Raw
        }
    }
    throw "The local release server did not start on port ${Port}: $($details -join ' ')"
}

Write-Host ""
Write-Host "Fake release $Tag is served from $ReleasesUrl" -ForegroundColor Green
Write-Host "Test build: $AppDir" -ForegroundColor Green
Write-Host ""

if ($NoLaunch) {
    Write-Host "Start the app manually with WAVE_PLAYER_UPDATE_API=$ReleasesUrl"
} else {
    $env:WAVE_PLAYER_UPDATE_API = $ReleasesUrl

    try {
        Start-Process -FilePath (Join-Path $AppDir "wave_player.exe") -WorkingDirectory $AppDir
    } finally {
        Remove-Item Env:WAVE_PLAYER_UPDATE_API -ErrorAction SilentlyContinue
    }
}

Write-Host "Expected: the sidebar banner offers build $UpdateBuild."
Write-Host "Click Update Now, then Restart to update."
Write-Host "After the restart the title should read '(build $UpdateBuild)' and"
Write-Host "assets/update-test-marker.txt should appear in the test folder."
Write-Host ""
Write-Host "Stop the server with: pwsh scripts/test-update.ps1 -StopServer" -ForegroundColor Yellow
