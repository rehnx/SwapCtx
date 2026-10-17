# Recipient-side offline acceptance. Requires only Windows PowerShell 5.1.
# Runs installed SwapCtx core commands with isolated data, never the Codex adapter.
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedSha256,
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256,
    [Parameter(Mandatory = $true)][string]$WorkDirectory,
    [Parameter(Mandatory = $true)][string]$Report
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Windows acceptance requires Windows' }
$taskWork = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($WorkDirectory)
$taskReport = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Report)
$taskParent = [IO.Path]::GetDirectoryName($taskWork)
if (!(Test-Path -LiteralPath $taskParent -PathType Container) -or !(Test-Path -LiteralPath ([IO.Path]::GetDirectoryName($taskReport)) -PathType Container)) { throw 'Work/report parents must exist' }
if ((Get-Item -LiteralPath $taskWork -Force -ErrorAction SilentlyContinue) -or (Get-Item -LiteralPath $taskReport -Force -ErrorAction SilentlyContinue)) { throw 'Work directory and report must be new; nothing overwritten' }
if ($taskReport.StartsWith($taskWork + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Report must be outside disposable work directory' }
$taskArchive = (Get-Item -LiteralPath $Archive -Force).FullName
$taskInstaller = (Get-Item -LiteralPath $Installer -Force).FullName
$taskOriginalPath = $env:PATH
$taskOriginalLocal = $env:LOCALAPPDATA
$taskOriginalLocation = Get-Location
$taskChecks = [Collections.Generic.List[string]]::new()
$taskExit = 1
$taskPhase = 'verify_inputs'
$taskCreatedWork = $false
function Confirm-Task([bool]$Condition,[string]$Name) {
    if (!$Condition) { throw "Acceptance assertion failed: $Name" }
    $taskChecks.Add($Name)
}
function Invoke-Candidate([string[]]$Arguments,[int]$Expected = 0) {
    # Expected failure commands may emit native stderr, which PowerShell 5.1
    # wraps as error records; check the process exit code explicitly.
    $taskPrior = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $taskOutput = & swapctx @Arguments 2>&1; $taskCode = $LASTEXITCODE }
    finally { $ErrorActionPreference = $taskPrior }
    if ($taskCode -ne $Expected) { throw "Unexpected SwapCtx exit $taskCode (expected $Expected)" }
    return (($taskOutput | ForEach-Object { $_.ToString() }) -join "`n")
}
try {
    Confirm-Task ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash -ieq $ExpectedSha256) 'archive_hash'
    Confirm-Task ((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ieq $InstallerSha256) 'installer_hash'
    [void][IO.Directory]::CreateDirectory($taskWork)
    $taskCreatedWork = $true
    $taskInstall = Join-Path $taskWork 'installed application'
    $taskPhase = 'install'
    $taskPowerShell = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $taskInstallOutput = & $taskPowerShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $taskInstaller -Archive $taskArchive -ExpectedSha256 $ExpectedSha256 -Destination $taskInstall 2>&1
    Confirm-Task ($LASTEXITCODE -eq 0) 'verified_installation'
    $taskManifest = Get-Content -LiteralPath (Join-Path $taskInstall 'manifest.json') -Raw | ConvertFrom-Json
    $taskExecutable = Join-Path $taskInstall 'swapctx.exe'
    $taskEntry = @($taskManifest.files | Where-Object { $_.path -ceq 'swapctx.exe' })
    Confirm-Task ($taskEntry.Count -eq 1 -and (Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash -ieq $taskEntry[0].sha256) 'installed_binary_hash'
    $taskPhase = 'core_without_developer_path'
    $env:PATH = $taskInstall + ';' + (Join-Path $env:SystemRoot 'System32') + ';' + $env:SystemRoot + ';' + (Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0')
    $env:LOCALAPPDATA = Join-Path $taskWork 'isolated local data'
    Confirm-Task (@(Get-Command node,cargo,rustc,codex -CommandType Application -ErrorAction SilentlyContinue).Count -eq 0) 'development_tools_absent_from_path'
    Confirm-Task ((Get-Command swapctx -CommandType Application).Source -ieq $taskExecutable) 'bare_command_resolution'
    Confirm-Task ((Invoke-Candidate @('--version')).Trim() -ceq ('swapctx ' + $taskManifest.version)) 'version_matches_package'
    [void](Invoke-Candidate @('--help'))
    [void](Invoke-Candidate @('capabilities'))
    Confirm-Task (!(Test-Path -LiteralPath (Join-Path $env:LOCALAPPDATA 'SwapCtx/swapctx.db'))) 'informational_commands_do_not_create_store'
    $taskFirst = Join-Path $taskWork 'first project'
    $taskNested = Join-Path $taskFirst 'src/deep'
    $taskSecond = Join-Path $taskWork 'second project'
    [void][IO.Directory]::CreateDirectory($taskNested)
    [void][IO.Directory]::CreateDirectory($taskSecond)
    Set-Location -LiteralPath $taskFirst
    [void](Invoke-Candidate @('init'))
    [void](Invoke-Candidate @('memory','add','--kind','requirement','--key','interface','Preserve public interfaces'))
    Set-Location -LiteralPath $taskNested
    $taskContext = Invoke-Candidate @('context') | ConvertFrom-Json
    Confirm-Task ($taskContext.records.Count -eq 1 -and $taskContext.records[0].content -ceq 'Preserve public interfaces') 'nested_default_store_retrieval'
    Set-Location -LiteralPath $taskSecond
    [void](Invoke-Candidate @('init'))
    $taskContext = Invoke-Candidate @('context') | ConvertFrom-Json
    Confirm-Task ($taskContext.records.Count -eq 0) 'separate_project_scope'
    [void](Invoke-Candidate @('memory','add','--kind','note','Second project only'))
    Set-Location -LiteralPath $taskFirst
    $taskContext = Invoke-Candidate @('context') | ConvertFrom-Json
    Confirm-Task ($taskContext.records.Count -eq 1 -and $taskContext.records[0].content -ceq 'Preserve public interfaces') 'original_project_preserved'
    [void](Invoke-Candidate @('protect','add','--path','src/auth/**','--reason','Manual review required'))
    [void](Invoke-Candidate -Arguments @('protect','check','src/auth/login.rs') -Expected 2)
    $taskChecks.Add('manual_boundary_deny_exit')
    $taskPhase = 'recovery'
    $taskBackup = Join-Path $taskWork 'backup.db'
    [void](Invoke-Candidate @('backup',$taskBackup))
    $taskBackupHash = (Get-FileHash -LiteralPath $taskBackup -Algorithm SHA256).Hash
    [void](Invoke-Candidate -Arguments @('backup',$taskBackup) -Expected 1)
    Confirm-Task ((Get-FileHash -LiteralPath $taskBackup -Algorithm SHA256).Hash -ceq $taskBackupHash) 'backup_never_overwrites'
    $taskRecovery = Join-Path $taskWork 'new recovery store'
    [void](Invoke-Candidate @('--data-dir',$taskRecovery,'restore',$taskBackup))
    [void](Invoke-Candidate @('--data-dir',$taskRecovery,'doctor'))
    $taskContext = Invoke-Candidate @('--data-dir',$taskRecovery,'context') | ConvertFrom-Json
    Confirm-Task ($taskContext.records.Count -eq 1 -and $taskContext.records[0].content -ceq 'Preserve public interfaces') 'restored_context'
    [void](Invoke-Candidate -Arguments @('--data-dir',$taskRecovery,'restore',$taskBackup) -Expected 1)
    $taskChecks.Add('restore_refuses_existing_directory')
    [void](Invoke-Candidate @('doctor'))
    $taskChecks.Add('original_store_integrity')
    $taskPhase = 'complete'
    $taskExit = 0
} catch {
    Write-Warning ('Acceptance failed in phase ' + $taskPhase + ': ' + $_.Exception.Message)
} finally {
    $env:PATH = $taskOriginalPath
    $env:LOCALAPPDATA = $taskOriginalLocal
    Set-Location -LiteralPath $taskOriginalLocation.Path
    if ($taskCreatedWork) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskWork)) -ne $taskParent) { throw 'Refusing cleanup outside work parent' }
        Remove-Item -LiteralPath $taskWork -Recurse -Force
    }
    $taskResult = [ordered]@{ format_version = 1; status = $(if ($taskExit -eq 0) { 'passed' } else { 'failed' }); phase = $taskPhase; checks = @($taskChecks.ToArray()); archive_sha256 = $ExpectedSha256.ToLowerInvariant(); installer_sha256 = $InstallerSha256.ToLowerInvariant(); windows_version = [Environment]::OSVersion.Version.ToString(); powershell_version = $PSVersionTable.PSVersion.ToString(); limits = @('Same installed OS; developer tools removed from PATH, not uninstalled', 'No persistent PATH or clean-user-profile validation', 'Checksums do not authenticate a publisher', 'No Codex, model behavior or automatic context recovery tested'); model_calls = 0; user_data_modified = $false }
    $taskBytes = [Text.UTF8Encoding]::new($false).GetBytes(($taskResult | ConvertTo-Json -Depth 5))
    $taskStream = [IO.File]::Open($taskReport,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write)
    try { $taskStream.Write($taskBytes,0,$taskBytes.Length); $taskStream.Flush($true) } finally { $taskStream.Dispose() }
}
if ($taskExit -ne 0) { throw "Windows preview acceptance failed; inspect $taskReport" }
Write-Output "Windows preview acceptance passed; report: $taskReport"
