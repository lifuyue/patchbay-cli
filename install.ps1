$ErrorActionPreference = "Stop"

$Repo = if ($env:ISSUE_FINDER_REPO) { $env:ISSUE_FINDER_REPO } else { "lifuyue/issue-finder" }
$Version = if ($env:ISSUE_FINDER_VERSION) { $env:ISSUE_FINDER_VERSION } else { "latest" }
$InstallDir = if ($env:ISSUE_FINDER_INSTALL_DIR) {
    $env:ISSUE_FINDER_INSTALL_DIR
} elseif ($env:LOCALAPPDATA) {
    Join-Path $env:LOCALAPPDATA "Issue Finder\bin"
} else {
    Join-Path $HOME ".issue-finder\bin"
}

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
switch ($Arch) {
    "X64" { $Asset = "issue-finder-x86_64-pc-windows-msvc.zip" }
    default {
        throw "issue-finder installer: unsupported Windows architecture: $Arch. Download a release manually from https://github.com/$Repo/releases"
    }
}

if ($Version -eq "latest") {
    $BaseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
    $BaseUrl = "https://github.com/$Repo/releases/download/$Version"
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("issue-finder-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempDir | Out-Null

try {
    $Archive = Join-Path $TempDir $Asset
    $Checksums = Join-Path $TempDir "SHA256SUMS"

    Write-Host "Downloading Issue Finder $Version for Windows $Arch..."
    Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $Archive
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile $Checksums

    $Pattern = "\s$([System.Text.RegularExpressions.Regex]::Escape($Asset))$"
    $ExpectedLine = Get-Content $Checksums | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if (-not $ExpectedLine) {
        throw "issue-finder installer: SHA256SUMS does not include $Asset"
    }

    $ExpectedHash = (($ExpectedLine -split "\s+")[0]).ToUpperInvariant()
    $ActualHash = (Get-FileHash -Path $Archive -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($ActualHash -ne $ExpectedHash) {
        throw "issue-finder installer: checksum mismatch for $Asset"
    }

    Expand-Archive -Path $Archive -DestinationPath $TempDir -Force
    $Binary = Join-Path $TempDir "issue-finder.exe"
    if (-not (Test-Path $Binary)) {
        throw "issue-finder installer: archive did not contain issue-finder.exe"
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $InstalledPath = Join-Path $InstallDir "issue-finder.exe"
    Copy-Item -Path $Binary -Destination $InstalledPath -Force

    Write-Host "Installed issue-finder to $InstalledPath"

    $PathEntries = $env:PATH -split ";"
    if ($PathEntries -notcontains $InstallDir) {
        Write-Host "Add $InstallDir to your PATH to run issue-finder from any shell."
    }
} finally {
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}
