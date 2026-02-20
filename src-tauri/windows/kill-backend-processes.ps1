param(
    [Parameter(Mandatory = $false)]
    [string]$InstallDir
)

$ErrorActionPreference = "SilentlyContinue"

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    exit 0
}

try {
    $installRootRaw = [System.IO.Path]::GetFullPath($InstallDir).TrimEnd([char]92)
} catch {
    exit 0
}

if ([string]::IsNullOrWhiteSpace($installRootRaw)) {
    exit 0
}

$installRoot = $installRootRaw
$installRootWithSep = $installRoot + [string][char]92
$currentPid = $PID
$targetProcessNames = @(
    "python.exe",
    "pythonw.exe",
    "astrbot-desktop-tauri.exe",
    "astrbot.exe"
)

function Test-IsUnderInstallRoot {
    param(
        [Parameter(Mandatory = $false)]
        [string]$PathValue
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    try {
        $normalized = [System.IO.Path]::GetFullPath($PathValue).TrimEnd([char]92)
        return $normalized -ieq $installRoot -or $normalized.StartsWith($installRootWithSep, [System.StringComparison]::OrdinalIgnoreCase)
    } catch {
        return $false
    }
}

function Get-CommandExecutablePath {
    param(
        [Parameter(Mandatory = $false)]
        [string]$CommandLine
    )

    if ([string]::IsNullOrWhiteSpace($CommandLine)) {
        return $null
    }

    $trimmed = $CommandLine.Trim()
    if ($trimmed.StartsWith('"')) {
        $endQuote = $trimmed.IndexOf('"', 1)
        if ($endQuote -gt 1) {
            return $trimmed.Substring(1, $endQuote - 1)
        }
        return $null
    }

    $firstSpace = $trimmed.IndexOf(' ')
    if ($firstSpace -gt 0) {
        return $trimmed.Substring(0, $firstSpace)
    }
    return $trimmed
}

$nameFilter = ($targetProcessNames | ForEach-Object { "Name='$_'" }) -join " OR "

Get-CimInstance Win32_Process -Filter $nameFilter |
    ForEach-Object {
        if ($_.ProcessId -eq $currentPid) {
            return
        }

        $shouldStop = Test-IsUnderInstallRoot -PathValue $_.ExecutablePath

        if (-not $shouldStop) {
            $commandExePath = Get-CommandExecutablePath -CommandLine $_.CommandLine
            $shouldStop = Test-IsUnderInstallRoot -PathValue $commandExePath
        }

        if ($shouldStop) {
            Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
        }
    }
