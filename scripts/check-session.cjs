'use strict';
const {test,after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const {AppServer,ManagedSession,atomicJson,context} = require('../integrations/codex/session.cjs');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-session-tests-'));
const exe = path.resolve(process.argv[2] || (process.platform === 'win32'?'target/debug/swapctx.exe':'target/debug/swapctx'));
const server = `
const readline=require('node:readline');const mode=process.env.FIXTURE_MODE;let first;
readline.createInterface({input:process.stdin}).on('line',line=>{
 const m=JSON.parse(line);if(m.id===987&&m.error){console.log(JSON.stringify({method:'fixture/approval_denied',params:{code:m.error.code}}));return;}
 if(m.id===undefined)return;
 if(mode==='permission'||(mode==='late_permission'&&m.method==='exit')){process.stderr.write('Access is denied (os error 5) password=DoNotExposeThisBackendSecret',()=>process.exit(5));return;}
 if(mode==='permission_fragment'){process.stderr.write('Access is ');setTimeout(()=>process.stderr.write('denied password=DoNotExposeThisBackendSecret',()=>process.exit(5)),20);return;}
 if(mode==='exit'){process.exit(5);return;}
 if(mode==='hang')return;
 if(mode==='malformed'){console.log('not JSON');return;}
 if(mode==='utf8'){process.stdout.write(Buffer.from([255,10]));return;}
 if(mode==='oversize'){process.stdout.write('x'.repeat(8388609));return;}
 if(mode==='rpc_error'){console.log(JSON.stringify({id:m.id,error:{code:-32000,message:'password=DoNotExposeThisBackendSecret'}}));return;}
 if(mode==='reorder'){if(!first){first=m;return;}console.log(JSON.stringify({id:m.id,result:{value:2}}));console.log(JSON.stringify({id:first.id,result:{value:1}}));return;}
 if(mode==='request')console.log(JSON.stringify({id:987,method:'item/commandExecution/requestApproval',params:{}}));
 const out=Buffer.from(JSON.stringify({id:m.id,result:{codexHome:process.cwd(),value:'ü unicode'}})+'\\n');
 if(mode==='fragment')for(const byte of out)process.stdout.write(Buffer.from([byte]));else process.stdout.write(out);
});`;
fs.writeFileSync(path.join(root,'app-server'),server);
function api(mode,timeout=1000) {
  const env={FIXTURE_MODE:mode};for(const key of ['SystemRoot','WINDIR','PATH','TEMP','TMP'])if(process.env[key])env[key]=process.env[key];
  return new AppServer(process.execPath,root,env,timeout);
}
for(const [mode,code] of [['malformed','codex_protocol_invalid'],['utf8','codex_protocol_invalid'],['oversize','codex_protocol_oversized'],['exit','codex_connection_closed'],['hang','codex_rpc_timeout'],['rpc_error','codex_rpc_failed'],['permission','codex_startup_permission_denied'],['permission_fragment','codex_startup_permission_denied']]) {
  test('transport fails transparently on '+mode,async()=>{
    const p=api(mode,mode==='hang'?150:2000);
    try {await assert.rejects(p.initialize(),e=>{assert.equal(e.code,code);assert.ok(!e.message.includes('DoNotExpose'));return true;});}
    finally {await p.close();}
  });
}
test('permission-like stderr after initialization is not misreported as a startup failure',async()=>{
  const p=api('late_permission');try {await p.initialize();await assert.rejects(p.call('exit',{}),e=>e.code==='codex_connection_closed'&&!e.message.includes('DoNotExpose'));} finally {await p.close();}
});
test('UTF-8 framing survives byte-by-byte fragmentation',async()=>{
  const p=api('fragment');try {assert.equal((await p.initialize()).value,'ü unicode');} finally {await p.close();}
});
test('out-of-order concurrent responses resolve their own request IDs',async()=>{
  const p=api('reorder');try {const [a,b]=await Promise.all([p.call('first',{}),p.call('second',{})]);assert.equal(a.value,1);assert.equal(b.value,2);} finally {await p.close();}
});
test('interactive approval requests are explicitly rejected, never granted',async()=>{
  const p=api('request');const observed=[];
  p.on('notification',m=>observed.push(m));
  try {await p.initialize();await new Promise(r=>setTimeout(r,100));assert.ok(observed.some(m=>m.method==='fixture/approval_denied'&&m.params.code===-32601));} finally {await p.close();}
});
test('turn waiter rejects promptly when transport disconnects',async()=>{
  const p=api('exit');const waiter=p.waitTurn('thread');
  const checked=assert.rejects(waiter.promise,e=>e.code==='codex_connection_closed');
  try {await assert.rejects(p.call('exit',{}));await checked;} finally {await p.close();}
});
test('atomic metadata publication never overwrites an existing config',()=>{
  const file=path.join(root,'config.json');atomicJson(file,{keep:'original'},false);
  assert.throws(()=>atomicJson(file,{replace:'wrong'},false));assert.deepEqual(JSON.parse(fs.readFileSync(file,'utf8')),{keep:'original'});assert.ok(!fs.readdirSync(root).some(n=>n.endsWith('.tmp')));
});
test('failed metadata publication preserves unrelated paths and cleans staging',()=>{
  const file=path.join(root,'directory');fs.mkdirSync(file);fs.writeFileSync(path.join(file,'keep'),'untouched');
  assert.throws(()=>atomicJson(file,{wrong:true}));assert.equal(fs.readFileSync(path.join(file,'keep'),'utf8'),'untouched');assert.ok(!fs.readdirSync(root).some(n=>n.endsWith('.tmp')));
});
test('context fingerprints do not mutate the v1 packet and remain stable across reads',()=>{
  const project=path.join(root,'project'),data=path.join(root,'data');fs.mkdirSync(project);
  for(const args of [['init'],['memory','add','ordinary requirement','--kind','requirement']]) {const r=spawnSync(exe,['--data-dir',data,...args],{cwd:project,encoding:'utf8'});assert.equal(r.status,0,r.stderr);}
  const a=context({swapctx:exe,dataDir:data,project}),b=context({swapctx:exe,dataDir:data,project});
  assert.ok(Number.isSafeInteger(a.packet.budget.bytes_used));assert.equal(a.hash,b.hash);assert.ok(a.packet.budget.bytes_used<8192);
});
test('tool outcomes are observable without echoing commands or output',()=>{
  const events=[],session=new ManagedSession({}, {},'',event=>events.push(event));
  session.observe({method:'item/completed',params:{item:{type:'commandExecution',status:'completed',exitCode:1,command:'password=SECRET',aggregatedOutput:'SECRET'}}});
  session.observe({method:'item/completed',params:{item:{type:'fileChange',status:'failed'}}});
  session.observe({method:'item/completed',params:{item:{type:'mcpToolCall',status:'SECRET'}}});
  assert.deepEqual(events,[{event:'tool_completed',kind:'commandExecution',status:'completed',exitCode:1},{event:'tool_completed',kind:'fileChange',status:'failed'},{event:'tool_completed',kind:'mcpToolCall',status:'unknown'}]);
});

test('failed retrieval reports measured phase without implying model delivery',async()=>{
  const events=[],project=path.join(root,'missing-context-project');fs.mkdirSync(project);
  const session=new ManagedSession({swapctx:exe,dataDir:root,project},{},path.join(root,'never-created-state'),event=>events.push(event));
  await assert.rejects(session.perform('must not launch'),e=>e.code==='context_unavailable');
  assert.equal(events.length,1);assert.equal(events[0].event,'operation_metrics');assert.equal(events[0].success,false);assert.equal(events[0].phase,'context');assert.equal(events[0].sessionMs,null);assert.equal(events[0].turnMs,null);assert.ok(events[0].totalMs>=0);assert.equal(fs.existsSync(session.stateFile),false);
});

after(()=>{assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});});
