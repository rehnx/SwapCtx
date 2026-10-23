# Package an already verified, trusted local binary. Never invokes an AI agent.
param(
    [Parameter(Mandatory = $true)][string]$Destination,
    [string]$Binary,
    # Optional local receipt from the independent offline release-build recipe.
    [string]$BuildInfo
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
function Resolve-ReleaseDirectoryKey([string]$Path) {
    return (Resolve-ReleaseDirectory $Path).TrimEnd('\')
}
function Test-ReleaseDirectoryEqual([string]$Left,[string]$Right) {
    return [String]::Equals((Resolve-ReleaseDirectoryKey $Left),(Resolve-ReleaseDirectoryKey $Right),[StringComparison]::OrdinalIgnoreCase)
}
function Get-TaskSha256([string]$Path) {
    $taskHasher = [Security.Cryptography.SHA256]::Create()
    try {
        $taskStream = [IO.File]::OpenRead($Path)
        try { return ([BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-','').ToLowerInvariant()) }
        finally { $taskStream.Dispose() }
    } finally { $taskHasher.Dispose() }
}
if ($env:OS -ne 'Windows_NT') { throw 'Windows packaging requires Windows' }
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
if (!$Binary) { $Binary = Join-Path $taskRoot 'target/release/swapctx.exe' }
$taskOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination)
$taskParent = [IO.Path]::GetDirectoryName($taskOutput)
if (!(Test-Path -LiteralPath $taskParent -PathType Container)) { throw 'Destination parent must exist' }
$taskParent = Resolve-ReleaseDirectory $taskParent
$taskOutput = [IO.Path]::Combine($taskParent,[IO.Path]::GetFileName($taskOutput))
if (Get-Item -LiteralPath $taskOutput -Force -ErrorAction SilentlyContinue) { throw 'Destination already exists; nothing was overwritten' }
$taskVersion = [regex]::Match((Get-Content -LiteralPath (Join-Path $taskRoot 'Cargo.toml') -Raw), '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+(?:-preview)?)"').Groups[1].Value
if (!$taskVersion) { throw 'Cannot determine package version' }
$taskInputs = [ordered]@{
    'swapctx.exe' = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Binary)
    'LICENSE' = (Join-Path $taskRoot 'LICENSE')
    'QUICKSTART.md' = (Join-Path $taskRoot 'docs/WINDOWS_RELEASE.md')
    'DEPENDENCIES.md' = (Join-Path $taskRoot 'docs/DEPENDENCIES.md')
    'Cargo.lock' = (Join-Path $taskRoot 'Cargo.lock')
    'THIRD_PARTY_NOTICES.txt' = (Join-Path $taskRoot 'THIRD_PARTY_NOTICES.txt')
    'third-party-inventory.json' = (Join-Path $taskRoot 'docs/third-party-inventory.json')
}
foreach ($taskInput in $taskInputs.Values) {
    $taskItem = Get-Item -LiteralPath $taskInput -Force
    if ($taskItem.PSIsContainer -or ($taskItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "Package input must be a regular file: $taskInput" }
}
$taskStage = Join-Path $taskParent ('.swapctx-package-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($taskStage)
$taskFailure = $null
try {
    # Copy first: validate and hash the exact bytes that will be distributed.
    $taskPayload = Join-Path $taskStage 'payload'
    [void][IO.Directory]::CreateDirectory($taskPayload)
    foreach ($taskName in $taskInputs.Keys) { [IO.File]::Copy($taskInputs[$taskName], (Join-Path $taskPayload $taskName), $false) }
    if ($BuildInfo) {
        $taskReceiptItem = Get-Item -LiteralPath $BuildInfo -Force
        if ($taskReceiptItem.PSIsContainer -or ($taskReceiptItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $taskReceiptItem.Length -gt 128KB) { throw 'Build receipt must be a small regular file' }
        $taskReceiptPath = Join-Path $taskStage 'build-info.json'
        [IO.File]::Copy($taskReceiptItem.FullName,$taskReceiptPath,$false)
        $taskReceipt = Get-Content -LiteralPath $taskReceiptPath -Raw | ConvertFrom-Json
        $taskBinaryHash = Get-TaskSha256 (Join-Path $taskPayload 'swapctx.exe')
        if ($taskReceipt.format_version -ne 1 -or $taskReceipt.target -cne 'x86_64-pc-windows-msvc' -or $taskReceipt.recipe -cne 'static-crt-brepro-v1' -or $taskReceipt.reproducible_on_this_toolchain -ne $true -or $taskReceipt.binary_sha256 -cne $taskBinaryHash -or @($taskReceipt.independent_binary_hashes).Count -ne 2 -or @($taskReceipt.independent_binary_hashes | Where-Object { $_ -cne $taskBinaryHash }).Count -ne 0 -or @($taskReceipt.pe.dynamic_crt_imports).Count -ne 0) { throw 'Build receipt does not describe a matching static-runtime release binary' }
        if ($taskReceipt.inputs -isnot [Array] -or $taskReceipt.inputs.Count -lt 3 -or $taskReceipt.reproducible_on_this_toolchain -isnot [bool] -or $taskReceipt.pe.machine -cne 'x86_64') { throw 'Incomplete build receipt' }
        if ($taskReceipt.recipe_sha256 -ine (Get-TaskSha256 (Join-Path $PSScriptRoot 'build-windows.cjs'))) { throw 'Build recipe changed since receipt generation' }
        $taskSeenSources = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($taskSource in $taskReceipt.inputs) {
            if ($taskSource.path -isnot [string] -or $taskSource.path -cnotmatch '^(Cargo\.(toml|lock)|rust-toolchain\.toml|(src|tests|integrations/(codex|native))/[A-Za-z0-9_./-]+)$' -or @($taskSource.path.Split('/') | Where-Object { $_ -eq '..' }).Count -ne 0) { throw 'Unsafe source path in build receipt' }
            if (!$taskSeenSources.Add($taskSource.path) -or $taskSource.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Invalid source entry in build receipt' }
            if ($taskSource.sha256 -ine (Get-TaskSha256 (Join-Path $taskRoot $taskSource.path))) { throw 'Source changed since release build; rebuild before packaging' }
        }
        foreach ($taskRequired in @('Cargo.toml','Cargo.lock','rust-toolchain.toml','src/main.rs','integrations/codex/session.cjs')) { if (!$taskSeenSources.Contains($taskRequired)) { throw 'Incomplete source inventory in build receipt' } }
        # Release producers have Node; recipients/installers do not need it.
        & node (Join-Path $PSScriptRoot 'validate-build-info.cjs') (Join-Path $taskPayload 'swapctx.exe') $taskReceiptPath $taskRoot
        if ($LASTEXITCODE -ne 0) { throw 'Independent build receipt inspection failed' }
    }
    $taskInventory = Get-Content -LiteralPath (Join-Path $taskPayload 'third-party-inventory.json') -Raw | ConvertFrom-Json
    if ($taskInventory.lock_sha256 -ine (Get-TaskSha256 (Join-Path $taskPayload 'Cargo.lock')) -or $taskInventory.notices_sha256 -ine (Get-TaskSha256 (Join-Path $taskPayload 'THIRD_PARTY_NOTICES.txt'))) { throw 'Third-party notices are stale or changed; regenerate and review' }
    $taskExecutable = Join-Path $taskPayload 'swapctx.exe'
    $taskReader = [IO.BinaryReader]::new([IO.File]::OpenRead($taskExecutable))
    try {
        if ($taskReader.ReadUInt16() -ne 0x5a4d) { throw 'Not a Windows PE executable' }
        $taskReader.BaseStream.Position = 0x3c
        $taskOffset = $taskReader.ReadUInt32()
        $taskReader.BaseStream.Position = $taskOffset
        if ($taskReader.ReadUInt32() -ne 0x4550 -or $taskReader.ReadUInt16() -ne 0x8664) { throw 'Expected an x64 Windows PE executable' }
    } finally { $taskReader.Dispose() }
    $taskReported = & $taskExecutable --version
    if ($LASTEXITCODE -ne 0 -or "$taskReported" -cne "swapctx $taskVersion") { throw 'Executable version does not match Cargo.toml' }
    $taskFiles = @()
    foreach ($taskName in $taskInputs.Keys) {
        $taskPath = Join-Path $taskPayload $taskName
        if ((Get-Item -LiteralPath $taskPath).Length -gt 32MB) { throw 'Payload exceeds installer file-size limit' }
        $taskFiles += [ordered]@{ path = $taskName; bytes = (Get-Item -LiteralPath $taskPath).Length; sha256 = (Get-TaskSha256 $taskPath) }
    }
    $taskManifest = [ordered]@{ format_version = 1; name = 'swapctx'; version = $taskVersion; target = 'x86_64-pc-windows-msvc'; license = 'Apache-2.0'; files = $taskFiles }
    [IO.File]::WriteAllText((Join-Path $taskPayload 'manifest.json'), ($taskManifest | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    if ((Get-ChildItem -LiteralPath $taskPayload -File | Measure-Object -Property Length -Sum).Sum -gt 64MB) { throw 'Payload exceeds installer total-size limit' }
    $taskArchiveName = "swapctx-$taskVersion-windows-x64.zip"
    $taskArchivePath = Join-Path $taskStage $taskArchiveName
    $taskArchive = [IO.Compression.ZipFile]::Open($taskArchivePath, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($taskName in (@($taskInputs.Keys) + @('manifest.json') | Sort-Object)) {
            $taskEntry = $taskArchive.CreateEntry($taskName, [IO.Compression.CompressionLevel]::Optimal)
            $taskEntry.LastWriteTime = [DateTimeOffset]::new(2000, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
            $taskSource = [IO.File]::OpenRead((Join-Path $taskPayload $taskName))
            try {
                $taskSink = $taskEntry.Open()
                try { $taskSource.CopyTo($taskSink) } finally { $taskSink.Dispose() }
            } finally { $taskSource.Dispose() }
        }
    } finally { $taskArchive.Dispose() }
    $taskHash = Get-TaskSha256 $taskArchivePath
    $taskInstaller = Join-Path $taskStage 'install-windows.ps1'
    [IO.File]::Copy((Join-Path $PSScriptRoot 'install-windows.ps1'),$taskInstaller,$false)
    $taskInstallerHash = Get-TaskSha256 $taskInstaller
    $taskChecksums = "$taskHash  $taskArchiveName`n$taskInstallerHash  install-windows.ps1`n"
    $taskCmdInstaller = Join-Path $taskStage 'install-windows.cmd'
    [IO.File]::Copy((Join-Path $PSScriptRoot 'install-windows.cmd'),$taskCmdInstaller,$false)
    $taskChecksums += (Get-TaskSha256 $taskCmdInstaller) + "  install-windows.cmd`n"
    $taskAcceptance = Join-Path $taskStage 'test-windows-preview.ps1'
    [IO.File]::Copy((Join-Path $PSScriptRoot 'test-windows-preview.ps1'),$taskAcceptance,$false)
    $taskChecksums += (Get-TaskSha256 $taskAcceptance) + "  test-windows-preview.ps1`n"
    if ($BuildInfo) { $taskChecksums += (Get-TaskSha256 $taskReceiptPath) + "  build-info.json`n" }
    [IO.File]::WriteAllText((Join-Path $taskStage 'SHA256SUMS.txt'), $taskChecksums, [Text.UTF8Encoding]::new($false))
    # Delete only our known staging child; publish both outputs in one directory rename.
    if (-not (Test-ReleaseDirectoryEqual ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskPayload))) $taskStage)) { throw 'Refusing cleanup outside staging directory' }
    if ((Get-Item -LiteralPath $taskPayload -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing cleanup of redirected payload directory' }
    Remove-Item -LiteralPath $taskPayload -Recurse -Force
    [IO.Directory]::Move($taskStage, $taskOutput)
    Write-Output "Packaged: $(Join-Path $taskOutput $taskArchiveName)"
    Write-Output 'Unsigned local candidate; checksums do not authenticate the publisher or prove verification.'
} catch {
    $taskFailure = $_
    throw
} finally {
    try {
        if (Test-Path -LiteralPath $taskStage) {
            $taskActualParent = Resolve-ReleaseDirectory ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskStage)))
            if (-not [String]::Equals($taskActualParent.TrimEnd('\'),$taskParent.TrimEnd('\'),[StringComparison]::OrdinalIgnoreCase)) { throw "Refusing cleanup outside staging parent: actual '$taskActualParent', expected '$taskParent'" }
            if ((Get-Item -LiteralPath $taskStage -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing cleanup of redirected staging directory' }
            Remove-Item -LiteralPath $taskStage -Recurse -Force
        }
    } catch {
        if ($taskFailure) { [Console]::Error.WriteLine('Additional cleanup failure (original error retained): ' + $_.Exception.Message) }
        else { throw }
    }
}
