# Offline producer pipeline. Stages and verifies locally; never publishes.
param(
    [Parameter(Mandatory = $true)][string]$Destination,
    # Reuse an explicitly trusted build with a complete current receipt.
    [string]$ExistingBuild
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Resolve an existing directory by handle, including Windows 8.3 aliases and
# junctions. Keep all new children in one canonical namespace before comparing.
if (-not ('SwapCtx.ReleaseDirectoryPaths' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
namespace SwapCtx {
    public static class ReleaseDirectoryPaths {
        [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
        private static extern SafeFileHandle CreateFile(string path, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);
        [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
        private static extern uint GetFinalPathNameByHandle(SafeFileHandle file, StringBuilder path, uint length, uint flags);
        public static string Resolve(string path) {
            using (SafeFileHandle file = CreateFile(Path.GetFullPath(path), 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {
                if (file.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot open release directory");
                StringBuilder text = new StringBuilder(32768);
                uint length = GetFinalPathNameByHandle(file, text, (uint)text.Capacity, 0);
                if (length == 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot resolve release directory");
                if (length >= text.Capacity) throw new IOException("Resolved release path is too long");
                string result = text.ToString();
                if (result.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)) return @"\\" + result.Substring(8);
                if (result.StartsWith(@"\\?\", StringComparison.Ordinal) && result.Length > 6 &&
                    Char.IsLetter(result[4]) && result[5] == ':' && result[6] == '\\') return result.Substring(4);
                throw new IOException("Unsupported release directory namespace");
            }
        }
    }
}
'@
}
function Resolve-ReleaseDirectory([string]$Path) {
    return [SwapCtx.ReleaseDirectoryPaths]::Resolve($Path)
}
function Get-TaskSha256([string]$Path) {
    $taskHasher = [Security.Cryptography.SHA256]::Create()
    try {
        $taskStream = [IO.File]::OpenRead($Path)
        try { return ([BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-','').ToLowerInvariant()) }
        finally { $taskStream.Dispose() }
    } finally { $taskHasher.Dispose() }
}
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$taskOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination)
$taskParent = [IO.Path]::GetDirectoryName($taskOutput)
if ($ExistingBuild) { $ExistingBuild = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ExistingBuild) }
if (!(Test-Path -LiteralPath $taskParent -PathType Container)) { throw 'Destination parent must exist' }
$taskParent = Resolve-ReleaseDirectory $taskParent
$taskOutput = [IO.Path]::Combine($taskParent,[IO.Path]::GetFileName($taskOutput))
if (Get-Item -LiteralPath $taskOutput -Force -ErrorAction SilentlyContinue) { throw 'Destination exists; nothing overwritten' }
$taskNode = (Get-Command node -ErrorAction Stop).Source
$taskPS = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
$taskStage = Join-Path $taskParent ('.swapctx-preview-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($taskStage)
$taskSteps = [Collections.Generic.List[string]]::new()
$taskOriginalPath = $env:PATH
function Invoke-PreviewCommand([string]$Name,[string]$Program,[string[]]$Arguments) {
    Write-Host "Preview gate: $Name"
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Preview gate failed: $Name (exit $LASTEXITCODE); no output published" }
    $taskSteps.Add($Name)
}
Push-Location -LiteralPath $taskRoot
try {
    $env:PATH = [IO.Path]::GetDirectoryName($taskNode) + ';' + $env:PATH
    Invoke-PreviewCommand 'foundation_offline' $taskPS @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',(Join-Path $PSScriptRoot 'verify.ps1'),'-DeterministicOnly','-ReleaseTests')
    $taskBuild = Join-Path $taskStage 'build'
    if ($ExistingBuild) {
        $taskExisting = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ExistingBuild)
        [void][IO.Directory]::CreateDirectory($taskBuild)
        foreach ($taskName in @('swapctx.exe','build-info.json')) {
            $taskItem = Get-Item -LiteralPath (Join-Path $taskExisting $taskName) -Force
            if ($taskItem.PSIsContainer -or ($taskItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Existing build files must be regular files' }
            [IO.File]::Copy($taskItem.FullName,(Join-Path $taskBuild $taskName),$false)
        }
    } else {
        Invoke-PreviewCommand 'independent_release_builds_and_native_tests' $taskNode @((Join-Path $PSScriptRoot 'build-windows.cjs'),$taskBuild,'--test')
    }
    $taskBinary = Join-Path $taskBuild 'swapctx.exe'
    $taskReceipt = Join-Path $taskBuild 'build-info.json'
    $taskCandidate = Join-Path $taskStage 'candidate'
    Invoke-PreviewCommand 'receipt_checked_packaging' $taskPS @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',(Join-Path $PSScriptRoot 'package-windows.ps1'),'-Destination',$taskCandidate,'-Binary',$taskBinary,'-BuildInfo',$taskReceipt)
    foreach ($taskScript in @('check-context.cjs','check-session.cjs','check-everyday.cjs','check-native.cjs','check-package.cjs')) {
        Invoke-PreviewCommand $taskScript $taskNode @((Join-Path $PSScriptRoot $taskScript),$taskBinary)
    }
    Invoke-PreviewCommand 'static_imports_and_command_discovery' $taskNode @((Join-Path $PSScriptRoot 'check-release.cjs'),$taskBinary,'--static')
    Invoke-PreviewCommand 'receipt_failure_regressions' $taskNode @((Join-Path $PSScriptRoot 'check-build-receipt.cjs'),$taskBinary,$taskReceipt)
    $taskZip = @(Get-ChildItem -LiteralPath $taskCandidate -Filter '*.zip' -File)
    if ($taskZip.Count -ne 1) { throw 'Expected exactly one candidate ZIP' }
    $taskZipHash = Get-TaskSha256 $taskZip[0].FullName
    $taskInstaller = Join-Path $taskCandidate 'install-windows.ps1'
    $taskInstallerHash = Get-TaskSha256 $taskInstaller
    $taskAcceptanceReport = Join-Path $taskCandidate 'windows-acceptance.json'
    Invoke-PreviewCommand 'recipient_powershell_only_acceptance' $taskPS @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',(Join-Path $taskCandidate 'test-windows-preview.ps1'),'-Archive',$taskZip[0].FullName,'-ExpectedSha256',$taskZipHash,'-Installer',$taskInstaller,'-InstallerSha256',$taskInstallerHash,'-WorkDirectory',(Join-Path $taskStage 'acceptance-work'),'-Report',$taskAcceptanceReport)
    $taskAcceptanceResult = Get-Content -LiteralPath $taskAcceptanceReport -Raw | ConvertFrom-Json
    if ($taskAcceptanceResult.status -cne 'passed') { throw 'Recipient acceptance report did not pass' }
    $taskResult = [ordered]@{ format_version = 1; status = 'passed'; steps = @($taskSteps.ToArray()); existing_build_reused = [bool]$ExistingBuild; archive_sha256 = $taskZipHash; installer_sha256 = $taskInstallerHash; limits = @('Local same-machine evidence, not hosted or clean-machine approval','No authenticated build provenance or publisher signature','No persistent PATH or package-manager registration verified'); model_calls = 0; published = $false }
    $taskGateFile = Join-Path $taskCandidate 'release-checks.json'
    [IO.File]::WriteAllText($taskGateFile,($taskResult | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    foreach ($taskFile in @($taskAcceptanceReport,$taskGateFile)) {
        $taskLine = (Get-TaskSha256 $taskFile) + '  ' + [IO.Path]::GetFileName($taskFile) + "`n"
        [IO.File]::AppendAllText((Join-Path $taskCandidate 'SHA256SUMS.txt'),$taskLine,[Text.UTF8Encoding]::new($false))
    }
    [IO.Directory]::Move($taskCandidate,$taskOutput)
    Write-Output "Offline preview gates passed. Local output: $taskOutput"
} finally {
    $env:PATH = $taskOriginalPath
    Pop-Location
    if (Test-Path -LiteralPath $taskStage) {
        $taskActualParent = Resolve-ReleaseDirectory ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskStage)))
        if (-not [String]::Equals($taskActualParent.TrimEnd('\'),$taskParent.TrimEnd('\'),[StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing cleanup outside staging parent' }
        if ((Get-Item -LiteralPath $taskStage -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing cleanup of redirected staging directory' }
        Remove-Item -LiteralPath $taskStage -Recurse -Force
    }
}
