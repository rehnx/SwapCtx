# Local, offline installation into a NEW directory. Never runs package contents.
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedSha256,
    [Parameter(Mandatory = $true)][string]$Destination
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
    return (Resolve-ReleaseDirectory $Path).TrimEnd('\\')
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
if ($env:OS -ne 'Windows_NT') { throw 'Windows installation requires Windows' }
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
# PowerShell location can differ from the process working directory used by .NET.
$taskOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination)
$taskParent = [IO.Path]::GetDirectoryName($taskOutput)
if (!(Test-Path -LiteralPath $taskParent -PathType Container)) { throw 'Destination parent must exist' }
$taskParent = Resolve-ReleaseDirectory $taskParent
$taskOutput = [IO.Path]::Combine($taskParent,[IO.Path]::GetFileName($taskOutput))
if (Get-Item -LiteralPath $taskOutput -Force -ErrorAction SilentlyContinue) { throw 'Destination exists; choose a new installation directory' }
$taskInput = Get-Item -LiteralPath $Archive -Force
if ($taskInput.PSIsContainer -or ($taskInput.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $taskInput.Length -gt 64MB) { throw 'Archive must be a regular file no larger than 64 MiB' }
$taskStage = Join-Path $taskParent ('.swapctx-install-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($taskStage)
$taskFailure = $null
try {
    $taskSnapshot = Join-Path $taskStage 'input.zip'
    [IO.File]::Copy($taskInput.FullName, $taskSnapshot, $false)
    if ((Get-TaskSha256 $taskSnapshot) -ine $ExpectedSha256) { throw 'Archive SHA-256 mismatch; nothing installed' }
    $taskPayload = Join-Path $taskStage 'payload'
    [void][IO.Directory]::CreateDirectory($taskPayload)
    $taskNames = @('swapctx.exe','LICENSE','QUICKSTART.md','DEPENDENCIES.md','Cargo.lock','THIRD_PARTY_NOTICES.txt','third-party-inventory.json')
    $taskZip = [IO.Compression.ZipFile]::OpenRead($taskSnapshot)
    try {
        if ($taskZip.Entries.Count -ne $taskNames.Count + 1) { throw 'Unexpected ZIP entry count' }
        $taskEntries = [Collections.Generic.Dictionary[string,IO.Compression.ZipArchiveEntry]]::new([StringComparer]::Ordinal)
        $taskTotal = 0L
        foreach ($taskEntry in $taskZip.Entries) {
            if ($taskEntry.FullName -cnotin ($taskNames + @('manifest.json'))) { throw 'Unknown or unsafe ZIP entry name' }
            if ($taskEntries.ContainsKey($taskEntry.FullName)) { throw 'Duplicate ZIP entry' }
            $taskUnixKind = ($taskEntry.ExternalAttributes -shr 16) -band 0xf000
            if (($taskUnixKind -ne 0 -and $taskUnixKind -ne 0x8000) -or ($taskEntry.ExternalAttributes -band 0x410)) { throw 'ZIP links/directories are not supported' }
            if ($taskEntry.Length -gt 32MB) { throw 'ZIP entry exceeds size limit' }
            $taskTotal += $taskEntry.Length
            if ($taskTotal -gt 64MB) { throw 'ZIP payload exceeds size limit' }
            $taskEntries.Add($taskEntry.FullName,$taskEntry)
        }
        $taskManifestEntry = $taskEntries['manifest.json']
        if ($taskManifestEntry.Length -gt 128KB) { throw 'Manifest exceeds size limit' }
        $taskReader = [IO.StreamReader]::new($taskManifestEntry.Open(),[Text.UTF8Encoding]::new($false,$true))
        try { $taskManifestText = $taskReader.ReadToEnd() } finally { $taskReader.Dispose() }
        $taskManifest = $taskManifestText | ConvertFrom-Json
        if ($taskManifest.format_version -ne 1 -or $taskManifest.name -cne 'swapctx' -or $taskManifest.target -cne 'x86_64-pc-windows-msvc' -or $taskManifest.license -cne 'Apache-2.0' -or $taskManifest.version -cnotmatch '^\d+\.\d+\.\d+$') { throw 'Unsupported package manifest' }
        if ($taskManifest.files -isnot [Array] -or $taskManifest.files.Count -ne $taskNames.Count) { throw 'Incomplete manifest' }
        $taskSeen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($taskFile in $taskManifest.files) {
            if ($taskFile.path -isnot [string] -or $taskFile.path -cnotin $taskNames -or !$taskSeen.Add($taskFile.path)) { throw 'Unknown or duplicate manifest file' }
            if ($taskFile.sha256 -isnot [string] -or $taskFile.sha256 -cnotmatch '^[0-9a-f]{64}$' -or ($taskFile.bytes -isnot [int] -and $taskFile.bytes -isnot [long]) -or $taskFile.bytes -ne $taskEntries[$taskFile.path].Length) { throw 'Invalid manifest hash or length' }
            $taskSource = $taskEntries[$taskFile.path].Open()
            try {
                $taskFilePath = Join-Path $taskPayload $taskFile.path
                $taskSink = [IO.File]::Open($taskFilePath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write)
                try {
                    # Bound actual decoded bytes as well as the ZIP's declared length.
                    $taskBuffer = [byte[]]::new(65536)
                    $taskCopied = 0L
                    while (($taskRead = $taskSource.Read($taskBuffer,0,$taskBuffer.Length)) -gt 0) {
                        $taskCopied += $taskRead
                        if ($taskCopied -gt $taskFile.bytes) { throw 'ZIP decoded size exceeds manifest' }
                        $taskSink.Write($taskBuffer,0,$taskRead)
                    }
                    if ($taskCopied -ne $taskFile.bytes) { throw 'Truncated ZIP payload' }
                    $taskSink.Flush($true)
                } finally { $taskSink.Dispose() }
            } finally { $taskSource.Dispose() }
            if ((Get-TaskSha256 $taskFilePath) -ine $taskFile.sha256) { throw 'Payload SHA-256 mismatch; nothing installed' }
        }
        [IO.File]::WriteAllText((Join-Path $taskPayload 'manifest.json'),$taskManifestText,[Text.UTF8Encoding]::new($false))
    } finally { $taskZip.Dispose() }
    # Never merge with an existing installation or store. Move refuses races.
    [IO.Directory]::Move($taskPayload,$taskOutput)
    Write-Output "Installed verified payload: $taskOutput"
    Write-Output 'No executable was started. No PATH or user data was changed. Publisher authentication depends on the source of your expected hash.'
    Write-Output ('To enable swapctx in this PowerShell session: $env:PATH = ''' + ($taskOutput + ';').Replace("'","''") + ''' + $env:PATH')
} catch {
    $taskFailure = $_
    throw
} finally {
    try {
        if (Test-Path -LiteralPath $taskStage) {
            $taskActualParent = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskStage))
            if (-not (Test-ReleaseDirectoryEqual $taskActualParent $taskParent)) { throw "Refusing cleanup outside staging parent: actual '$(Resolve-ReleaseDirectory $taskActualParent)', expected '$(Resolve-ReleaseDirectory $taskParent)'" }
            if ((Get-Item -LiteralPath $taskStage -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing cleanup of redirected staging directory' }
            Remove-Item -LiteralPath $taskStage -Recurse -Force
        }
    } catch {
        if ($taskFailure) { [Console]::Error.WriteLine('Additional cleanup failure (original error retained): ' + $_.Exception.Message) }
        else { throw }
    }
}
