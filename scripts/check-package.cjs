'use strict';
// Offline Windows distribution checks. Only the packaged SwapCtx CLI is executed.
const {test, after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
if (process.platform !== 'win32') throw new Error('Windows package checks require Windows');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'swapctx-package-check-'));
after(() => fs.rmSync(root, {recursive:true, force:true}));
const script = path.resolve(__dirname, 'package-windows.ps1');
const installer = path.resolve(__dirname,'install-windows.ps1');
const binary = path.resolve(process.argv[2] || 'target/release/swapctx.exe');
const version = fs.readFileSync(path.join(__dirname,'../Cargo.toml'),'utf8').match(/^version = "([^"]+)"/m)[1];
const destination = path.join(root,'candidate with spaces');
const extracted = path.join(root,'extracted with spaces');
const archiveName = `swapctx-${version}-windows-x64.zip`;
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function diagnostic(result, phase) {
  return `${phase}: status=${result.status}, signal=${result.signal || 'none'}, error=${result.error?.code || 'none'} ${result.error?.message || ''}\ncommand: ${JSON.stringify(result.command || result.error?.spawnargs || [])}\nstdout:\n${result.stdout || ''}\nstderr:\n${result.stderr || ''}`;
}
function ps(args) {
  const command = ['powershell.exe','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass',...args];
  const result = spawnSync(command[0],command.slice(1),{encoding:'utf8',timeout:60000});
  result.command = command;
  assert.ifError(result.error && new Error(diagnostic(result,'PowerShell launch failure')));
  assert.equal(result.signal,null,diagnostic(result,'PowerShell interrupted'));
  return result;
}
function pack(dest, input = binary) { return ps(['-File',script,'-Destination',dest,'-Binary',input]); }
function packWithScript(scriptPath, dest, input = binary) { return ps(['-File',scriptPath,'-Destination',dest,'-Binary',input]); }
function ok(result, phase = 'command') { assert.equal(result.status,0,diagnostic(result,phase)); return result.stdout; }
function install(archive, destination, expected = hash(archive)) {return ps(['-File',installer,'-Archive',archive,'-ExpectedSha256',expected,'-Destination',destination]);}
function modifiedArchive(label, change) {
  const dir = path.join(root,label+' payload'), archive = path.join(root,label+'.zip');
  fs.cpSync(extracted,dir,{recursive:true});
  change(dir);
  const script = path.join(root,'zip.ps1');
  fs.writeFileSync(script,"param([string]$Source,[string]$Archive)\n$ErrorActionPreference='Stop'\nAdd-Type -AssemblyName System.IO.Compression.FileSystem\n[IO.Compression.ZipFile]::CreateFromDirectory($Source,$Archive)\n");
  ok(ps(['-File',script,'-Source',dir,'-Archive',archive]));
  return archive;
}
function setupFixture() {
  fs.mkdirSync(path.resolve(__dirname,'../.build'),{recursive:true});
  ok(pack(destination),'initial packaging');
  assert.ok(fs.existsSync(path.join(destination,archiveName)),`Initial packaging returned success without ${archiveName}`);
  const expand = path.join(root,'expand.ps1');
  fs.writeFileSync(expand, "param([string]$Archive,[string]$Destination)\n$ErrorActionPreference='Stop'\nAdd-Type -AssemblyName System.IO.Compression.FileSystem\n[IO.Compression.ZipFile]::ExtractToDirectory($Archive,$Destination)\n");
  ok(ps(['-File',expand,'-Archive',path.join(destination,archiveName),'-Destination',extracted]),'initial extraction');
}
function registerPackageTests() {
test('portable package contains only the allowlisted payload and correct hashes', () => {
  assert.deepEqual(fs.readdirSync(destination).sort(), ['SHA256SUMS.txt','install-windows.ps1','test-windows-preview.ps1',archiveName].sort());
  assert.equal(fs.readFileSync(path.join(destination,'SHA256SUMS.txt'),'utf8'), `${hash(path.join(destination,archiveName))}  ${archiveName}\n${hash(path.join(destination,'install-windows.ps1'))}  install-windows.ps1\n${hash(path.join(destination,'test-windows-preview.ps1'))}  test-windows-preview.ps1\n`);
  assert.deepEqual(fs.readdirSync(extracted).sort(), ['Cargo.lock','DEPENDENCIES.md','LICENSE','QUICKSTART.md','manifest.json','swapctx.exe','THIRD_PARTY_NOTICES.txt','third-party-inventory.json'].sort());
  const manifest = JSON.parse(fs.readFileSync(path.join(extracted,'manifest.json'),'utf8'));
  assert.equal(manifest.format_version,1);
  assert.equal(manifest.version,version);
  assert.equal(manifest.license,'Apache-2.0');
  assert.equal(manifest.target,'x86_64-pc-windows-msvc');
  assert.equal(manifest.files.length,7);
  for (const file of manifest.files) {
    assert.equal(file.sha256,hash(path.join(extracted,file.path)));
    assert.equal(file.bytes,fs.statSync(path.join(extracted,file.path)).size);
  }
  assert.equal(hash(path.join(extracted,'swapctx.exe')),hash(binary));
  assert.match(fs.readFileSync(path.join(extracted,'LICENSE'),'utf8'),/Apache License/);
  const notices = JSON.parse(fs.readFileSync(path.join(extracted,'third-party-inventory.json'),'utf8'));
  const expectedNotices = JSON.parse(fs.readFileSync(path.join(__dirname,'../docs/third-party-inventory.json'),'utf8'));
  assert.deepEqual(notices,expectedNotices,'Packaged notices must exactly match the reviewed locked inventory');
  assert.equal(notices.lock_sha256,hash(path.join(extracted,'Cargo.lock')));
  assert.equal(notices.notices_sha256,hash(path.join(extracted,'THIRD_PARTY_NOTICES.txt')));
  const text = fs.readFileSync(path.join(extracted,'THIRD_PARTY_NOTICES.txt'),'utf8');
  assert.match(text,/UNICODE LICENSE V3/);
  assert.match(text,/Unicode Data Files/);
  assert.match(text,/The author disclaims copyright/);
});
test('packaged executable supports isolated nested-project retrieval, backup and restore', () => {
  const project = path.join(root,'project with spaces'), data = path.join(root,'private data');
  fs.mkdirSync(path.join(project,'src'),{recursive:true});
  const exe = path.join(extracted,'swapctx.exe');
  function cli(args, cwd = project, store = data) {
    return ok(spawnSync(exe,['--data-dir',store,...args],{cwd,encoding:'utf8',timeout:15000}));
  }
  assert.equal(cli(['--version']).trim(),`swapctx ${version}`);
  cli(['--help']);
  assert.equal(fs.existsSync(data),false,'informational commands must not create a store');
  cli(['init']);
  cli(['memory','add','--kind','requirement','Preserve portable releases']);
  const packet = JSON.parse(cli(['context'],path.join(project,'src')));
  assert.equal(packet.records.length,1);
  assert.equal(packet.records[0].content,'Preserve portable releases');
  const backup = path.join(root,'snapshot.db');
  cli(['backup',backup]);
  const recovered = path.join(root,'recovered');
  cli(['restore',backup],project,recovered);
  cli(['doctor'],project,recovered);
  assert.equal(JSON.parse(cli(['context'],project,recovered)).records[0].content,'Preserve portable releases');
  // Exercise both embedded Node modules without starting Codex or relying on
  // a source checkout next to the extracted executable.
  const {paths,identity,atomicJson,SUPPORTED} = require('../integrations/codex/session.cjs');
  const files = paths({project,dataDir:data},true);
  atomicJson(files.settings,{version:1,project:identity(project),dataDir:identity(data),codex:path.join(project,'DOES-NOT-EXIST.exe'),codexVersion:SUPPORTED,sandbox:'read-only'},false);
  const events = cli(['codex','--node',process.execPath,'--json','status']).trim().split('\n').map(JSON.parse);
  assert.equal(events.at(-1).event,'local_status');
  assert.equal(events.at(-1).offline,true);
  assert.equal(events.at(-1).context.records,1);
});
test('existing output is preserved and packaging leaves no staging files', () => {
  const before = hash(path.join(destination,archiveName));
  assert.notEqual(pack(destination).status,0);
  assert.equal(hash(path.join(destination,archiveName)),before);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-package-')),false);
});
test('invalid executable fails without publishing output or leaving staging files', () => {
  const invalid = path.join(root,'invalid.exe');
  fs.writeFileSync(invalid,'not an executable');
  const output = path.join(root,'invalid output');
  assert.notEqual(pack(output,invalid).status,0);
  assert.equal(fs.existsSync(output),false);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-package-')),false);
});
test('same inputs produce the same archive bytes on this toolchain', () => {
  const second = path.join(root,'second candidate');
  ok(pack(second));
  assert.equal(hash(path.join(second,archiveName)),hash(path.join(destination,archiveName)));
});
test('documented command locates the default release binary outside the source cwd', () => {
  const output = path.join(root,'default binary candidate');
  const result = spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',script,'-Destination',output],{cwd:path.dirname(binary),encoding:'utf8',timeout:60000});
  ok(result);
  assert.equal(fs.existsSync(path.join(output,archiveName)),true);
});
test('packaging does not depend on Get-FileHash cmdlet availability', () => {
  const source = path.join(root,'hash fallback source');
  fs.mkdirSync(path.join(source,'scripts'),{recursive:true});
  fs.mkdirSync(path.join(source,'docs'));
  const original = fs.readFileSync(script,'utf8');
  const insertionPoint = 'Set-StrictMode -Version Latest';
  assert.ok(original.includes(insertionPoint), 'PowerShell script strict-mode marker must remain present');
  fs.writeFileSync(path.join(source,'scripts/package-windows.ps1'), original.replace(insertionPoint, insertionPoint + "\nfunction Get-FileHash { throw 'GET_FILE_HASH_SHOULD_NOT_BE_CALLED' }"));
  fs.copyFileSync(path.join(__dirname,'install-windows.ps1'),path.join(source,'scripts/install-windows.ps1'));
  fs.copyFileSync(path.join(__dirname,'test-windows-preview.ps1'),path.join(source,'scripts/test-windows-preview.ps1'));
  for (const name of ['Cargo.toml','Cargo.lock','LICENSE','THIRD_PARTY_NOTICES.txt']) fs.copyFileSync(path.join(__dirname,'..',name),path.join(source,name));
  for (const name of ['WINDOWS_RELEASE.md','DEPENDENCIES.md','third-party-inventory.json']) fs.copyFileSync(path.join(__dirname,'../docs',name),path.join(source,'docs',name));
  const output = path.join(root,'hash fallback candidate');
  const result = packWithScript(path.join(source,'scripts/package-windows.ps1'),output,binary);
  ok(result,'hash fallback packaging');
  assert.equal(fs.existsSync(path.join(output,archiveName)),true);
  assert.doesNotMatch(result.stdout + result.stderr,/GET_FILE_HASH_SHOULD_NOT_BE_CALLED/);
});
test('cleanup parent guard accepts equivalent paths with different trailing separators', () => {
  const source = path.join(root,'trailing separator source');
  fs.mkdirSync(path.join(source,'scripts'),{recursive:true});
  fs.mkdirSync(path.join(source,'docs'));
  const marker = "$taskParent = Resolve-ReleaseDirectory $taskParent";
  const original = fs.readFileSync(script,'utf8');
  assert.ok(original.includes(marker),'task parent marker must remain explicit');
  fs.writeFileSync(path.join(source,'scripts/package-windows.ps1'), original.replace(marker, marker + "\n$taskParent = $taskParent + '\\'"));
  fs.copyFileSync(path.join(__dirname,'install-windows.ps1'),path.join(source,'scripts/install-windows.ps1'));
  fs.copyFileSync(path.join(__dirname,'test-windows-preview.ps1'),path.join(source,'scripts/test-windows-preview.ps1'));
  for (const name of ['Cargo.toml','Cargo.lock','LICENSE','THIRD_PARTY_NOTICES.txt']) fs.copyFileSync(path.join(__dirname,'..',name),path.join(source,name));
  for (const name of ['WINDOWS_RELEASE.md','DEPENDENCIES.md','third-party-inventory.json']) fs.copyFileSync(path.join(__dirname,'../docs',name),path.join(source,'docs',name));
  const output = path.join(root,'trailing separator candidate');
  ok(packWithScript(path.join(source,'scripts/package-windows.ps1'),output,binary),'trailing separator packaging');
  assert.equal(fs.existsSync(path.join(output,archiveName)),true);
});
test('distributed installer verifies every file and installs into a new directory', () => {
  const output = path.join(root,'installed application');
  ok(ps(['-File',path.join(destination,'install-windows.ps1'),'-Archive',path.join(destination,archiveName),'-ExpectedSha256',hash(path.join(destination,archiveName)),'-Destination',output]));
  assert.deepEqual(fs.readdirSync(output).sort(),fs.readdirSync(extracted).sort());
  for (const name of fs.readdirSync(output)) assert.equal(hash(path.join(output,name)),hash(path.join(extracted,name)));
  const versionResult = spawnSync(path.join(output,'swapctx.exe'),['--version'],{encoding:'utf8',timeout:10000});
  assert.equal(ok(versionResult).trim(),`swapctx ${version}`);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-install-')),false);
});
test('installer rejects wrong archive hash and preserves existing destinations', () => {
  const archive = path.join(destination,archiveName), output = path.join(root,'hash failure');
  assert.notEqual(install(archive,output,'0'.repeat(64)).status,0);
  assert.equal(fs.existsSync(output),false);
  const existing = path.join(root,'existing installation');
  fs.mkdirSync(existing);fs.writeFileSync(path.join(existing,'keep'),'unchanged');
  assert.notEqual(install(archive,existing).status,0);
  assert.equal(fs.readFileSync(path.join(existing,'keep'),'utf8'),'unchanged');
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-install-')),false);
});
test('installer rejects tampered payload even with a matching new archive hash', () => {
  const archive = modifiedArchive('tampered',dir => fs.appendFileSync(path.join(dir,'swapctx.exe'),'tamper'));
  const output = path.join(root,'tampered install');
  const result = install(archive,output);
  assert.notEqual(result.status,0);
  assert.match(result.stdout+result.stderr,/manifest hash or length|SHA-256 mismatch/);
  assert.equal(fs.existsSync(output),false);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-install-')),false);
});
test('installer refuses unknown entries and duplicate manifest paths', () => {
  const extra = modifiedArchive('extra',dir => fs.writeFileSync(path.join(dir,'extra.ps1'),'never execute'));
  const first = path.join(root,'extra install');
  assert.notEqual(install(extra,first).status,0);assert.equal(fs.existsSync(first),false);
  const duplicate = modifiedArchive('duplicate',dir => {
    const file = path.join(dir,'manifest.json'), manifest = JSON.parse(fs.readFileSync(file,'utf8'));
    manifest.files[1] = manifest.files[0];fs.writeFileSync(file,JSON.stringify(manifest));
  });
  const second = path.join(root,'duplicate install');
  const result = install(duplicate,second);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/duplicate manifest/);
  assert.equal(fs.existsSync(second),false);
});
test('installer rejects traversal names, duplicate ZIP entries and malformed archives', () => {
  const mutate = path.join(root,'mutate-zip.ps1');
  fs.writeFileSync(mutate,"param([string]$Archive,[string]$Entry)\n$ErrorActionPreference='Stop'\nAdd-Type -AssemblyName System.IO.Compression\nAdd-Type -AssemblyName System.IO.Compression.FileSystem\n$zip=[IO.Compression.ZipFile]::Open($Archive,[IO.Compression.ZipArchiveMode]::Update)\ntry { $zip.GetEntry('QUICKSTART.md').Delete(); [void]$zip.CreateEntry($Entry) } finally { $zip.Dispose() }\n");
  for (const [label,entry,error] of [['traversal','../outside.txt',/unsafe ZIP entry/],['zipduplicate','LICENSE',/Duplicate ZIP entry/]]) {
    const archive = path.join(root,label+'.zip');
    fs.copyFileSync(path.join(destination,archiveName),archive);
    ok(ps(['-File',mutate,'-Archive',archive,'-Entry',entry]));
    const output = path.join(root,label+' install'),result = install(archive,output);
    assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,error);
    assert.equal(fs.existsSync(output),false);
  }
  const invalid = path.join(root,'not-a-zip.zip');fs.writeFileSync(invalid,'invalid ZIP');
  const output = path.join(root,'invalid zip install');
  assert.notEqual(install(invalid,output).status,0);assert.equal(fs.existsSync(output),false);
  assert.equal(fs.existsSync(path.join(root,'outside.txt')),false);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-install-')),false);
});
test('packaging refuses notice bundles that no longer match their recorded hashes', () => {
  const source = path.join(root,'stale source');
  fs.mkdirSync(path.join(source,'scripts'),{recursive:true});
  fs.mkdirSync(path.join(source,'docs'));
  fs.copyFileSync(script,path.join(source,'scripts/package-windows.ps1'));
  for (const name of ['Cargo.toml','Cargo.lock','LICENSE','THIRD_PARTY_NOTICES.txt']) fs.copyFileSync(path.join(__dirname,'..',name),path.join(source,name));
  for (const name of ['WINDOWS_RELEASE.md','DEPENDENCIES.md','third-party-inventory.json']) fs.copyFileSync(path.join(__dirname,'../docs',name),path.join(source,'docs',name));
  fs.appendFileSync(path.join(source,'THIRD_PARTY_NOTICES.txt'),'changed');
  const output = path.join(root,'stale output');
  const result = ps(['-File',path.join(source,'scripts/package-windows.ps1'),'-Destination',output,'-Binary',binary]);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/notices are stale or changed/);
  assert.equal(fs.existsSync(output),false);
  assert.equal(fs.readdirSync(root).some(name => name.startsWith('.swapctx-package-')),false);
});
test('distributed PowerShell acceptance passes without Node/Rust/Codex on PATH',()=> {
  const report = path.join(root,'acceptance.json'), work = path.resolve(__dirname,'../.build/acceptance-'+crypto.randomUUID());
  const result = ps(['-File',path.join(destination,'test-windows-preview.ps1'),'-Archive',path.join(destination,archiveName),'-ExpectedSha256',hash(path.join(destination,archiveName)),'-Installer',path.join(destination,'install-windows.ps1'),'-InstallerSha256',hash(path.join(destination,'install-windows.ps1')),'-WorkDirectory',work,'-Report',report]);
  ok(result);
  const evidence = JSON.parse(fs.readFileSync(report,'utf8'));
  assert.equal(evidence.status,'passed');assert.equal(evidence.checks.length,16);
  assert.ok(evidence.checks.includes('development_tools_absent_from_path'));
  assert.ok(evidence.checks.includes('restored_context'));
  assert.equal(fs.existsSync(work),false);
});
test('acceptance refuses untrusted installer bytes and preserves prior reports',()=> {
  const report = path.join(root,'rejected-acceptance.json'), work = path.resolve(__dirname,'../.build/rejected-acceptance-'+crypto.randomUUID());
  const args = ['-File',path.join(destination,'test-windows-preview.ps1'),'-Archive',path.join(destination,archiveName),'-ExpectedSha256',hash(path.join(destination,archiveName)),'-Installer',path.join(destination,'install-windows.ps1'),'-InstallerSha256','0'.repeat(64),'-WorkDirectory',work,'-Report',report];
  assert.notEqual(ps(args).status,0);
  const evidence = JSON.parse(fs.readFileSync(report,'utf8'));
  assert.equal(evidence.status,'failed');assert.equal(evidence.phase,'verify_inputs');
  assert.deepEqual(evidence.checks,['archive_hash']);assert.equal(fs.existsSync(work),false);
  const before = hash(report);assert.notEqual(ps(args).status,0);assert.equal(hash(report),before);
});
test('short and long Windows parent spellings resolve to the same directory',t=> {
  const fixture = path.join(root,'alias-check.ps1');
  const source = fs.readFileSync(script,'utf8');
  const definition = source.match(/Add-Type -TypeDefinition @'\r?\n([\s\S]*?)\r?\n'@/)[1];
  fs.writeFileSync(fixture,"param([string]$Directory)\n$ErrorActionPreference='Stop'\nAdd-Type -TypeDefinition @'\n"+definition+"\n'@\n$short=(New-Object -ComObject Scripting.FileSystemObject).GetFolder($Directory).ShortPath\n$long=[SwapCtx.ReleaseDirectoryPaths]::Resolve($Directory)\n[pscustomobject]@{short=$short;long=$long;resolvedShort=[SwapCtx.ReleaseDirectoryPaths]::Resolve($short);resolvedLong=[SwapCtx.ReleaseDirectoryPaths]::Resolve($long);oldMixedParentComparison=([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath([IO.Path]::Combine($long,'child'))) -eq $short)} | ConvertTo-Json -Compress\n");
  const aliases = JSON.parse(ok(ps(['-File',fixture,'-Directory',root]),'Windows alias resolution'));
  assert.equal(aliases.resolvedShort.toLowerCase(),aliases.resolvedLong.toLowerCase());
  assert.equal(aliases.resolvedLong.toLowerCase(),fs.realpathSync.native(root).toLowerCase());
  if (aliases.short.toLowerCase() === aliases.long.toLowerCase()) {t.diagnostic('Filesystem exposes no differing 8.3 alias; canonical resolution still checked');return;}
  assert.equal(aliases.oldMixedParentComparison,false,'the previous mixed-spelling comparison rejects the same directory');
  const output = path.join(aliases.short,'alias candidate');ok(pack(output),'packaging through short alias');
  const zip = path.join(output,archiveName);assert.ok(fs.existsSync(zip));
  // Alternate spelling must still be recognized as an existing destination.
  const before = hash(zip);const result = pack(path.join(aliases.long,'alias candidate'));
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/Destination already exists/);assert.equal(hash(zip),before);
  const installed = path.join(aliases.short,'alias install');ok(install(zip,installed),'installation through short alias');
  assert.equal(hash(path.join(installed,'swapctx.exe')),hash(binary));
  assert.equal(fs.readdirSync(root).some(name=>name.startsWith('.swapctx-package-')||name.startsWith('.swapctx-install-')),false);
});
test('failed fixture setup reports the first PowerShell error without cascading ZIP errors',()=> {
  const missing = path.join(root,'MISSING-RELEASE.exe');
  const result = spawnSync(process.execPath,[__filename,missing],{encoding:'utf8',timeout:30000,maxBuffer:1024*1024});
  assert.equal(result.status,1,result.stdout+result.stderr);
  const output = result.stdout+result.stderr;
  assert.match(output,/package fixture setup failed/);assert.match(output,/initial packaging: status=1/);
  assert.match(output,/MISSING-RELEASE\.exe/);assert.match(output,/stderr:/);
  assert.doesNotMatch(output,/ENOENT:.*(?:\.zip|extracted with spaces)/);
  assert.match(output,/tests 1(?:\r?\n|$)/);
});
test('cleanup guards preserve the primary error and refuse redirected staging',()=> {
  for (const mode of ['parent mismatch','junction']) {
    const source = path.join(root,'cleanup '+mode);fs.mkdirSync(path.join(source,'scripts'),{recursive:true});fs.mkdirSync(path.join(source,'docs'));
    for (const name of ['Cargo.toml','Cargo.lock','LICENSE','THIRD_PARTY_NOTICES.txt']) fs.copyFileSync(path.join(__dirname,'..',name),path.join(source,name));
    for (const name of ['WINDOWS_RELEASE.md','DEPENDENCIES.md','third-party-inventory.json']) fs.copyFileSync(path.join(__dirname,'../docs',name),path.join(source,'docs',name));
    const outside = path.join(source,'outside');fs.mkdirSync(outside);fs.writeFileSync(path.join(outside,'keep.txt'),'preserve');
    const injection = mode === 'junction'
      ? "[IO.Directory]::Move($taskStage,$taskStage+'-saved')\n[void](New-Item -ItemType Junction -Path $taskStage -Target (Join-Path $taskRoot 'outside'))"
      : "$taskParent = Join-Path $taskParent 'wrong-parent'";
    const original = fs.readFileSync(script,'utf8');
    const marker = "[void][IO.Directory]::CreateDirectory($taskPayload)";
    assert.ok(original.includes(marker),'fault injection location must remain explicit');
    fs.writeFileSync(path.join(source,'scripts/package-windows.ps1'),original.replace(marker,marker+'\n'+injection+"\nthrow 'PRIMARY_PACKAGE_FAILURE'"));
    const output = path.join(source,'candidate');
    const result = ps(['-File',path.join(source,'scripts/package-windows.ps1'),'-Destination',output,'-Binary',binary]);
    assert.notEqual(result.status,0);
    assert.match(result.stderr,/PRIMARY_PACKAGE_FAILURE/);
    assert.match(result.stderr,/Additional cleanup failure \(original error retained\)/);
    assert.match(result.stderr,mode === 'junction' ? /Refusing cleanup of redirected staging/ : /Refusing cleanup outside staging parent/);
    assert.equal(fs.existsSync(output),false);assert.equal(fs.readFileSync(path.join(outside,'keep.txt'),'utf8'),'preserve');
  }
});
}
// Setup is a prerequisite, not an ordinary test that later tests can ignore.
// A failure registers one originating error and no dependent filesystem checks.
try {
  setupFixture();
  registerPackageTests();
} catch (error) {
  test('package fixture setup failed',()=>{throw error;});
}
