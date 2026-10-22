'use strict';
// Offline receipt/packaging boundary checks. Never invokes an AI agent.
const {test,after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
if (!process.argv[2] || !process.argv[3]) throw new Error('Usage: node scripts/check-build-receipt.cjs BINARY BUILD_INFO');
const binary = path.resolve(process.argv[2]), receipt = path.resolve(process.argv[3]);
const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-build-receipt-'));
after(()=>fs.rmSync(root,{recursive:true,force:true}));
const script = path.resolve(__dirname,'package-windows.ps1');
const hash = file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function pack(destination,info = receipt) {
  return spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',script,'-Destination',destination,'-Binary',binary,'-BuildInfo',info],{encoding:'utf8',timeout:60000});
}
test('release package carries a receipt matching the binary and current source inputs',()=> {
  const output = path.join(root,'candidate');const result = pack(output);
  assert.equal(result.status,0,result.stdout+result.stderr);
  const copied = path.join(output,'build-info.json');
  assert.equal(hash(copied),hash(receipt));
  const info = JSON.parse(fs.readFileSync(copied,'utf8'));
  assert.equal(info.binary_sha256,hash(binary));
  assert.deepEqual(info.independent_binary_hashes,[hash(binary),hash(binary)]);
  assert.deepEqual(info.pe.dynamic_crt_imports,[]);
  assert.ok(fs.readFileSync(path.join(output,'SHA256SUMS.txt'),'utf8').includes(hash(copied)+'  build-info.json\n'));
});
test('receipt with a different binary hash fails without publishing output',()=> {
  const info = JSON.parse(fs.readFileSync(receipt,'utf8'));info.binary_sha256='0'.repeat(64);
  const changed = path.join(root,'wrong-binary.json');fs.writeFileSync(changed,JSON.stringify(info));
  const output = path.join(root,'wrong binary'),result = pack(output,changed);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/matching static-runtime/);
  assert.equal(fs.existsSync(output),false);
});
test('changed source or recipe hashes fail without publishing output',()=> {
  for (const [label,change,error] of [
    ['source',info=>{info.inputs[0].sha256='0'.repeat(64);},/Source changed since release build/],
    ['recipe',info=>{info.recipe_sha256='0'.repeat(64);},/Build recipe changed/]
  ]) {
    const info = JSON.parse(fs.readFileSync(receipt,'utf8'));change(info);
    const changed = path.join(root,label+'.json');fs.writeFileSync(changed,JSON.stringify(info));
    const output = path.join(root,label+' candidate'),result = pack(output,changed);
    assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,error);
    assert.equal(fs.existsSync(output),false);
  }
  assert.equal(fs.readdirSync(root).some(name=>name.startsWith('.swapctx-package-')),false);
});
test('empty receipt source inventory fails instead of bypassing source checks',()=> {
  const info = JSON.parse(fs.readFileSync(receipt,'utf8'));info.inputs=[];
  const changed = path.join(root,'empty.json');fs.writeFileSync(changed,JSON.stringify(info));
  const output = path.join(root,'empty candidate'),result = pack(output,changed);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/Incomplete build receipt/);
  assert.equal(fs.existsSync(output),false);
});
test('partial nonempty source inventory cannot bypass complete source verification',()=> {
  const info = JSON.parse(fs.readFileSync(receipt,'utf8'));info.inputs=info.inputs.filter(input=>input.path!=='src/memory.rs');
  const changed = path.join(root,'partial.json');fs.writeFileSync(changed,JSON.stringify(info));
  const output = path.join(root,'partial candidate'),result = pack(output,changed);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/Incomplete or changed release source inventory/);
  assert.equal(fs.existsSync(output),false);
});
test('receipt PE claims are independently checked against actual executable imports',()=> {
  const info = JSON.parse(fs.readFileSync(receipt,'utf8'));info.pe.imports=[];
  const changed = path.join(root,'false-pe.json');fs.writeFileSync(changed,JSON.stringify(info));
  const output = path.join(root,'false pe candidate'),result = pack(output,changed);
  assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,/PE metadata does not match/);
  assert.equal(fs.existsSync(output),false);
});
test('release receipts require native tests and current helper hashes',()=> {
  for (const [label,change,error] of [
    ['untested',info=>{info.native_release_tests_run=false;},/requires native release tests/],
    ['tools',info=>{info.tools[0].sha256='0'.repeat(64);},/tool inventory changed or missing/]
  ]) {
    const info = JSON.parse(fs.readFileSync(receipt,'utf8'));change(info);
    const changed = path.join(root,label+'.json');fs.writeFileSync(changed,JSON.stringify(info));
    const output = path.join(root,label+' candidate'),result = pack(output,changed);
    assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,error);
    assert.equal(fs.existsSync(output),false);
  }
});
test('receipt rejects pre-test mutation and missing native toolchain identity',()=> {
  for(const [label,change,error] of [
    ['mutation',info=>{info.build_observations[0].before='0'.repeat(64);},/Build hashes differ/],
    ['native',info=>{delete info.native_toolchain;},/native toolchain identity/],
    ['native-hash',info=>{info.native_toolchain.identity.artifacts[0].sha256='0'.repeat(64);},/native toolchain identity/],
  ]) {
    const info=JSON.parse(fs.readFileSync(receipt,'utf8'));change(info);
    const changed=path.join(root,label+'.json');fs.writeFileSync(changed,JSON.stringify(info));
    const output=path.join(root,label+' candidate'),result=pack(output,changed);
    assert.notEqual(result.status,0);assert.match(result.stdout+result.stderr,error);
    assert.equal(fs.existsSync(output),false);
  }
});
