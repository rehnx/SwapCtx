'use strict';
// Offline PE and command-discovery checks, with isolated data and child PATH.
const {test,after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
const {inspect} = require('./inspect-pe.cjs');
if (process.platform !== 'win32') throw new Error('Windows release checks require Windows');
fs.mkdirSync(path.resolve(__dirname,'../.build'),{recursive:true});
const root = fs.mkdtempSync(path.resolve(__dirname,'../.build/command-discovery-'));
after(()=>fs.rmSync(root,{recursive:true,force:true}));
const binary = path.resolve(process.argv[2] || 'target/release/swapctx.exe');
const bytes = fs.readFileSync(binary);
test('PE import inspection is read-only and identifies CRT imports',()=> {
  const before = crypto.createHash('sha256').update(bytes).digest('hex');
  const result = inspect(bytes);
  assert.equal(result.machine,'x86_64');
  assert.ok(result.imports.includes('KERNEL32.DLL'));
  assert.ok(result.dynamic_crt_imports.every(name=>result.imports.includes(name)||result.delay_imports.includes(name)));
  if (process.argv.includes('--static')) assert.deepEqual(result.dynamic_crt_imports,[]);
  assert.equal(crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex'),before);
});
test('PE inspection rejects truncated, foreign and invalid directory inputs',()=> {
  for (const length of [0,1,64,256]) assert.throws(()=>inspect(bytes.subarray(0,length)));
  const foreign = Buffer.from(bytes);foreign.writeUInt16LE(0xaa64,foreign.readUInt32LE(0x3c)+4);
  assert.throws(()=>inspect(foreign),/Expected x64/);
  const invalid = Buffer.from(bytes);invalid.writeUInt32LE(0xffffffff,invalid.readUInt32LE(0x3c)+24+112+8);
  assert.throws(()=>inspect(invalid),/Unmapped/);
});
const installation = path.join(root,'application with spaces');fs.mkdirSync(installation);
fs.copyFileSync(binary,path.join(installation,'swapctx.exe'));
const env = {...process.env,PATH:installation+path.delimiter+process.env.PATH};
const data = path.join(root,'isolated data');
function run(cwd,args) {
  const result = spawnSync('swapctx.exe',['--data-dir',data,...args],{cwd,env,encoding:'utf8',timeout:15000});
  assert.equal(result.status,0,result.stdout+result.stderr+String(result.error||''));return result.stdout;
}
test('bare swapctx command resolves through PATH and keeps project discovery scoped',()=> {
  for (const name of ['first project','second project']) {
    const project = path.join(root,name), nested = path.join(project,'src/deep');
    fs.mkdirSync(nested,{recursive:true});
    run(project,['init']);run(project,['memory','add','--kind','requirement',name]);
    const packet = JSON.parse(run(nested,['context']));
    assert.equal(packet.records.length,1);assert.equal(packet.records[0].content,name);
  }
});
test('documented PowerShell session PATH works from a nested project with spaces',()=> {
  const script = path.join(root,'path-check.ps1');
  fs.writeFileSync(script,"param([string]$InstallDirectory,[string]$Project,[string]$Data)\n$ErrorActionPreference='Stop'\n$env:PATH=$InstallDirectory+';'+$env:PATH\nif ((Get-Command swapctx -CommandType Application).Source -ine (Join-Path $InstallDirectory 'swapctx.exe')) { throw 'Wrong executable resolved' }\nSet-Location -LiteralPath $Project\nswapctx --data-dir $Data context\nif ($LASTEXITCODE -ne 0) { throw 'Context command failed' }\n");
  const result = spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',script,'-InstallDirectory',installation,'-Project',path.join(root,'second project/src/deep'),'-Data',data],{encoding:'utf8',timeout:15000});
  assert.equal(result.status,0,result.stdout+result.stderr);
  assert.equal(JSON.parse(result.stdout).records[0].content,'second project');
});
