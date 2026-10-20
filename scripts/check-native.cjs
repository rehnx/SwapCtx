'use strict';
// Deterministic offline fixtures only. Never discovers or executes a provider.
const {test,after}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {spawnSync}=require('node:child_process');
const {EventEmitter}=require('node:events');
const adapter=require('../integrations/native/native.cjs');
const script=path.resolve(__dirname,'../integrations/native/native.cjs');
const exe=path.resolve(process.argv[2]||'target/debug/swapctx.exe');
const root=fs.mkdtempSync(path.join(os.tmpdir(),"swapctx-native-test-ü-' space-"));
after(()=>fs.rmSync(root,{recursive:true,force:true}));
let sequence=0;
function fixture() {
  const project=path.join(root,'project '+(++sequence)),dataDir=path.join(root,'data '+sequence);fs.mkdirSync(project);
  const cli=(args,cwd=project)=>spawnSync(exe,['--data-dir',dataDir,...args],{cwd,encoding:'utf8',timeout:10000});
  const ok=args=>{const r=cli(args);assert.equal(r.status,0,r.stderr+r.stdout);return r;};
  ok(['init']);ok(['memory','add','Keep the public interface stable','--kind','requirement','--source','offline fixture']);
  ok(['protect','add','--path','protected/**','--reason','Retain reviewed files','--policy','ask']);
  const options={provider:'claude',swapctx:exe,dataDir,project,executable:'NEVER-EXECUTE-A-PROVIDER.exe'};
  const raw=()=>ok(['context','--max-bytes',String(adapter.PACKET_BYTES),'--limit','100']).stdout;
  const packet=()=>JSON.parse(raw());
  return {project,dataDir,cli,ok,options,raw,packet};
}
function wire(packet) {
  // bytes_used includes its own digit width.
  for(let i=0;i<5;i++) packet.budget.bytes_used=Buffer.byteLength(JSON.stringify(packet));
  return JSON.stringify(packet)+'\n';
}
function config(f) {
  const directory=fs.mkdtempSync(path.join(root,'hooks-'));
  const setup=adapter.configure(f.options,directory,script);
  return {directory,setup,file:path.join(directory,'adapter.json')};
}
function hook(f,c,event) {
  return spawnSync(process.execPath,[script,'hook',c.file],{cwd:f.project,input:Buffer.isBuffer(event)?event:JSON.stringify(event),encoding:'utf8',timeout:10000});
}

function npmFixture(f,provider,extraEnv={}) {
  const bin=path.join(root,'npm '+(++sequence)),pkg=path.join(bin,'node_modules',provider==='codex'?'@openai/codex':'@anthropic-ai/claude-code');
  fs.mkdirSync(pkg,{recursive:true});
  fs.writeFileSync(path.join(f.project,provider+'.cmd'),'@echo off\r\necho UNTRUSTED-CWD-SHIM\r\nexit /b 98\r\n');
  fs.writeFileSync(path.join(bin,provider+'.cmd'),'@echo off\r\necho SHIM-MUST-NOT-EXECUTE\r\nexit /b 99\r\n');
  fs.writeFileSync(path.join(pkg,'package.json'),JSON.stringify({name:provider==='codex'?'@openai/codex':'@anthropic-ai/claude-code',bin:{[provider]:'fixture.cjs'}}));
  fs.copyFileSync(path.join(__dirname,'../tests/fixtures/native-provider.cjs'),path.join(pkg,'fixture.cjs'));
  const report=path.join(bin,'synthetic-report.json');
  const env={...process.env,PATH:[f.project,bin,path.dirname(process.execPath)].join(path.delimiter),SWAPCTX_FAKE_PROVIDER:provider,
    SWAPCTX_FAKE_REPORT:report,SWAPCTX_FAKE_CLI:exe,SWAPCTX_FAKE_DATA:f.dataDir,...extraEnv};
  const run=(args=[],data=f.dataDir)=>spawnSync(exe,['--data-dir',data,'native',provider,...args],{cwd:f.project,env,encoding:'utf8',timeout:45000});
  return {bin,pkg,report,env,run,read:()=>JSON.parse(fs.readFileSync(report,'utf8'))};
}

test('actual Rust CLI and embedded adapter deliver all lifecycle events to both synthetic npm providers',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) {
    const f=fixture(),n=npmFixture(f,provider,{SWAPCTX_FAKE_MODE:'refresh'});
    const forwarded=provider==='codex'?['resume','synthetic-session','value with space & percent% literal']:['-c','value with space & percent% literal'];
    const r=n.run(['--context','--',...forwarded]);
    assert.equal(r.status,0,r.stderr);assert.match(r.stdout,/synthetic-native-interface/);
    assert.doesNotMatch(r.stdout,/SHIM-MUST/);
    const report=n.read();assert.deepEqual(report.probes,['--version','--help']);
    assert.deepEqual(report.arguments.slice(-forwarded.length),forwarded);
    assert.deepEqual(report.callbacks.map(c=>c.source),['startup','resume','compact','clear','fork']);
    for(const callback of report.callbacks) assert.ok(callback.context&&callback.expected_requirement&&callback.project_matches&&callback.protocol_valid&&callback.mandatory_complete,JSON.stringify(callback));
    assert.match(r.stderr,/5 contexts prepared, 5 output writes completed, 0 failure observations/);
    assert.match(r.stderr,/Provider receipt and model adherence remain unknown/);
    for(const dir of [report.hook_directory,report.script_directory]) assert.equal(fs.existsSync(dir),false);
  }
});

test('native health is read-only and never even probes either synthetic provider',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) {
    const f=fixture(),n=npmFixture(f,provider),before=fs.readFileSync(path.join(f.dataDir,'swapctx.db'));
    const r=n.run(['--check']);assert.equal(r.status,0,r.stderr);
    assert.match(r.stdout,/Hook configuration: generated locally/);assert.match(r.stdout,/Provider receipt: unknown/);
    assert.doesNotMatch(r.stdout+r.stderr,/Keep the public interface/);assert.equal(fs.existsSync(n.report),false);
    assert.deepEqual(fs.readFileSync(path.join(f.dataDir,'swapctx.db')),before);
    const missing=path.join(root,'no-health-store-'+(++sequence));
    const bad=n.run(['--check'],missing);assert.equal(bad.status,1);assert.match(bad.stderr,/context_store_unavailable/);
    assert.equal(fs.existsSync(missing),false);assert.equal(fs.existsSync(n.report),false);
    f.ok(['memory','add','Competing choice','--kind','requirement','--key','choice']);
    f.ok(['memory','add','Different choice','--kind','requirement','--key','choice']);
    const invalid=n.run(['--check']);assert.equal(invalid.status,1);assert.match(invalid.stderr,/mandatory_context_incomplete/);
    assert.equal(fs.existsSync(n.report),false);
    const launch=n.run(['--context']);assert.equal(launch.status,1);assert.match(launch.stderr,/mandatory_context_incomplete/);
    assert.equal(n.read().ui_started,undefined);
  }
});

test('npm declared native exe supports health, plain launch and the complete lifecycle chain without executing its shim',()=>{
  if(process.platform!=='win32') return;
  const f=fixture(),n=npmFixture(f,'claude');
  const native=path.join(n.pkg,'bin','claude.exe');
  fs.mkdirSync(path.dirname(native));
  const compiled=spawnSync('rustc',[path.join(__dirname,'../tests/fixtures/native-provider-exe.rs'),'-o',native],{encoding:'utf8',timeout:60000});
  assert.equal(compiled.status,0,compiled.error?.message||compiled.stderr);
  const manifest=path.join(n.pkg,'package.json');
  const metadata={name:'@anthropic-ai/claude-code',version:'2.1.304',bin:{claude:'bin/claude.exe'}};
  fs.writeFileSync(manifest,JSON.stringify(metadata));
  n.env.SWAPCTX_FAKE_NODE=process.execPath;
  n.env.SWAPCTX_FAKE_SCRIPT=path.join(n.pkg,'fixture.cjs');
  const before=fs.readFileSync(path.join(f.dataDir,'swapctx.db'));
  const health=n.run(['--check']);
  assert.equal(health.status,0,health.stderr);assert.match(health.stdout,/ready \(native executable\)/);
  assert.equal(fs.existsSync(n.report),false);
  assert.deepEqual(fs.readFileSync(path.join(f.dataDir,'swapctx.db')),before);
  // No Node lookup is needed for plain native entry points. The synthetic exe
  // alone knows the absolute fixture runtime; neither shim nor PATH supplies it.
  n.env.PATH=[f.project,n.bin].join(path.delimiter);
  const missing=path.join(root,'no-exe-store-'+(++sequence)),args=['--resume','space & | < > ^ % ! " literal'];
  n.env.SWAPCTX_FAKE_EXIT='27';
  const plain=n.run(['--',...args],missing);
  assert.equal(plain.status,27,plain.stderr);assert.deepEqual(n.read().arguments,args);
  assert.deepEqual(n.read().probes,[]);assert.equal(fs.existsSync(missing),false);
  delete n.env.SWAPCTX_FAKE_EXIT;
  fs.unlinkSync(n.report);
  n.env.SWAPCTX_FAKE_MODE='refresh';
  const context=n.run(['--context','--node',process.execPath,'--','-c','literal & value']);
  assert.equal(context.status,0,context.stderr);
  const report=n.read();assert.deepEqual(report.probes,['--version','--help']);
  assert.deepEqual(report.arguments.slice(-2),['-c','literal & value']);
  assert.deepEqual(report.callbacks.map(c=>c.source),['startup','resume','compact','clear','fork']);
  for(const c of report.callbacks) assert.ok(c.context&&c.expected_requirement&&c.project_matches&&c.protocol_valid&&c.mandatory_complete,JSON.stringify(c));
  assert.match(context.stderr,/5 contexts prepared, 5 output writes completed, 0 failure observations/);
  assert.match(context.stderr,/Provider receipt and model adherence remain unknown/);
  for(const dir of [report.hook_directory,report.script_directory]) assert.equal(fs.existsSync(dir),false);
  fs.unlinkSync(n.report);
  const outside=path.join(n.bin,'outside');fs.mkdirSync(outside);
  fs.copyFileSync(native,path.join(outside,'escape.exe'));
  const redirect=path.join(n.pkg,'redirect');fs.symlinkSync(outside,redirect,'junction');
  for(const bin of ['../outside/escape.exe','redirect/escape.exe','bin/claude.exe:stream','bin/missing.exe']) {
    fs.writeFileSync(manifest,JSON.stringify({...metadata,bin:{claude:bin}}));
    const bad=n.run(['--check','--node',process.execPath]);
    assert.equal(bad.status,1);assert.match(bad.stdout,/unsupported_provider_layout/);
    assert.equal(fs.existsSync(n.report),false);
  }
  fs.unlinkSync(redirect);
  const self=path.join(n.pkg,'bin','self.exe');fs.copyFileSync(exe,self);
  fs.writeFileSync(manifest,JSON.stringify({...metadata,bin:{claude:'bin/self.exe'}}));
  const recursive=spawnSync(self,['--data-dir',f.dataDir,'native','claude','--check','--node',process.execPath],
    {cwd:f.project,env:n.env,encoding:'utf8',timeout:10000});
  assert.equal(recursive.status,1);assert.match(recursive.stdout,/unsupported_provider_layout/);
  assert.equal(fs.existsSync(n.report),false);
});

test('full CLI handles both providers missing hooks, invalid events, wrong projects and provider exits without claiming receipt',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) for(const mode of ['no-hooks','bad-hook','outside','nested','error','interrupt']) {
    const f=fixture(),overrides={SWAPCTX_FAKE_MODE:mode};
    if(mode==='outside') overrides.SWAPCTX_FAKE_EVENT_CWD=root;
    if(mode==='nested') {const nested=path.join(f.project,'child');fs.mkdirSync(nested);assert.equal(f.cli(['init'],nested).status,0);overrides.SWAPCTX_FAKE_EVENT_CWD=nested;}
    if(mode==='error') overrides.SWAPCTX_FAKE_EXIT='29';
    const n=npmFixture(f,provider,overrides),r=n.run(['--context']);
    assert.equal(r.status,mode==='error'?29:mode==='interrupt'?1:0,`${provider}/${mode}: ${r.error?.code||''}\n${r.stderr}`);
    const report=n.read();
    if(['no-hooks','interrupt'].includes(mode)) assert.match(r.stderr,/no hook callback was recorded/);
    else if(mode==='error') assert.match(r.stderr,/5 contexts prepared/);
    else {assert.equal(report.callbacks.length,5);assert.ok(report.callbacks.every(c=>!c.context&&c.advisory));assert.match(r.stderr,/0 contexts prepared, 0 output writes completed, 5 failure observations/);}
    for(const dir of [report.hook_directory,report.script_directory]) assert.equal(fs.existsSync(dir),false);
  }
});

test('unsupported provider probes preserve shell-free npm arguments and do not create a store',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) {
    const f=fixture(),n=npmFixture(f,provider,{SWAPCTX_FAKE_UNSUPPORTED:'1',SWAPCTX_FAKE_EXIT:'27'}),missing=path.join(root,'absent-'+(++sequence));
    const r=n.run(['--context','--','--resume','a & b % literal'],missing);
    assert.equal(r.status,27,r.stderr);assert.match(r.stderr,/without SwapCtx context/);assert.equal(fs.existsSync(missing),false);
    assert.deepEqual(n.read().arguments,['--resume','a & b % literal']);
  }
});

test('full CLI distinguishes Node/config entry and receipt failure even when no callback receipt survives',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) for(const mode of ['config-invalid','receipt-lock']) {
    const f=fixture(),n=npmFixture(f,provider,{SWAPCTX_FAKE_MODE:mode});
    const r=n.run(['--context']);assert.equal(r.status,0,`${provider}/${mode}: ${r.stderr}`);
    assert.match(r.stderr,/Node entry=true/);
    assert.match(r.stderr,provider==='codex'?/Windows bootstrap=true/:/Windows bootstrap=false/);
    if(mode==='config-invalid') {
      assert.match(r.stderr,/config validated=false, receipt write failed=false/);
      assert.match(r.stderr,/Node entry observed but configuration validation not observed/);
      assert.ok(n.read().callbacks.every(c=>!c.context&&c.advisory));
    } else {
      assert.match(r.stderr,/config validated=true, receipt write failed=true/);
      assert.match(r.stderr,/logging failed or metadata was removed/);
      assert.ok(n.read().callbacks.every(c=>c.context));
    }
    assert.match(r.stderr,/Provider receipt and model adherence remain unknown/);
    assert.doesNotMatch(r.stderr,/Keep the public interface|synthetic-do-not-retain/);
    assert.equal(fs.existsSync(n.read().hook_directory),false);
  }
});

test('plain npm native launching forwards literal arguments without probes, hooks or memory creation',()=>{
  if(process.platform!=='win32') return;
  for(const provider of ['codex','claude']) {
    const f=fixture(),n=npmFixture(f,provider),missing=path.join(root,'no-plain-store-'+(++sequence));
    const args=['--resume','space & | < > ^ % ! " literal'];
    const r=n.run(['--',...args],missing);assert.equal(r.status,0,r.stderr);
    assert.deepEqual(n.read().arguments,args);assert.deepEqual(n.read().probes,[]);
    assert.equal(n.read().hook_directory,undefined);assert.equal(fs.existsSync(missing),false);
  }
});

test('npm metadata cannot escape scope or disguise arbitrary shell shims; health diagnoses missing runtimes',()=>{
  if(process.platform!=='win32') return;
  const f=fixture(),n=npmFixture(f,'codex');
  const manifest=path.join(n.pkg,'package.json'),original=fs.readFileSync(manifest,'utf8');
  const arbitrary=path.join(n.bin,'unrelated.cmd');fs.writeFileSync(arbitrary,'@exit /b 99');
  const unrelated=n.run(['--check','--executable',arbitrary]);assert.equal(unrelated.status,1);assert.match(unrelated.stdout,/unsupported_provider_layout/);
  for(const change of [
    {name:'unrelated',bin:{codex:'fixture.cjs'}},
    {name:'@openai/codex',bin:{codex:'../outside.cjs'}},
    {name:'@openai/codex',bin:{codex:'fixture:stream.cjs'}},
    {name:'@openai/codex',bin:{codex:'fixture.cjs & echo injected'}},
  ]) {
    fs.writeFileSync(manifest,JSON.stringify(change));
    const r=n.run(['--check']);assert.equal(r.status,1);assert.match(r.stdout,/unsupported_provider_layout/);
    assert.equal(fs.existsSync(n.report),false);
  }
  fs.writeFileSync(manifest,'{"name":"@openai/codex","name":"unrelated","bin":{"codex":"fixture.cjs"}}');
  assert.match(n.run(['--context']).stderr,/unsupported_provider_layout/);
  fs.writeFileSync(manifest,original);
  const outside=path.join(n.bin,'outside-package');fs.mkdirSync(outside);fs.copyFileSync(path.join(n.pkg,'fixture.cjs'),path.join(outside,'escape.cjs'));
  const redirected=path.join(n.pkg,'redirected');fs.symlinkSync(outside,redirected,'junction');
  fs.writeFileSync(manifest,JSON.stringify({name:'@openai/codex',bin:{codex:'redirected/escape.cjs'}}));
  assert.match(n.run(['--context']).stderr,/unsupported_provider_layout/);assert.equal(fs.existsSync(n.report),false);
  fs.unlinkSync(redirected);fs.writeFileSync(manifest,original);
  const missing=n.run(['--check','--node',path.join(root,'missing-node.exe')]);
  assert.equal(missing.status,1);assert.match(missing.stdout,/Node runtime: unavailable/);assert.match(missing.stdout,/Memory store: readable/);
  const self=n.run(['--check','--executable',exe]);assert.equal(self.status,1);assert.match(self.stdout,/provider_self_recursion/);
  const old=path.join(n.bin,'old-node.cjs');fs.writeFileSync(old,"Object.defineProperty(process.versions,'node',{value:'22.0.0'});");
  const result=spawnSync(exe,['--data-dir',f.dataDir,'native','codex','--check'],{
    cwd:f.project,env:{...n.env,NODE_OPTIONS:'--require='+JSON.stringify(old)},encoding:'utf8',timeout:10000});
  assert.equal(result.status,1);assert.match(result.stderr,/node_24_required/);assert.equal(fs.existsSync(n.report),false);
});
test('real read-only packet preserves requirements, provenance, scope and byte budget',()=>{
  const f=fixture(),before=fs.readFileSync(path.join(f.dataDir,'swapctx.db'));
  const result=adapter.retrieve(f.options,f.project);
  assert.match(result,/Keep the public interface stable/);assert.ok(Buffer.byteLength(result)<=8192);
  assert.deepEqual(fs.readFileSync(path.join(f.dataDir,'swapctx.db')),before);
});
test('representative native SessionStart payloads return context without retaining provider metadata',()=>{
  const f=fixture(),c=config(f);
  // Common fields from both official contracts; Claude resume metadata is
  // optional. No transcript is opened, and no provider/model is involved.
  for(const [source,extra] of [
    ['startup',{model:'synthetic-model',transcript_path:null}],
    ['resume',{seconds_since_last_response:5400,context_tokens:123,prompt_cache_likely_expired:true,estimated_cache_write_usd:0.01}],
    ['clear',{}],['compact',{model:'synthetic-model'}],['fork',{agent_type:'synthetic-agent',session_title:'private-fixture-title'}],
  ]) {
    const event={session_id:'private-fixture-session',transcript_path:path.join(root,'never-read.jsonl'),permission_mode:'default',
      hook_event_name:'SessionStart',source,cwd:f.project,...extra};
    const r=hook(f,c,event);
    assert.equal(r.status,0,r.stderr);const out=JSON.parse(r.stdout);
    assert.equal(out.hookSpecificOutput.hookEventName,'SessionStart');
    assert.match(out.hookSpecificOutput.additionalContext,/Keep the public interface stable/);
    assert.doesNotMatch(r.stdout,/private-fixture|never-read|synthetic-model|synthetic-agent|context_tokens/);
  }
  const entries=fs.readFileSync(c.setup.receipt,'utf8');assert.equal(entries.trim().split('\n').length,20);
  assert.doesNotMatch(entries,/Keep the|project|transcript|session_id/);
});

test('mandatory coverage matches the existing case-insensitive requirement kind contract',()=>{
  const f=fixture();
  f.ok(['memory','add','Retain mixed-case requirement','--kind','Requirement']);
  const text=adapter.retrieve(f.options,f.project);
  assert.match(text,/Retain mixed-case requirement/);
  assert.match(text,/Keep the public interface stable/);
});
test('nested directory works; separately registered nested project and sibling scopes are rejected',()=>{
  const f=fixture(),nested=path.join(f.project,'nested');fs.mkdirSync(nested);
  const event={hook_event_name:'SessionStart',source:'startup',cwd:nested};
  assert.ok(adapter.eventContext(f.options,event).hookSpecificOutput);
  const registered=f.cli(['init'],nested);assert.equal(registered.status,0,registered.stderr);
  assert.throws(()=>adapter.eventContext(f.options,event),/context_scope_mismatch/);
  event.cwd=root;assert.throws(()=>adapter.eventContext(f.options,event),/event_outside_project/);
});
test('invalid protocol, coverage, freshness, conflicts, provenance and sensitive fields fail closed',()=>{
  const f=fixture();
  const mutations=[
    p=>p.version=2,p=>p.ok=false,p=>p.policy.fresh_only=false,p=>p.policy.conflicts_excluded=false,
    p=>p.policy.unknown_provenance_excluded=false,p=>p.policy.secret_like_records_excluded=false,
    p=>p.coverage.mandatory_context_complete=false,p=>p.coverage.requirements_returned=0,
    p=>p.coverage.review_required_records=1,p=>p.scan.truncated=true,p=>p.generated_unix=1,
    p=>p.records[0].freshness='stale',p=>p.records[0].conflicted=true,
    p=>p.records[0].expires_unix=1,p=>p.records[0].origin.id='unknown',
    p=>p.records[0].provenance_status='unknown',p=>p.records[0].origin.authenticated=true,
    p=>p.records[0].content='password="NeverExposeThisValue"',
    p=>p.records[0].origin.source='sk-abcdefghijklmnopqrstuvwxyz',
    p=>p.additive={private:'-----BEGIN RSA PRIVATE KEY-----'},p=>p.additive={'sk-abcdefghijklmnopqrstuvwxyz':'value'},
    p=>p.records.push({...p.records[0]}),p=>p.project.root=root,p=>p.budget.max_records=101,
    p=>p.boundary_rules[0].enforced=true,p=>p.boundary_rules[0].pattern='',
    p=>p.boundary_rules[0].policy='approve',p=>p.boundary_rules=[],
    p=>p.boundary_rules.push({...p.boundary_rules[0]}),
  ];
  for(const mutate of mutations) {const p=f.packet();mutate(p);assert.throws(()=>adapter.validatePacket(wire(p),f.project));}
  assert.throws(()=>adapter.validatePacket('{',f.project),/context_json_invalid/);
  assert.throws(()=>adapter.validatePacket(' '.repeat(7169),f.project),/context_budget_exceeded/);
});
test('retrieval errors, timeout, invalid UTF-8 and excess output never become model context',()=>{
  const f=fixture();
  for(const result of [{status:1,stdout:Buffer.from(f.raw())},{status:0,error:{code:'ETIMEDOUT'}},
    {status:0,stdout:Buffer.from([0xff])},{status:0,stdout:Buffer.alloc(8000,32)}]) {
    assert.throws(()=>adapter.retrieve(f.options,f.project,()=>result));
    const stages=[];
    assert.throws(()=>adapter.eventContext(f.options,{hook_event_name:'SessionStart',source:'resume',cwd:f.project},()=>result,stage=>stages.push(stage)));
    assert.deepEqual(stages,['retrieval_attempted'],'failed retrieval must never report preparation');
  }
  assert.throws(()=>adapter.retrieve(f.options,f.project,()=>({status:1,stdout:Buffer.from(JSON.stringify({protocol:'swapctx.context',version:1,ok:false,error:{code:'migration_required'}}))})),/context_migration_required/);
});

test('ambiguous JSON keys cannot hide rejected context behind the value seen by JSON.parse',()=>{
  const f=fixture();
  for(const member of ['"content":"password=PrivateFixtureValue",', '"\\u0063ontent":"old conflicting requirement",']) {
    const p=f.packet();let raw;
    for(let i=0;i<5;i++) {
      raw=JSON.stringify(p).replace('"content":',member+'"content":');
      p.budget.bytes_used=Buffer.byteLength(raw);
    }
    assert.throws(()=>adapter.validatePacket(raw,f.project),/context_duplicate_key/);
  }
});

test('a full-budget context packet permits only bounded transport line endings',()=>{
  const f=fixture(),p=f.packet();p.additive='';
  p.additive='x'.repeat(adapter.PACKET_BYTES-Buffer.byteLength(wire(p).trimEnd()));
  const raw=wire(p);assert.equal(Buffer.byteLength(raw.trimEnd()),adapter.PACKET_BYTES);
  for(const ending of ['','\n','\r\n']) {
    const result=adapter.retrieve(f.options,f.project,()=>({status:0,stdout:Buffer.from(raw.trimEnd()+ending)}));
    assert.ok(Buffer.byteLength(result)<=8192);
  }
  assert.throws(()=>adapter.validatePacket(raw+'\n\n',f.project),/context_budget_exceeded/);
});

test('broken hook stdout reports failure without claiming output was emitted',async()=>{
  const {Readable,Writable}=require('node:stream');
  const f=fixture(),c=config(f);
  const input=Readable.from([Buffer.from(JSON.stringify({hook_event_name:'SessionStart',source:'startup',cwd:f.project}))]);
  const output=new Writable({write(_chunk,_encoding,callback){callback(Object.assign(Error('private pipe error'),{code:'EPIPE'}));}});
  await assert.rejects(adapter.hook(c.file,input,output),/hook_output_failed/);
  const receipts=fs.readFileSync(c.setup.receipt,'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(receipts.map(e=>e.stage),['callback_requested','retrieval_attempted','context_prepared','output_failed']);
  assert.equal(receipts.at(-1).code,'hook_output_failed');
  assert.equal(adapter.summarizeReceipts(c.setup.receipt).written,0);
});

test('Claude short continue flag passes unchanged; Codex config and both workspace overrides remain rejected',async()=>{
  const f=fixture(),base={probe:()=>true,retrieve:()=>'',tempRoot:root,warn:()=>{}};
  assert.equal(await adapter.launch(f.options,['-c'],{...base,runProvider:async(_exe,args)=>{
    assert.deepEqual(args.slice(2),['-c']);return 0;
  }}),0);
  for(const provider of ['codex','claude']) for(const arg of ['-C','--cwd=elsewhere','--worktree','-w']) {
    await assert.rejects(adapter.launch({...f.options,provider},[arg],base),/context_launch_argument_conflict/);
  }
  await assert.rejects(adapter.launch({...f.options,provider:'codex'},['-c','hooks.SessionStart=[]'],base),/context_launch_argument_conflict/);
});
test('malformed, oversized, unsupported events and missing config emit advisory failure without context',()=>{
  const f=fixture(),c=config(f);
  for(const event of [Buffer.from([0xff]),Buffer.from('{'),Buffer.alloc(32769,32),
    {hook_event_name:'PreToolUse',source:'startup',cwd:f.project},
    {hook_event_name:'SessionStart',source:'new-unknown',cwd:f.project}]) {
    const r=hook(f,c,event),out=JSON.parse(r.stdout);assert.equal(r.status,0,r.stderr);
    assert.equal(out.hookSpecificOutput,undefined);assert.match(out.systemMessage,/no memory was supplied/);
  }
  fs.unlinkSync(c.file);const r=hook(f,c,{});assert.equal(JSON.parse(r.stdout).hookSpecificOutput,undefined);
});
test('post-compaction callback rereads current requirements instead of replaying a launch snapshot',()=>{
  const f=fixture(),c=config(f);
  const first=hook(f,c,{hook_event_name:'SessionStart',source:'startup',cwd:f.project});
  assert.match(first.stdout,/Keep the public interface stable/);
  f.ok(['memory','supersede',f.packet().records[0].id,'Use the updated release contract']);
  const later=hook(f,c,{hook_event_name:'SessionStart',source:'compact',cwd:f.project});
  assert.match(later.stdout,/Use the updated release contract/);
  assert.doesNotMatch(later.stdout,/Keep the public interface stable/);
});
test('real store sensitive/conflicting requirements prevent automatic handoff',()=>{
  const f=fixture();
  f.ok(['memory','add','password=PrivateFixtureValue','--kind','requirement','--allow-sensitive']);
  assert.throws(()=>adapter.retrieve(f.options,f.project),/mandatory_context_incomplete/);
  const other=fixture();
  other.ok(['memory','add','Old choice','--kind','requirement','--key','choice']);
  other.ok(['memory','add','New choice','--kind','requirement','--key','choice']);
  assert.throws(()=>adapter.retrieve(other.options,other.project),/mandatory_context_incomplete/);
});
test('version/capability probes use only bounded --version and --help and require advertised interfaces',()=>{
  const calls=[];
  const run=(_exe,args,opts)=>{calls.push(args);assert.equal(opts.shell,false);assert.equal(opts.timeout,5000);return {status:0,stdout:args[0]==='--version'?'codex-cli 0.162.1\n':'--config'};};
  assert.equal(adapter.probe('codex','FAKE',run),false);assert.deepEqual(calls,[['--version'],['--help']]);
  assert.equal(adapter.probe('codex','FAKE',(_exe,args)=>({status:0,stdout:args[0]==='--version'?'codex-cli 9.0.0':'--config --dangerously-bypass-hook-trust'})),true);
  assert.equal(adapter.probe('claude','FAKE',(_exe,args)=>({status:0,stdout:args[0]==='--version'?'2.1.296 (Claude Code)':'--plugin-dir'})),true);
  assert.equal(adapter.probe('claude','FAKE',(_exe,args)=>({status:0,stdout:args[0]==='--version'?'2.1.295 (Claude Code)':'--plugin-dir'})),false);
  assert.equal(adapter.probe('claude','FAKE',()=>({status:0,stdout:'1.0.0 (Claude Code)'})),false);
  assert.equal(adapter.probe('claude','FAKE',()=>({error:{code:'ENOENT'}})),false);
});
test('unsupported hooks preserve ordinary provider arguments without touching memory',async()=>{
  const f=fixture(),warnings=[],args=['--resume','fixture'];
  const result=await adapter.launch(f.options,args,{probe:()=>false,warn:s=>warnings.push(s),retrieve:()=>assert.fail('must not retrieve'),runProvider:async(executable,actual)=>{assert.equal(executable,f.options.executable);assert.deepEqual(actual,args);return 7;}});
  assert.equal(result,7);assert.match(warnings.join(),/without SwapCtx context/);
});
test('session-only plugin and Codex config preserve user flags, permissions, auth and config files',()=>{
  const f=fixture(),sentinels=['AGENTS.md','CLAUDE.md','.codex/config.toml','.claude/settings.json'];
  for(const name of sentinels) {
    const file=path.join(f.project,name);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,'preserve '+name);
  }
  const claude=config(f);
  assert.deepEqual(claude.setup.args,['--plugin-dir',claude.directory]);
  const hooks=JSON.parse(fs.readFileSync(path.join(claude.directory,'hooks','hooks.json')));
  assert.equal(hooks.hooks.SessionStart[0].hooks[0].type,'command');
  assert.equal(hooks.hooks.SessionStart[0].hooks[0].command,process.execPath);
  assert.equal(hooks.hooks.SessionStart[0].hooks[0].timeout,10);
  assert.deepEqual(hooks.hooks.SessionStart[0].hooks[0].args,[script,'hook',path.join(claude.directory,'adapter.json')]);
  f.options.provider='codex';const codex=config(f);
  assert.equal(codex.setup.args[0],'-c');assert.match(codex.setup.args[1],/^hooks.SessionStart=\[/);
  assert.doesNotMatch(codex.setup.args.join(' '),/dangerously|approval_policy|sandbox_mode|CODEX_HOME/);
  for(const name of sentinels) assert.equal(fs.readFileSync(path.join(f.project,name),'utf8'),'preserve '+name);
  assert.deepEqual(fs.readdirSync(f.project).sort(),['.claude','.codex','AGENTS.md','CLAUDE.md']);
});
test('Windows encoded command executes actual adapter with spaces and apostrophes and synthetic event',()=>{
  if(process.platform!=='win32') return;
  const f=fixture();f.options.provider='codex';
  const c=config(f),commands={commandWindows:JSON.parse(/command_windows=("(?:\\.|[^"\\])*")/.exec(c.setup.args[1])[1])};
  assert.ok(c.setup.args[1].includes('additionalContextLimit=0'));
  const encoded=commands.commandWindows.split(' ').at(-1);
  const shell=/^([A-Za-z]:\\[A-Za-z0-9_.\\-]+) -NoProfile /.exec(commands.commandWindows);
  assert.ok(shell,'Windows hook must use a shell-neutral absolute system PowerShell path');
  assert.equal(shell[1].toLowerCase(),path.join(process.env.SystemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe').toLowerCase());
  // Poison both implicit search locations; only the selected absolute helper may run.
  fs.writeFileSync(path.join(f.project,'powershell.exe'),'not a Windows executable');
  const r=spawnSync(shell[1],['-NoProfile','-NonInteractive','-EncodedCommand',encoded],{
    env:{...process.env,...c.setup.env,PATH:f.project},
    cwd:f.project,input:JSON.stringify({session_id:'synthetic-codex-session',transcript_path:null,model:'synthetic-model',permission_mode:'default',
      hook_event_name:'SessionStart',source:'compact',cwd:f.project}),encoding:'utf8',timeout:15000});
  assert.equal(r.status,0,r.stderr);assert.match(JSON.parse(r.stdout).hookSpecificOutput.additionalContext,/Keep the public interface stable/);
});
test('unavailable Windows hook interpreter fails closed and cleans up without affecting Claude exec hooks',async()=>{
  if(process.platform!=='win32') return;
  const f=fixture(),saved=process.env.SystemRoot;
  const before=fs.readdirSync(root).sort();
  try {
    process.env.SystemRoot=f.project; // No system PowerShell exists in this fixture.
    await assert.rejects(adapter.launch({...f.options,provider:'codex'},[],{
      probe:()=>true,retrieve:()=>'',tempRoot:root,warn:()=>{},
      runProvider:()=>assert.fail('must not launch after failed hook setup'),
    }),{code:'windows_hook_interpreter_unavailable'});
    assert.deepEqual(fs.readdirSync(root).sort(),before);
    assert.equal(config(f).setup.args[0],'--plugin-dir');
  } finally {
    if(saved===undefined) delete process.env.SystemRoot; else process.env.SystemRoot=saved;
  }
  // Do not remove the outer quotes by accepting shell-special interpreter paths.
  const redirected=path.join(root,'Windows & space');fs.symlinkSync(saved,redirected,'junction');
  try {
    process.env.SystemRoot=redirected;
    assert.throws(()=>adapter.hookCommands(script,path.join(root,'adapter.json')),{code:'windows_hook_interpreter_path_unsupported'});
  } finally {process.env.SystemRoot=saved;fs.unlinkSync(redirected);}
});

test('generated Codex TOML hook survives real Windows outer shells before reaching the adapter',()=>{
  if(process.platform!=='win32') return;
  const runner=path.join(root,'windows-hook-runner.exe');
  const built=spawnSync('rustc',[path.join(__dirname,'../tests/fixtures/windows-hook-runner.rs'),'-o',runner],{encoding:'utf8',timeout:60000});
  assert.equal(built.status,0,built.error?.message||built.stderr);
  const f=fixture();f.options.provider='codex';
  const note='Preserve café, 日本語, emoji 😀 and literal $ & % ! characters';
  f.ok(['memory','add',note,'--source','offline fixture']);
  const scriptDirectory=path.join(root,"adapter ' & $ ; ( ) % ! ^ ü");fs.mkdirSync(scriptDirectory);
  const copiedScript=path.join(scriptDirectory,'native.cjs');fs.copyFileSync(script,copiedScript);
  fs.writeFileSync(path.join(f.project,'powershell.exe'),'poisoned implicit search');
  fs.writeFileSync(path.join(f.project,'node.exe'),'poisoned implicit search');
  const failures=[];
  for(const mode of ['cmd-raw','cmd-escaped','powershell']) {
    const directory=fs.mkdtempSync(path.join(root,'shell-hooks-'));
    const c={setup:adapter.configure(f.options,directory,copiedScript)};
    const configFile=path.join(directory,'adapter.json'),startupFile=path.join(directory,'startup.bin');
    const resetStartup=()=>fs.writeFileSync(startupFile,'SCX10000');
    // Extract exactly the string written in the invocation's TOML, rather than
    // split it into executable/args (which bypasses the actual failing layer).
    const command=JSON.parse(/command_windows=("(?:\\.|[^"\\])*")/.exec(c.setup.args[1])[1]);
    assert.ok(command.length<=8000,'encoded command must fit cmd dispatch');
    const shell=path.join(process.env.SystemRoot,'System32',mode==='powershell'?'WindowsPowerShell/v1.0/powershell.exe':'cmd.exe');
    const run=(line,input=JSON.stringify({hook_event_name:'SessionStart',source:'startup',cwd:f.project}))=>
      spawnSync(runner,[mode,shell,line],{cwd:f.project,env:{...process.env,...c.setup.env,PATH:f.project},encoding:'utf8',timeout:15000,input});
    if(mode!=='cmd-raw') {
      const old=run(command.replace(/^(\S+)/,'"$1"'));
      assert.equal(old.status,1);assert.equal(fs.existsSync(c.setup.receipt),false);
      assert.equal(old.stdout,'');assert.match(old.stderr,mode==='powershell'?/UnexpectedToken/:/not recognized/);
      assert.deepEqual(adapter.startupState(configFile),{bootstrap:false,node:false,config:false,receiptFailed:false});
    }
    const r=run(command);
    if(r.status!==0) {failures.push({mode,status:r.status,receipt:fs.existsSync(c.setup.receipt),stderr:r.stderr});continue;}
    const entries=fs.readFileSync(c.setup.receipt,'utf8').trim().split('\n').map(JSON.parse);
    assert.deepEqual(adapter.startupState(configFile),{bootstrap:true,node:true,config:true,receiptFailed:false});
    assert.deepEqual(entries.map(e=>e.stage),['callback_requested','retrieval_attempted','context_prepared','output_written']);
    const text=JSON.parse(r.stdout).hookSpecificOutput.additionalContext;
    assert.ok(Buffer.byteLength(text)<=8192);
    const packet=JSON.parse(text.slice(text.indexOf('\n')+1));
    assert.equal(text,adapter.validatePacket(JSON.stringify(packet)+'\n',f.project));
    assert.equal(packet.records[0].content,'Keep the public interface stable');
    assert.equal(packet.records.find(r=>r.kind==='note').content,note);
    // Before-config failures have no receipt, and must not masquerade as an
    // input/validation failure. Only safe stage codes are emitted by SwapCtx.
    fs.unlinkSync(c.setup.receipt);
    resetStartup();
    const missingAdapter=adapter.hookCommands(path.join(root,'missing-adapter.cjs'),path.join(directory,'adapter.json')).commandWindows;
    const unavailable=run(missingAdapter);
    assert.equal(unavailable.status,1);assert.match(unavailable.stderr,/hook_adapter_unavailable/);
    assert.equal(unavailable.stdout,'');assert.equal(fs.existsSync(c.setup.receipt),false);
    assert.deepEqual(adapter.startupState(configFile),{bootstrap:true,node:false,config:false,receiptFailed:false});
    resetStartup();
    const encoded=command.split(' ').at(-1);
    const ps=Buffer.from(encoded,'base64').toString('utf16le');
    const missingNode=ps.replace("'"+process.execPath.replace(/'/g,"''")+"'", "'"+path.join(root,'missing-node.exe').replace(/'/g,"''")+"'");
    const startup=run(command.slice(0,-encoded.length)+Buffer.from(missingNode,'utf16le').toString('base64'));
    assert.equal(startup.status,1);assert.match(startup.stderr,/hook_node_start_failed/);
    assert.equal(startup.stdout,'');assert.equal(fs.existsSync(c.setup.receipt),false);
    assert.deepEqual(adapter.startupState(configFile),{bootstrap:true,node:false,config:false,receiptFailed:false});
    resetStartup();
    const savedConfig=fs.readFileSync(configFile);
    fs.writeFileSync(configFile,'{private-invalid-config');
    const invalid=run(command);
    assert.equal(invalid.status,0);assert.match(invalid.stderr,/hook_config_invalid/);
    assert.equal(JSON.parse(invalid.stdout).hookSpecificOutput,undefined);
    assert.doesNotMatch(invalid.stdout+invalid.stderr,/private-invalid-config/);
    assert.deepEqual(adapter.startupState(configFile),{bootstrap:true,node:true,config:false,receiptFailed:false});
    assert.equal(fs.existsSync(c.setup.receipt),false);fs.writeFileSync(configFile,savedConfig);
    const inputFailure=run(command,'{invalid');
    assert.equal(inputFailure.status,0);assert.match(inputFailure.stderr,/hook_input_invalid/);
    assert.equal(adapter.summarizeReceipts(c.setup.receipt).prepared,0);
    fs.unlinkSync(c.setup.receipt);fs.writeFileSync(c.setup.receipt+'.lock','');
    const logFailure=run(command);
    assert.equal(logFailure.status,0);assert.match(logFailure.stderr,/continuity metadata unavailable/);
    assert.ok(JSON.parse(logFailure.stdout).hookSpecificOutput.additionalContext);
    assert.equal(fs.existsSync(c.setup.receipt),false);assert.ok(fs.existsSync(c.setup.receipt+'.incomplete'));
    assert.deepEqual(adapter.startupState(configFile),{bootstrap:true,node:true,config:true,receiptFailed:true});
    fs.unlinkSync(c.setup.receipt+'.lock');
  }
  assert.deepEqual(failures,[]);
});

test('startup observations stay bounded, reject redirected/corrupt metadata and never gate valid context',()=>{
  const f=fixture(),c=config(f),file=path.join(c.directory,'startup.bin');
  const event={hook_event_name:'SessionStart',source:'startup',cwd:f.project};
  assert.equal(fs.statSync(file).size,8);
  const corrupt='private marker text must not escape';fs.writeFileSync(file,corrupt);
  let r=hook(f,c,event);assert.equal(r.status,0,r.stderr);
  assert.ok(JSON.parse(r.stdout).hookSpecificOutput.additionalContext);
  assert.match(r.stderr,/hook_startup_metadata_unavailable/);assert.doesNotMatch(r.stderr,/private marker/);
  assert.equal(fs.readFileSync(file,'utf8'),corrupt);
  fs.unlinkSync(file);
  r=hook(f,c,event);assert.equal(r.status,0,r.stderr);assert.ok(JSON.parse(r.stdout).hookSpecificOutput.additionalContext);
  assert.equal(fs.existsSync(file),false,'legacy hook must not create a diagnostic target');
  const outside=path.join(root,'outside-startup');fs.mkdirSync(outside);
  const target=path.join(outside,'startup.bin');fs.writeFileSync(target,'SCX10000');
  if(process.platform==='win32') {
    fs.symlinkSync(outside,file,'junction');r=hook(f,c,event);
    assert.equal(r.status,0);assert.equal(fs.readFileSync(target,'utf8'),'SCX10000');fs.unlinkSync(file);
  } else {
    fs.symlinkSync(target,file);r=hook(f,c,event);
    assert.equal(r.status,0);assert.equal(fs.readFileSync(target,'utf8'),'SCX10000');fs.unlinkSync(file);
  }
});

test('Codex definitions stay stable for the same scope while child paths remain launch-local',async()=>{
  const f=fixture();f.options.provider='codex';
  const first=config(f),next=config(f);
  assert.equal(first.setup.args[1],next.setup.args[1],'temporary paths must not change the reviewed definition');
  assert.notDeepEqual(first.setup.env,next.setup.env,'callback state must remain launch-local');
  const other=fixture();other.options.provider='codex';
  assert.notEqual(first.setup.args[1],config(other).setup.args[1],'another project/store requires a different definition');
  assert.match(first.setup.args[1],/^hooks.SessionStart=\[\{matcher="\^\(startup\|resume\|clear\|compact\|fork\)\$",hooks=\[/);
  assert.match(first.setup.args[1],/additionalContextLimit=0/);
  assert.doesNotMatch(first.setup.args[1],/bypass|features\.hooks/);
  const warnings=[];
  await adapter.launch(f.options,[],{probe:()=>true,retrieve:()=>'',tempRoot:root,warn:s=>warnings.push(s),runProvider:async()=>0});
  assert.match(warnings.join(),/definition is stable/);
  assert.match(warnings.join(),/Windows bootstrap=false, Node entry=false, config validated=false/);
  assert.match(warnings.join(),/Do not submit a model prompt/);
});

test('stable Codex callback pins adapter bytes, scope and launch-local environment without persisting hooks',async()=>{
  if(process.platform!=='win32') return;
  const f=fixture();f.options.provider='codex';
  const copy=path.join(root,"stable adapter ' & ü.cjs");fs.copyFileSync(script,copy);
  const make=(options=f.options)=>{
    const directory=fs.mkdtempSync(path.join(root,'stable-session-'));
    return {directory,setup:adapter.configure(options,directory,copy)};
  };
  const a=make(),b=make(),other=fixture();other.options.provider='codex';const c=make(other.options);
  const windows=setup=>JSON.parse(/command_windows=("(?:\\.|[^"\\])*")/.exec(setup.args[1])[1]);
  const shell=path.join(process.env.SystemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe');
  const run=(session,env=session.setup.env,cwd=f.project)=>spawnSync(shell,['-NoProfile','-NonInteractive','-EncodedCommand',windows(session.setup).split(' ').at(-1)],{
    cwd,env:{...Object.fromEntries(Object.entries(process.env).filter(([key])=>!/^SWAPCTX_NATIVE_(SCRIPT|CONFIG)$/i.test(key))),...env},
    input:JSON.stringify({hook_event_name:'SessionStart',source:'resume',cwd}),encoding:'utf8',timeout:15000});
  assert.equal(a.setup.args[1],b.setup.args[1]);
  const first=run(a);assert.equal(first.status,0,first.stderr);assert.ok(JSON.parse(first.stdout).hookSpecificOutput.additionalContext);
  // Equivalent definition can read only the session explicitly supplied in its
  // child environment. Receipts are not shared between simultaneous launches.
  const second=run(a,b.setup.env);assert.equal(second.status,0,second.stderr);
  assert.equal(adapter.summarizeReceipts(a.setup.receipt).written,1);
  assert.equal(adapter.summarizeReceipts(b.setup.receipt).written,1);
  const mismatch=run(a,c.setup.env,other.project);
  assert.equal(mismatch.status,0);assert.match(mismatch.stderr,/hook_session_scope_mismatch/);
  assert.equal(JSON.parse(mismatch.stdout).hookSpecificOutput,undefined);
  const missing=run(a,{});assert.equal(missing.status,1);assert.match(missing.stderr,/hook_session_environment_invalid/);assert.equal(missing.stdout,'');
  fs.appendFileSync(copy,"\nprocess.stdout.write('MUST-NOT-EXECUTE');\n");
  const changed=make();assert.notEqual(a.setup.args[1],changed.setup.args[1],'adapter changes must change reviewed command');
  const tampered=run(a);assert.equal(tampered.status,1);assert.match(tampered.stderr,/hook_adapter_integrity_failed/);
  assert.equal(tampered.stdout,'');assert.doesNotMatch(tampered.stderr,/MUST-NOT-EXECUTE/);
  fs.copyFileSync(script,copy);
  const saved=fs.readFileSync(path.join(a.directory,'adapter.json'));
  const configValue=JSON.parse(saved);configValue.dataDir=other.dataDir;
  fs.writeFileSync(path.join(a.directory,'adapter.json'),JSON.stringify(configValue));
  const storeChanged=run(a);assert.equal(storeChanged.status,0);assert.match(storeChanged.stderr,/hook_session_scope_mismatch/);
  assert.equal(JSON.parse(storeChanged.stdout).hookSpecificOutput,undefined);
  fs.writeFileSync(path.join(a.directory,'adapter.json'),saved);
  const anotherStore=make({...f.options,dataDir:other.dataDir});
  assert.notEqual(a.setup.args[1],anotherStore.setup.args[1]);
  const anotherExecutable=make({...f.options,swapctx:process.execPath});
  assert.notEqual(a.setup.args[1],anotherExecutable.setup.args[1]);
  fs.rmSync(a.directory,{recursive:true}); // Fixture-owned; simulate normal cleanup.
  const expired=run(a);assert.equal(expired.status,0);assert.match(expired.stderr,/hook_config_invalid/);
  assert.equal(JSON.parse(expired.stdout).hookSpecificOutput,undefined);
});

test('native child environment overrides stale aliases without changing parent state or other provider options',async()=>{
  const f=fixture();f.options.provider='codex';
  const saved=process.env.SWAPCTX_NATIVE_CONFIG;
  process.env.SWAPCTX_NATIVE_CONFIG='stale-parent-value';
  try {
    await adapter.launch(f.options,['resume','fixture'],{probe:()=>true,retrieve:()=>'',tempRoot:root,warn:()=>{},
      runProvider:async(_exe,args,options)=>{
        assert.deepEqual(args.slice(-2),['resume','fixture']);
        assert.notEqual(options.env.SWAPCTX_NATIVE_CONFIG,'stale-parent-value');
        assert.equal(fs.existsSync(options.env.SWAPCTX_NATIVE_CONFIG),true);
        assert.equal(Object.keys(options.env).filter(k=>k.toLowerCase()==='swapctx_native_config').length,1);
        assert.equal(process.env.SWAPCTX_NATIVE_CONFIG,'stale-parent-value');return 0;
      }});
    assert.equal(process.env.SWAPCTX_NATIVE_CONFIG,'stale-parent-value');
  } finally {if(saved===undefined)delete process.env.SWAPCTX_NATIVE_CONFIG;else process.env.SWAPCTX_NATIVE_CONFIG=saved;}
});

test('missing callbacks, provider exit errors and interrupted launches are diagnosed and cleaned',async()=>{
  const f=fixture(),warnings=[];
  const base={probe:()=>true,retrieve:()=>'',tempRoot:root,warn:s=>warnings.push(s)};
  const before=fs.readdirSync(root).sort();
  assert.equal(await adapter.launch(f.options,['--resume','id'],{...base,runProvider:async()=>13}),13);
  assert.match(warnings.join(),/no hook callback/);assert.match(warnings.join(),/shell launch, Node startup, adapter\/config loading or receipt writing/);
  assert.match(warnings.join(),/does not test provider shell dispatch/);assert.deepEqual(fs.readdirSync(root).sort(),before);
  assert.match(warnings.join(),/Windows bootstrap=false, Node entry=false, config validated=false, receipt write failed=false/);
  assert.match(warnings.join(),/cannot distinguish skipped loading\/trust\/policy/);
  await assert.rejects(()=>adapter.launch(f.options,[],{...base,runProvider:async()=>{throw Error('interrupted fixture');}}),/interrupted fixture/);
  assert.deepEqual(fs.readdirSync(root).sort(),before);
  await assert.rejects(()=>adapter.launch(f.options,[],{...base,retrieve:()=>{throw Error('invalid context');}}),/invalid context/);
  assert.deepEqual(fs.readdirSync(root).sort(),before);
  await assert.rejects(()=>adapter.launch(f.options,['--cwd=elsewhere'],base),/context_launch_argument_conflict/);
});
test('redirected cleanup preserves outside files and the originating provider error',async()=>{
  const f=fixture(),warnings=[],outside=path.join(root,'outside-cleanup');fs.mkdirSync(outside);
  fs.writeFileSync(path.join(outside,'sentinel'),'preserve');let directory;
  const pending=adapter.launch(f.options,[],{probe:()=>true,retrieve:()=>'',tempRoot:root,warn:s=>warnings.push(s),runProvider:async(_exe,args)=>{
    directory=args[1];fs.rmSync(directory,{recursive:true});fs.symlinkSync(outside,directory,process.platform==='win32'?'junction':'dir');
    throw Error('originating provider failure');
  }});
  await assert.rejects(()=>pending,/originating provider failure/);
  assert.match(warnings.join(),/cleanup failed/);assert.equal(fs.readFileSync(path.join(outside,'sentinel'),'utf8'),'preserve');
  fs.unlinkSync(directory);
});
test('invalid hook receipts do not hide provider exit failures or echo their contents',async()=>{
  const f=fixture(),warnings=[];
  const code=await adapter.launch(f.options,[],{probe:()=>true,retrieve:()=>'',tempRoot:root,warn:s=>warnings.push(s),runProvider:async(_exe,args)=>{
    fs.writeFileSync(path.join(args[1],'receipt.jsonl'),'private payload must not be echoed');return 29;
  }});
  assert.equal(code,29);assert.match(warnings.join(),/metadata is invalid/);
  assert.doesNotMatch(warnings.join(),/private payload/);
});
test('fake provider executable inherits terminal streams and preserves its exit status',async()=>{
  const file=path.join(root,'fake-provider.cjs');fs.writeFileSync(file,'process.exit(Number(process.argv[2]));');
  assert.equal(await adapter.runProvider(process.execPath,[file,'19']),19);
  await assert.rejects(()=>adapter.runProvider(path.join(root,'missing.exe'),[]),/native_provider_launch_failed/);
});
test('interrupt handler forwards signal and removes listeners after child closes',async()=>{
  const counts=['SIGINT','SIGTERM'].map(s=>process.listenerCount(s));let child;
  const pending=adapter.runProvider('FAKE',[],{spawn:()=>{child=new EventEmitter();child.kill=signal=>{assert.equal(signal,'SIGINT');queueMicrotask(()=>child.emit('close',null,'SIGINT'));};return child;}});
  process.emit('SIGINT');assert.equal(await pending,1);
  assert.deepEqual(['SIGINT','SIGTERM'].map(s=>process.listenerCount(s)),counts);
});
test('Rust native command forwards to a fake executable; bare nonterminal use never starts a provider',()=>{
  const f=fixture(),file=path.join(root,'fake-cli.cjs');fs.writeFileSync(file,'process.exit(17);');
  const r=f.cli(['native','claude','--executable',process.execPath,'--',file]);assert.equal(r.status,17,r.stderr);
  const bare=f.cli([]);assert.equal(bare.status,1);assert.match(bare.stderr,/Provider selection needs an interactive terminal/);
});
test('Rust context launch falls back for unsupported fake provider even without a database',()=>{
  const directory=path.join(root,'unregistered');fs.mkdirSync(directory);
  const file=path.join(root,'fake-unsupported.cjs');fs.writeFileSync(file,'process.exit(23);');
  const r=spawnSync(exe,['--data-dir',path.join(root,'missing-data'),'native','claude','--context',
    '--node',process.execPath,'--executable',process.execPath,'--',file],{cwd:directory,encoding:'utf8',timeout:10000});
  assert.equal(r.status,23,r.stderr);assert.match(r.stderr,/without SwapCtx context/);
  assert.equal(fs.existsSync(path.join(root,'missing-data')),false);
});
test('native executable discovery ignores current-directory and relative PATH entries',()=>{
  const project=path.join(root,'untrusted-project'),bin=path.join(root,'trusted-bin');
  fs.mkdirSync(project);fs.mkdirSync(bin);
  fs.writeFileSync(path.join(project,'codex.exe'),'This project file must never execute');
  fs.copyFileSync(exe,path.join(bin,'codex.exe'));
  const env={...process.env,PATH:['.',project,bin].join(path.delimiter)};
  const r=spawnSync(exe,['native','codex','--','--version'],{cwd:project,env,encoding:'utf8',timeout:10000});
  assert.equal(r.status,0,r.stderr);assert.match(r.stdout,/swapctx 0\.1\.0/);
  assert.equal(fs.readFileSync(path.join(project,'codex.exe'),'utf8'),'This project file must never execute');
});
test('input interruption or a missing end event is bounded and emits no context',async()=>{
  const {PassThrough}=require('node:stream');
  const f=fixture(),c=config(f),input=new PassThrough(),output=new PassThrough();let text='';
  output.on('data',chunk=>text+=chunk);
  await adapter.hook(c.file,input,output);
  assert.match(JSON.parse(text).systemMessage,/hook_input_timeout/);
  assert.equal(JSON.parse(text).hookSpecificOutput,undefined);
});
test('fake native UI exercises generated Claude hook, observes outputs and exits without retained plugin files',async()=>{
  const f=fixture(),warnings=[],before=fs.readdirSync(root).sort();
  const file=path.join(root,'fake-native-ui.cjs');
  fs.writeFileSync(file,`const fs=require('node:fs');const path=require('node:path');const {spawnSync}=require('node:child_process');
const dir=process.argv[process.argv.indexOf('--plugin-dir')+1];
const definition=JSON.parse(fs.readFileSync(path.join(dir,'hooks','hooks.json'))).hooks.SessionStart[0].hooks[0];
const r=spawnSync(definition.command,definition.args,{shell:false,input:JSON.stringify({hook_event_name:'SessionStart',source:'resume',cwd:${JSON.stringify(f.project)}}),encoding:'utf8'});
const out=JSON.parse(r.stdout);if(!out.hookSpecificOutput?.additionalContext.includes('Keep the public interface stable'))process.exit(42);`);
  const snapshot=fs.readdirSync(root).sort();
  const code=await adapter.launch(f.options,[],{probe:()=>true,retrieve:()=>adapter.retrieve(f.options,f.project),tempRoot:root,warn:s=>warnings.push(s),
    runProvider:(_exe,args)=>adapter.runProvider(process.execPath,[file,...args])});
  assert.equal(code,0);assert.match(warnings.join(),/1 contexts prepared, 1 output writes completed, 0 failure observations/);
  assert.deepEqual(fs.readdirSync(root).sort(),snapshot);
  fs.unlinkSync(file);assert.deepEqual(fs.readdirSync(root).sort(),before);
});

test('validated fingerprints ignore wire ordering and snapshot time but retain semantic changes',()=>{
  const f=fixture(),key='ab'.repeat(32),now=Math.floor(Date.now()/1000),p=f.packet();p.generated_unix=now;
  const original=adapter.fingerprint(wire(p),f.project,key,now);
  p.generated_unix=now+1;
  const reordered=Object.fromEntries(Object.entries(p).reverse());
  assert.equal(adapter.fingerprint(wire(reordered),f.project,key,now+1),original);
  for(const mutate of [p=>p.records[0].content+=' revised',p=>p.boundary_rules[0].policy='deny',
    p=>p.records[0].origin.source='new origin',p=>p.records[0].expires_unix=now+100,
    p=>p.omitted.stale++,p=>p.omitted.conflicted++]) {
    const changed=structuredClone(p);mutate(changed);
    assert.notEqual(adapter.fingerprint(wire(changed),f.project,key,now+1),original);
  }
  const other=fixture(),changed=structuredClone(p);changed.project.root=other.project;
  assert.notEqual(adapter.fingerprint(wire(changed),other.project,key,now+1),original);
  for(const mutate of [p=>p.records[0].expires_unix=now,p=>p.records[0].conflicted=true,p=>p.coverage.mandatory_context_complete=false]) {
    const invalid=structuredClone(p);mutate(invalid);assert.throws(()=>adapter.fingerprint(wire(invalid),f.project,key,now+1));
  }
});

test('identical lifecycle callbacks always write full context; only diagnostic identities are deduplicated',()=>{
  const f=fixture(),c=config(f);
  for(const source of ['startup','startup','resume','compact','clear','fork']) {
    const result=hook(f,c,{hook_event_name:'SessionStart',source,cwd:f.project,session_id:'private-session',prompt:'private-conversation'});
    assert.equal(result.status,0,result.stderr);
    assert.match(JSON.parse(result.stdout).hookSpecificOutput.additionalContext,/Keep the public interface stable/);
  }
  const summary=adapter.summarizeReceipts(c.setup.receipt);
  assert.deepEqual(summary,{callbacks:6,prepared:6,written:6,failures:0,repeated:5,unfinished:0});
  const log=fs.readFileSync(c.setup.receipt,'utf8');
  assert.doesNotMatch(log,/Keep the|private-session|private-conversation|transcript|project|dataDir/);
  const next=config(f);hook(f,next,{hook_event_name:'SessionStart',source:'resume',cwd:f.project});
  const first=log.trim().split('\n').map(JSON.parse).find(e=>e.fingerprint).fingerprint;
  const second=fs.readFileSync(next.setup.receipt,'utf8').trim().split('\n').map(JSON.parse).find(e=>e.fingerprint).fingerprint;
  assert.notEqual(first,second,'different launches must not create a cross-session memory tracking identifier');
});

test('failed fresh snapshots never reuse earlier context and recovery is eligible after failure',()=>{
  const {DatabaseSync}=require('node:sqlite');
  const f=fixture(),c=config(f),event={hook_event_name:'SessionStart',source:'compact',cwd:f.project};
  assert.ok(JSON.parse(hook(f,c,event).stdout).hookSpecificOutput);
  const db=new DatabaseSync(path.join(f.dataDir,'swapctx.db'));
  try {db.exec('UPDATE memory_metadata SET expires_at=1');} finally {db.close();}
  const failed=hook(f,c,event);assert.equal(failed.status,0,failed.stderr);
  assert.equal(JSON.parse(failed.stdout).hookSpecificOutput,undefined);
  assert.doesNotMatch(failed.stdout,/Keep the public interface stable/);
  const recovered=new DatabaseSync(path.join(f.dataDir,'swapctx.db'));
  try {recovered.exec('UPDATE memory_metadata SET expires_at=NULL');} finally {recovered.close();}
  assert.ok(JSON.parse(hook(f,c,event).stdout).hookSpecificOutput);
  assert.deepEqual(adapter.summarizeReceipts(c.setup.receipt),{callbacks:3,prepared:2,written:2,failures:1,repeated:1,unfinished:0});
  const other=fixture();event.source='fork';event.cwd=other.project;
  assert.equal(JSON.parse(hook(f,c,event).stdout).hookSpecificOutput,undefined);
  assert.ok(JSON.parse(hook(other,config(other),event).stdout).hookSpecificOutput);
});

test('diagnostic capacity and an abandoned append lock cannot disable validated recovery',()=>{
  const f=fixture();
  for(const mode of ['full','near-full','lock']) {
    const c=config(f),event={hook_event_name:'SessionStart',source:'resume',cwd:f.project};
    const size=adapter.RECEIPT_BYTES-(mode==='near-full'?1:0);
    if(mode!=='lock') fs.writeFileSync(c.setup.receipt,Buffer.alloc(size,32));
    else fs.writeFileSync(c.setup.receipt+'.lock','abandoned fixture lock');
    const result=hook(f,c,event);assert.equal(result.status,0,result.stderr);
    assert.ok(JSON.parse(result.stdout).hookSpecificOutput);
    assert.match(result.stderr,/metadata unavailable/);
    assert.ok(fs.existsSync(c.setup.receipt+'.incomplete'));
    if(mode!=='lock') assert.equal(fs.statSync(c.setup.receipt).size,size);
    else assert.equal(fs.existsSync(c.setup.receipt),false);
  }
});

test('abrupt callback termination leaves an unfinished observation and never poisons the next refresh',async()=>{
  const {spawn}=require('node:child_process');
  const f=fixture(),c=config(f);
  const child=spawn(process.execPath,[script,'hook',c.file],{cwd:f.project,stdio:['pipe','pipe','pipe']});
  let out='';child.stdout.on('data',chunk=>out+=chunk);child.stderr.resume();
  const closed=new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',resolve);});
  try {
    const deadline=Date.now()+2000;
    while(!fs.existsSync(c.setup.receipt) && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,10));
    assert.ok(fs.existsSync(c.setup.receipt),'callback must record entry before awaiting EOF');
  } finally {child.kill();await closed;}
  assert.equal(out,'');assert.equal(adapter.summarizeReceipts(c.setup.receipt).unfinished,1);
  const result=hook(f,c,{hook_event_name:'SessionStart',source:'resume',cwd:f.project});
  assert.equal(result.status,0,result.stderr);assert.ok(JSON.parse(result.stdout).hookSpecificOutput);
  assert.equal(adapter.summarizeReceipts(c.setup.receipt).written,1);
});
