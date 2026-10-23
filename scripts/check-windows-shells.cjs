'use strict';
// Offline Windows regressions. All providers and installations are disposable.
const {test, after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
if (process.platform !== 'win32') throw new Error('Windows shell checks require Windows');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'swapctx shells ü 世界 & (offline)-'));
after(() => fs.rmSync(root, {recursive:true, force:true}));
const binary = path.resolve(process.argv[2] || 'target/release/swapctx.exe');
const cmd = path.join(process.env.SystemRoot, 'System32/cmd.exe');
const ps51 = path.join(process.env.SystemRoot, 'System32/WindowsPowerShell/v1.0/powershell.exe');
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const resultText = r => `${r.error || ''}\n${r.stdout}\n${r.stderr}`;
function run(exe, args, env = {}, extra = {}) {
  const r = spawnSync(exe, args, {cwd:root, env:{...process.env, ...env}, encoding:'utf8', timeout:60000, ...extra});
  assert.ifError(r.error); assert.equal(r.signal, null, resultText(r)); return r;
}
function ps(exe, code, env = {}) {
  return run(exe, ['-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',
    Buffer.from("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); " + code, 'utf16le').toString('base64')], env);
}
function batch(code, env = {}) {
  // Values travel through quoted environment references, never shell source.
  // /v:off and no CALL avoid delayed expansion or a second percent expansion.
  return run(cmd, ['/d','/v:off','/s','/c', '"' + code + '"'], env, {windowsVerbatimArguments:true});
}
function ok(r) { assert.equal(r.status, 0, resultText(r)); return r.stdout; }
const shells = [{name:'CMD', cmd:true}, {name:'Windows PowerShell 5.1', exe:ps51}];
const pwsh = spawnSync('pwsh.exe', ['-NoLogo','-NoProfile','-NonInteractive','-Command','exit 0'], {timeout:15000});
if (pwsh.error?.code === 'ENOENT') test('PowerShell 7 compatibility', {skip:'pwsh.exe is not installed'}, () => {});
else {
  assert.ifError(pwsh.error); assert.equal(pwsh.status,0,'Installed pwsh.exe must start successfully');
  shells.push({name:'PowerShell 7', exe:'pwsh.exe'});
}

const candidate = path.join(root, 'candidate');
ok(ps(ps51, '& $env:SC_PACKAGE -Destination $env:SC_DEST -Binary $env:SC_BINARY; exit $LASTEXITCODE', {
  SC_PACKAGE:path.join(__dirname,'package-windows.ps1'), SC_DEST:candidate, SC_BINARY:binary,
}));
const archive = path.join(candidate, fs.readdirSync(candidate).find(name => name.endsWith('.zip')));
let sequence = 0;
function install(shell, dest, expected = hash(archive), input = archive) {
  const env = {SC_INSTALLER:path.join(candidate, shell.cmd ? 'install-windows.cmd' : 'install-windows.ps1'),
    SC_ARCHIVE:input, SC_HASH:expected, SC_DEST:dest};
  return shell.cmd
    ? batch('"%SC_INSTALLER%" -Archive "%SC_ARCHIVE%" -ExpectedSha256 "%SC_HASH%" -Destination "%SC_DEST%"', env)
    : ps(shell.exe, '& $env:SC_INSTALLER -Archive $env:SC_ARCHIVE -ExpectedSha256 $env:SC_HASH -Destination $env:SC_DEST; exit $LASTEXITCODE', env);
}
for (const shell of shells) {
  test(`${shell.name}: fresh/repeated installation, spaces, Unicode and unchanged payload`, () => {
    const dest = path.join(root, 'installation ü 世界 ' + (++sequence));
    const output = ok(install(shell, dest));
    assert.equal(hash(path.join(dest,'swapctx.exe')), hash(binary));
    fs.writeFileSync(path.join(dest,'existing-user-file'),'preserve installation contents');
    assert.match(output, /CMD .*set "PATH=/);
    assert.match(output, /PowerShell 5.1\/7: \$env:PATH/);
    assert.equal(install(shell, dest).status, 1);
    assert.equal(hash(path.join(dest,'swapctx.exe')), hash(binary));
    assert.equal(fs.readFileSync(path.join(dest,'existing-user-file'),'utf8'),'preserve installation contents');
    const failed = path.join(root, 'failed ' + sequence);
    const failure = install(shell, failed, '0'.repeat(64));
    assert.equal(failure.status, 1); assert.match(resultText(failure), /SHA-256 mismatch/);
    assert.equal(fs.existsSync(failed), false);
    assert.equal(fs.readdirSync(root).some(n => n.startsWith('.swapctx-install-')), false);
  });

  test(`${shell.name}: PATH discovery, core memory, redirection, NO_COLOR and native exit status`, () => {
    const installDir = path.join(root, 'lookup ü 世界 ' + (++sequence));
    fs.mkdirSync(installDir); fs.copyFileSync(binary, path.join(installDir,'swapctx.exe'));
    const data = path.join(root, 'data ' + sequence);
    const env = {SC_INSTALL:installDir, SC_DATA:data, NO_COLOR:'1', SC_FAKE:path.join(root,'fake provider.exe')};
    const invoke = args => shell.cmd
      ? batch('set "PATH=%SC_INSTALL%;%PATH%" & swapctx ' + args, env)
      : ps(shell.exe, '$env:PATH=$env:SC_INSTALL+\';\'+$env:PATH; swapctx ' + args + '; exit $LASTEXITCODE', env);
    const dataArg = shell.cmd ? '"%SC_DATA%"' : '$env:SC_DATA';
    assert.match(ok(invoke('--version')), /^swapctx /);
    ok(invoke(`--data-dir ${dataArg} init`));
    ok(invoke(`--data-dir ${dataArg} memory add --kind requirement "Preserve shell compatibility"`));
    const packet = JSON.parse(ok(invoke(`--data-dir ${dataArg} context`)));
    assert.equal(packet.records[0].content, 'Preserve shell compatibility');
    const status = ok(invoke(`--data-dir ${dataArg} status`));
    assert.ok(!status.includes('\x1b'));
    const menu = invoke(`--data-dir ${dataArg}`);
    assert.equal(menu.status, 1); assert.match(resultText(menu), /interactive terminal/);
    assert.ok(!resultText(menu).includes('\x1b'));
    const fakeArg = shell.cmd ? '"%SC_FAKE%"' : '$env:SC_FAKE';
    const launched = invoke(`--data-dir ${dataArg} native codex --executable ${fakeArg}`);
    assert.equal(launched.status, 37, resultText(launched));
    assert.match(launched.stdout, /synthetic-native-terminal/);
    assert.equal(JSON.parse(ok(invoke(`--data-dir ${dataArg} context`))).records[0].content, packet.records[0].content);
  });
}

// A tiny native fake verifies exit propagation without a provider or Node hook.
const fakeSource = path.join(root, 'fake.rs');
fs.writeFileSync(fakeSource, 'fn main() { println!("synthetic-native-terminal"); std::process::exit(37); }\n');
ok(run('rustc', ['--edition=2021',fakeSource,'-o',path.join(root,'fake provider.exe')], {}, {cwd:path.resolve(__dirname,'..')}));

test('CMD wrapper uses its adjacent verified installer and propagates arbitrary exit codes', () => {
  const dir = path.join(root, 'wrapper literal ü & (offline)'); fs.mkdirSync(dir);
  const wrapper = path.join(dir,'install-windows.cmd');
  fs.copyFileSync(path.join(candidate,'install-windows.cmd'), wrapper);
  fs.copyFileSync(path.join(root,'fake provider.exe'),path.join(dir,'powershell.exe'));
  const missing = batch('"%SC_WRAPPER%"', {SC_WRAPPER:wrapper});
  assert.equal(missing.status, 2); assert.match(resultText(missing), /beside install-windows.cmd/);
  // This isolated companion is a test stub, never distributed as an installer.
  fs.writeFileSync(path.join(dir,'install-windows.ps1'),
    'param([string]$Archive,[string]$ExpectedSha256,[string]$Destination)\n' +
    'if ($Archive -cne $env:SC_LITERAL -or $Destination -cne $env:SC_LITERAL) { exit 99 }; exit 37\n');
  for (const shell of shells) {
    // PowerShell's own .cmd invocation goes through CMD before our wrapper.
    // Its caller must obey CMD percent-expansion rules; the wrapper itself
    // must not introduce a second expansion of CMD environment references.
    const literal = shell.cmd ? "space ü 世界 & () ! %SC_EXPAND% ' $()" : "space ü 世界 & () ' $()";
    const env = {SC_WRAPPER:wrapper, SC_LITERAL:literal, SC_EXPAND:'MUST_NOT_EXPAND', PATH:dir + path.delimiter + process.env.PATH};
    const r = shell.cmd
      ? batch('"%SC_WRAPPER%" -Archive "%SC_LITERAL%" -ExpectedSha256 abc -Destination "%SC_LITERAL%"', env)
      : ps(shell.exe, '& $env:SC_WRAPPER -Archive $env:SC_LITERAL -ExpectedSha256 abc -Destination $env:SC_LITERAL; exit $LASTEXITCODE', env);
    assert.equal(r.status, 37, shell.name + resultText(r));
    assert.doesNotMatch(r.stdout, /synthetic-native-terminal/, 'wrapper must not use a powershell.exe found on PATH');
  }
});

test('every installer host rejects altered payloads even with the correct outer ZIP hash', () => {
  const dir = path.join(root,'tamper source'), zip = path.join(root,'tampered.zip');
  ok(install(shells[0],dir));
  const exe = path.join(dir,'swapctx.exe'), bytes = fs.readFileSync(exe);
  bytes[bytes.length - 1] ^= 1; fs.writeFileSync(exe,bytes);
  ok(ps(ps51, 'Add-Type -AssemblyName System.IO.Compression.FileSystem; [IO.Compression.ZipFile]::CreateFromDirectory($env:SC_SOURCE,$env:SC_ZIP)', {SC_SOURCE:dir, SC_ZIP:zip}));
  for (const shell of shells) {
    const dest = path.join(root,'tampered destination ' + (++sequence));
    const r = install(shell,dest,hash(zip),zip);
    assert.equal(r.status,1); assert.match(resultText(r), /Payload SHA-256 mismatch/);
    assert.equal(fs.existsSync(dest),false);
  }
});

const consoleHarness = path.join(root,'menu console.exe');
ok(run('rustc', ['--edition=2021',path.join(__dirname,'../tests/fixtures/windows-menu-console.rs'),'-o',consoleHarness], {}, {cwd:path.resolve(__dirname,'..')}));
const fakeBin = path.join(root,'synthetic provider bin'); fs.mkdirSync(fakeBin);
fs.copyFileSync(path.join(root,'fake provider.exe'),path.join(fakeBin,'codex.exe'));
for (const shell of shells) {
  for (const mode of ['legacy','no-color','no-color-empty','dumb','styled','handoff']) {
    test(`${shell.name}: interactive ${mode} menu on console handles`, () => {
      const script = path.join(root, 'menu ' + (++sequence) + (shell.cmd ? '.cmd' : '.ps1'));
      const report = path.join(root, 'screen ' + sequence + '.txt');
      fs.writeFileSync(script, shell.cmd
        ? '@echo off\r\n"%SC_BINARY%" --data-dir "%SC_DATA%"\r\nexit /b %errorlevel%\r\n'
        : '& $env:SC_BINARY --data-dir $env:SC_DATA\nexit $LASTEXITCODE\n');
      const data = path.join(root,'menu missing store ' + sequence);
      ok(run(consoleHarness, [shell.cmd ? cmd : shell.exe,script,report,mode], {
        SC_BINARY:binary, SC_DATA:data, PATH:fakeBin + path.delimiter + process.env.PATH,
      }));
      const screen = fs.readFileSync(report,'utf8');
      assert.ok(!screen.includes('\x1b'), 'no literal escape characters in the console screen');
      if (mode === 'styled') {
        assert.match(screen, /Choose your coding agent:/);
        assert.match(screen, /Goodbye!/);
      } else {
        assert.match(screen, /SwapCtx - choose your coding agent/);
        assert.ok(/^[\x00-\x7f]*$/.test(screen), 'fallback UI must contain only ASCII');
        if (mode === 'handoff') {
          assert.match(screen, /Expose selected SwapCtx project memory/);
          assert.match(screen, /synthetic-native-terminal/);
        }
      }
      assert.equal(fs.existsSync(data), false, 'menu must not create a memory store');
    });
  }
}
