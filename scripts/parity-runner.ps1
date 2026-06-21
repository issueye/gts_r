param(
    [string]$FixtureRoot,
    [string]$RustGs,
    [string]$GoGs,
    [string]$GoRoot,
    [int]$CaseTimeoutSeconds = 10,
    [switch]$AllowSkip
)

$ErrorActionPreference = "Stop"

function Resolve-PathIfPresent([string]$PathValue) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $null
    }
    if (Test-Path -LiteralPath $PathValue) {
        return (Resolve-Path -LiteralPath $PathValue).Path
    }
    return $null
}

function Quote-Argument([string]$Value) {
    if ($null -eq $Value) {
        return '""'
    }
    if ($Value.Length -eq 0) {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    return '"' + ($Value -replace '\\(?=\\*")', '$0$0' -replace '"', '\"') + '"'
}

function Invoke-Captured {
    param(
        [Parameter(Mandatory = $true)][string]$FileName,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [int]$TimeoutSeconds = 10
    )

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $FileName
    $psi.WorkingDirectory = $WorkingDirectory
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $psi.Arguments = ($Arguments | ForEach-Object { Quote-Argument $_ }) -join " "

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $psi
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        try {
            $process.Kill($true)
        } catch {
            $process.Kill()
        }
        $process.WaitForExit()
        throw "command timed out after $TimeoutSeconds seconds: $FileName $($psi.Arguments)"
    }
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()

    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdout
        Stderr = $stderr
        Command = "$FileName $($psi.Arguments)".Trim()
    }
}

function Find-GoRunner {
    param([string]$RepoRoot)

    $envGoGs = Resolve-PathIfPresent $env:GTS_GO_GS
    if ($envGoGs) {
        return [pscustomobject]@{ Kind = "exe"; File = $envGoGs; Root = $null; Label = "GTS_GO_GS"; Optional = $false }
    }

    $argGoGs = Resolve-PathIfPresent $GoGs
    if ($argGoGs) {
        return [pscustomobject]@{ Kind = "exe"; File = $argGoGs; Root = $null; Label = "GoGs"; Optional = $false }
    }

    $envGoRoot = Resolve-PathIfPresent $env:GTS_GO_ROOT
    if ($envGoRoot) {
        return [pscustomobject]@{ Kind = "go-build"; File = $null; Root = $envGoRoot; Label = "GTS_GO_ROOT"; Optional = $false }
    }

    $argGoRoot = Resolve-PathIfPresent $GoRoot
    if ($argGoRoot) {
        return [pscustomobject]@{ Kind = "go-build"; File = $null; Root = $argGoRoot; Label = "GoRoot"; Optional = $false }
    }

    $siblingRoot = Resolve-PathIfPresent (Join-Path (Split-Path -Parent $RepoRoot) "gts")
    if ($siblingRoot) {
        $gsExe = Resolve-PathIfPresent (Join-Path $siblingRoot "gs.exe")
        if ($gsExe) {
            return [pscustomobject]@{ Kind = "exe"; File = $gsExe; Root = $null; Label = "sibling gs.exe"; Optional = $true }
        }

        $roadmapExe = Resolve-PathIfPresent (Join-Path $siblingRoot "gtp-scheduler.exe")
        if ($roadmapExe) {
            return [pscustomobject]@{ Kind = "exe"; File = $roadmapExe; Root = $null; Label = "sibling gtp-scheduler.exe"; Optional = $true }
        }
    }

    return $null
}

function Get-BuiltGoCli {
    param([Parameter(Mandatory = $true)]$Runner)

    if ($Runner.PSObject.Properties.Name -contains "BuiltFile") {
        if (Test-Path -LiteralPath $Runner.BuiltFile) {
            return $Runner.BuiltFile
        }
    }

    $buildDir = Join-Path ([System.IO.Path]::GetTempPath()) "gts-r-parity"
    New-Item -ItemType Directory -Force -Path $buildDir | Out-Null
    $out = Join-Path $buildDir "gs-go.exe"
    $build = Invoke-Captured -FileName "go" -Arguments @("build", "-o", $out, "./cmd/gs") -WorkingDirectory $Runner.Root -TimeoutSeconds 60
    if ($build.ExitCode -ne 0) {
        throw "go build ./cmd/gs failed:`n$($build.Stdout)$($build.Stderr)"
    }

    if ($Runner.PSObject.Properties.Name -contains "BuiltFile") {
        $Runner.BuiltFile = $out
    } else {
        $Runner | Add-Member -NotePropertyName BuiltFile -NotePropertyValue $out
    }
    return $out
}

function Invoke-GoRunner {
    param(
        [Parameter(Mandatory = $true)]$Runner,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [string[]]$Arguments = @()
    )

    switch ($Runner.Kind) {
        "exe" {
            return Invoke-Captured -FileName $Runner.File -Arguments $Arguments -WorkingDirectory $WorkingDirectory -TimeoutSeconds $CaseTimeoutSeconds
        }
        "go-build" {
            $exe = Get-BuiltGoCli -Runner $Runner
            return Invoke-Captured -FileName $exe -Arguments $Arguments -WorkingDirectory $WorkingDirectory -TimeoutSeconds $CaseTimeoutSeconds
        }
        default {
            throw "unknown Go runner kind: $($Runner.Kind)"
        }
    }
}

function Skip-Or-Throw([string]$Message, [bool]$OptionalRunner) {
    if ($AllowSkip -and $OptionalRunner) {
        Write-Host "skipping Go/Rust parity: $Message"
        exit 0
    }
    throw $Message
}

function Get-CaseArguments([string]$CaseDir) {
    if (Test-Path -LiteralPath (Join-Path $CaseDir "project.toml")) {
        return [pscustomobject]@{
            Go = @("run")
            Rust = @("run")
        }
    }
    return [pscustomobject]@{
        Go = @("main.gs")
        Rust = @("run", "main.gs")
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($FixtureRoot)) {
    $FixtureRoot = Join-Path $repoRoot "tests/fixtures/parity"
}
$FixtureRoot = Resolve-PathIfPresent $FixtureRoot
if (-not $FixtureRoot) {
    throw "parity fixture root was not found"
}

$RustGs = Resolve-PathIfPresent $RustGs
if (-not $RustGs) {
    throw "Rust gs binary was not found; pass -RustGs or run through cargo test"
}

$goRunner = Find-GoRunner -RepoRoot $repoRoot
if (-not $goRunner) {
    $message = "skipping Go/Rust parity: no Go runner found; set GTS_GO_GS or GTS_GO_ROOT"
    if ($AllowSkip) {
        Write-Host $message
        exit 0
    }
    throw $message
}

$probeCase = Get-ChildItem -LiteralPath $FixtureRoot -Directory | Where-Object { $_.Name -eq "basic_expression" } | Select-Object -First 1
if ($probeCase) {
    $probeArgs = Get-CaseArguments $probeCase.FullName
    try {
        $probe = Invoke-GoRunner -Runner $goRunner -WorkingDirectory $probeCase.FullName -Arguments $probeArgs.Go
    } catch {
        Skip-Or-Throw "unable to run Go runner $($goRunner.Label): $($_.Exception.Message)" $goRunner.Optional
    }
    if ($probe.ExitCode -ne 0 -or $probe.Stdout.Length -eq 0) {
        Skip-Or-Throw "Go runner $($goRunner.Label) is not a usable gs CLI for parity fixtures" $goRunner.Optional
    }
}

$caseDirs = Get-ChildItem -LiteralPath $FixtureRoot -Directory | Sort-Object Name
if ($caseDirs.Count -eq 0) {
    throw "no parity fixture groups found under $FixtureRoot"
}

$failures = New-Object System.Collections.Generic.List[string]
$skipped = 0
foreach ($case in $caseDirs) {
    $rustOnlyMarker = Join-Path $case.FullName "rust-only"
    if (Test-Path -LiteralPath $rustOnlyMarker) {
        Write-Host "skipping Go/Rust parity for $($case.Name): rust-only fixture"
        $skipped = $skipped + 1
        continue
    }

    $args = Get-CaseArguments $case.FullName
    try {
        $go = Invoke-GoRunner -Runner $goRunner -WorkingDirectory $case.FullName -Arguments $args.Go
    } catch {
        Skip-Or-Throw "unable to run Go runner $($goRunner.Label): $($_.Exception.Message)" $goRunner.Optional
    }
    try {
        $rust = Invoke-Captured -FileName $RustGs -Arguments $args.Rust -WorkingDirectory $case.FullName -TimeoutSeconds $CaseTimeoutSeconds
    } catch {
        $failures.Add("$($case.Name): unable to run Rust runner: $($_.Exception.Message)")
        continue
    }

    if ($go.ExitCode -ne $rust.ExitCode) {
        $failures.Add("$($case.Name): exit code mismatch Go=$($go.ExitCode) Rust=$($rust.ExitCode)")
    }
    if ($go.Stdout -cne $rust.Stdout) {
        $failures.Add("$($case.Name): stdout mismatch`n--- go ---`n$($go.Stdout)--- rust ---`n$($rust.Stdout)")
    }
    if ($go.Stderr -cne $rust.Stderr) {
        $failures.Add("$($case.Name): stderr mismatch`n--- go ---`n$($go.Stderr)--- rust ---`n$($rust.Stderr)")
    }
}

if ($failures.Count -gt 0) {
    Write-Error (($failures | ForEach-Object { $_ }) -join "`n`n")
    exit 1
}

$runCount = $caseDirs.Count - $skipped
Write-Host "Go/Rust parity passed for $runCount fixture group(s) using $($goRunner.Label); skipped $skipped rust-only fixture group(s)."
