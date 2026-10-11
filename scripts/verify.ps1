param(
    [switch]$Offline,
    [switch]$ReleaseTests,
    [switch]$Benchmark,
    [string]$CargoHome
)
$ErrorActionPreference = 'Stop'
$taskProjectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$taskToolchainText = Get-Content -LiteralPath (Join-Path $taskProjectRoot 'rust-toolchain.toml') -Raw
$taskVersion = [regex]::Match($taskToolchainText, 'channel\s*=\s*"([^"]+)"').Groups[1].Value
if (!$taskVersion) { throw 'Missing pinned Rust version' }

# Prefer a directly installed toolchain, including stable if it is the exact
# pinned version. This works when Rust was installed without a PATH refresh.
$taskRustBin = $null
foreach ($taskCandidate in @(
    (Join-Path $env:USERPROFILE ".rustup\toolchains\$taskVersion-x86_64-pc-windows-msvc\bin"),
    (Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin')
)) {
    $taskCompiler = Join-Path $taskCandidate 'rustc.exe'
    if (Test-Path -LiteralPath $taskCompiler) {
        $taskReported = & $taskCompiler --version
        if ($taskReported -match ('^rustc ' + [regex]::Escape($taskVersion) + ' ')) {
            $taskRustBin = $taskCandidate
            break
        }
    }
}
if ($taskRustBin) { $env:PATH = "$taskRustBin;$env:PATH" }
$taskCargo = (Get-Command cargo -ErrorAction Stop).Source
if ($CargoHome) { $env:CARGO_HOME = $CargoHome }
elseif (Test-Path -LiteralPath (Join-Path $taskProjectRoot '.build/cargo-home')) {
    $env:CARGO_HOME = Join-Path $taskProjectRoot '.build/cargo-home'
}

function Invoke-TaskCargo([string[]]$TaskArguments) {
    Write-Host ('cargo ' + ($TaskArguments -join ' '))
    & $taskCargo @TaskArguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo verification failed with exit code $LASTEXITCODE" }
}

$taskLocked = @('--locked')
if ($Offline) { $taskLocked += '--offline' }
Push-Location -LiteralPath $taskProjectRoot
try {
    Invoke-TaskCargo -TaskArguments @('fmt', '--check')
    Invoke-TaskCargo (@('clippy') + $taskLocked + @('--all-targets', '--', '-D', 'warnings'))
    Invoke-TaskCargo (@('test') + $taskLocked + @('--all-targets'))
    Invoke-TaskCargo (@('build') + $taskLocked + @('--release'))
    if ($ReleaseTests) { Invoke-TaskCargo (@('test') + $taskLocked + @('--release', '--all-targets')) }
    if ($Benchmark) { Invoke-TaskCargo (@('test') + $taskLocked + @('--release', '--test', 'cli', 'memory_performance_on_large_store', '--', '--ignored', '--nocapture')) }
} finally {
    Pop-Location
}
