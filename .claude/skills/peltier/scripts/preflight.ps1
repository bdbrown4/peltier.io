# peltier preflight - PowerShell entry point. Same contract as preflight.sh,
# for hosts where no POSIX shell exists (Windows-native harnesses). Works on
# Windows PowerShell 5.1 and pwsh 7+ on any OS.
#
# THE CONTRACT IS DEFINED ONCE: this script and preflight.sh must emit
# byte-identical output for the same environment. CI enforces that by running
# both on the same host and diffing stdout (skill-preflight job). If you
# change one, change the other, or the build goes red.
#
# STATUS=ok means the harness is present and runnable here. It does NOT mean
# this host can measure your change: only the A/A self-test decides that.
# A refusal (STATUS=refuse, exit 1) ends the *claim*, not the work.
#
# Source is deliberately pure ASCII: Windows PowerShell 5.1 misdecodes
# BOM-less UTF-8, so the em dashes in the shared refusal strings are composed
# at runtime instead of written literally.

$ErrorActionPreference = 'Stop'
$EM = [string][char]0x2014   # em dash, byte-identical to preflight.sh output

function Say([string]$s) { Write-Output $s }
function Refuse([string]$reason) {
    Say 'STATUS=refuse'
    Say "REASON=$reason"
    exit 1
}

# --- host -------------------------------------------------------------
# bench-runner shells every timed run through `sh -c`, and the in-repo
# pipeline is POSIX-only. Windows compiles the workspace but cannot run it.
# Host check comes FIRST, exactly as in preflight.sh.
$onLinux = (Test-Path variable:IsLinux) -and $IsLinux
$onMac   = (Test-Path variable:IsMacOS) -and $IsMacOS
if ($onLinux) { $hostKind = 'linux' }
elseif ($onMac) { $hostKind = 'darwin' }
else {
    Refuse ("unsupported host 'Windows' $EM peltier's harness is Linux/POSIX-only at runtime. " +
            "Run it on Linux (or macOS for verify mode). Do not substitute another benchmark.")
}

# --- trust layer ------------------------------------------------------
function Test-Peltier([string]$dir) {
    (Test-Path (Join-Path $dir 'crates/bench-runner/Cargo.toml') -PathType Leaf) -and
    (Test-Path (Join-Path $dir 'config/accept.toml') -PathType Leaf)
}

# Walk up from a starting dir looking for a checkout; parents terminate at
# the filesystem root, so this cannot spin.
function Find-Peltier([string]$start) {
    $d = $start
    while (-not [string]::IsNullOrEmpty($d)) {
        if (Test-Peltier $d) { return $d }
        $parent = Split-Path -Parent $d
        if ($parent -eq $d) { break }
        $d = $parent
    }
    return $null
}

$peltierHome = $null
if ($env:PELTIER_HOME) {
    $canon = $null
    try { $canon = (Resolve-Path -LiteralPath $env:PELTIER_HOME).Path } catch {}
    if (-not $canon) { Refuse "PELTIER_HOME='$($env:PELTIER_HOME)' does not exist" }
    if (-not (Test-Peltier $canon)) {
        Refuse "PELTIER_HOME='$($env:PELTIER_HOME)' is not a peltier checkout (no crates/bench-runner/Cargo.toml + config/accept.toml)"
    }
    $peltierHome = $canon
} else {
    # cwd first (working inside a checkout), then this script's own location
    # (the skill still living inside one). A skill copied into another project
    # finds neither - which is why PELTIER_HOME exists.
    $peltierHome = Find-Peltier (Get-Location).Path
    if (-not $peltierHome) { $peltierHome = Find-Peltier $PSScriptRoot }
}
if (-not $peltierHome) {
    Refuse ("no peltier checkout found $EM clone https://github.com/bdbrown4/peltier.io and set " +
            "PELTIER_HOME=/path/to/peltier.io. The statistics live in bench-runner; do not reimplement them.")
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Refuse "cargo not on PATH $EM needed to build bench-runner from $peltierHome"
}

# --- build the real harness -------------------------------------------
# Always build. cargo is a no-op when current, and skipping this because a
# binary happens to exist would silently measure with a stale copy of the
# statistics after a `git pull`.
$br = Join-Path $peltierHome 'target/release/bench-runner'
Push-Location $peltierHome
try { & cargo build --release -q -p bench-runner; $buildOk = ($LASTEXITCODE -eq 0) }
finally { Pop-Location }
if (-not $buildOk) {
    Refuse "bench-runner failed to build in $peltierHome $EM fix the build before claiming any number"
}
if (-not (Test-Path $br -PathType Leaf)) {
    Refuse "bench-runner did not appear at $br after a successful build"
}

# --- pinning ----------------------------------------------------------
# NOTE: config/accept.toml's `pin_prefix` is read ONLY by the in-repo verdict
# pipeline. For compare/aa/calibrate you must wrap the commands yourself
# (`taskset -c N <cmd>` on both sides). PIN_SUPPORTED just says whether that
# tool is available here.
if ($onLinux -and (Get-Command taskset -ErrorAction SilentlyContinue)) { $pin = 'yes' } else { $pin = 'no' }

Say 'STATUS=ok'
Say "PELTIER_HOME=$peltierHome"
Say "BENCH_RUNNER=$br"
Say "HOST=$hostKind"
Say "PIN_SUPPORTED=$pin"
Say 'NOTE=harness runnable here; the A/A self-test decides whether it can measure your change'
