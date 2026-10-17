'use strict';
// Offline only: real Rust CLI, in-process protocol fixtures, no Codex executable.
const {test,after} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {EventEmitter} = require('node:events');
const {spawn,spawnSync} = require('node:child_process');
const {DatabaseSync} = require('node:sqlite');
const {ManagedSession,AppServer,atomicJson,identity,paths,context,codexEnv,SUPPORTED} = require('../integrations/codex/session.cjs');
const {failureDetails,Observatory,snapshot,readSnapshot,observation,render} = require('../integrations/codex/observatory.cjs');
const {auditReport} = require('./audit-phase4.cjs');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-everyday-'));
const exe = path.resolve(process.argv[2] || 'target/debug/swapctx.exe');
const secret = 'PrivatePayloadNeverRetained123';
let sequence = 0;
function fixture() {
  const project = path.join(root,'project space '+(++sequence)),dataDir = path.join(root,'data '+sequence);
  fs.mkdirSync(project);
  function cli(args, input, cwd = project) {return spawnSync(exe,['--data-dir',dataDir,...args],{cwd,input,encoding:'utf8',timeout:15000});}
  function ok(args, input, cwd) {const result = cli(args,input,cwd);assert.equal(result.status,0,result.stdout+result.stderr);return result;}
  ok(['init']);
  const options = {project,dataDir,swapctx:exe};
  const files = paths(options,true);
  const settings = {version:1,project:identity(project),dataDir:identity(dataDir),codex:path.join(project,'NO-CODEX-EXECUTABLE.exe'),codexVersion:SUPPORTED,sandbox:'read-only'};
  atomicJson(files.settings,settings,false);
  const events = result => result.stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  return {project,dataDir,options,files,settings,cli,ok,events};
}
function fakeSession(f, behavior = {}) {
  const calls = [],events = [],instances = [];
  const threadId = crypto.randomUUID();
  class FakeApi extends EventEmitter {
    async initialize() {return {codexHome:behavior.home || f.project};}
    async call(method, params) {
      calls.push({method,params});
      if (method === 'account/read') return {requiresOpenaiAuth:false,account:null};
      if (method === 'config/read') return {config:{developer_instructions:behavior.policy || null}};
      if (method === 'thread/read') return {thread:{cwd:behavior.originalCwd || f.project}};
      if (['thread/start','thread/resume'].includes(method)) return {thread:{id:params.threadId || threadId,cwd:f.project,status:{type:'idle'}},sandbox:{type:params.sandbox === 'read-only'?'readOnly':'workspaceWrite'},model:'fixture-offline'};
      if (['turn/start','thread/compact/start'].includes(method)) {
        if (behavior.compact || method === 'thread/compact/start') {
          for (const name of ['started','completed']) this.emit('notification',{method:'item/'+name,params:{threadId:params.threadId,item:{type:'contextCompaction'}}});
        }
        this.emit('notification',{method:'turn/completed',params:{threadId:params.threadId,turn:behavior.turn || {status:'completed'}}});
        return {};
      }
      if (method === 'thread/inject_items') return {};
      throw Error('Unexpected fixture call '+method);
    }
    waitTurn(...args) {return AppServer.prototype.waitTurn.call(this,...args);}
    abort(code) {this.emit('disconnect',code);}
    async close() {this.closed = true;}
  }
  const session = new ManagedSession(f.options,f.settings,f.files.state,e => events.push(e),{
    checkVersion:(_exe,env) => {if (behavior.expectedHome) assert.equal(env.CODEX_HOME,behavior.expectedHome);},
    createApi:(_exe,cwd,env) => {assert.equal(cwd,f.project);if (behavior.expectedHome) assert.equal(env.CODEX_HOME,behavior.expectedHome);const api = new FakeApi();instances.push(api);return api;}
  });
  return {session,calls,events,instances};
}
test('legacy settings and local commands work with no installed Codex, including nested discovery',() => {
  const f = fixture(),nested = path.join(f.project,'nested');fs.mkdirSync(nested);
  const before = fs.readFileSync(path.join(f.dataDir,'swapctx.db'));
  const result = f.ok(['codex','--json','status'],undefined,nested);
  const s = f.events(result)[0];assert.equal(s.offline,true);assert.equal(s.context.coverage.mandatory_context_complete,true);assert.equal(s.explicitCodexHome,false);assert.equal(s.savedSession,null);
  assert.deepEqual(fs.readFileSync(path.join(f.dataDir,'swapctx.db')),before);
  const run = f.ok(['codex','--json','run'],'/help\n/status\n/context\n/exit\n',nested);
  assert.ok(f.events(run).some(e => e.event === 'help'));
  assert.equal(fs.existsSync(f.files.lock),false);assert.equal(fs.existsSync(f.files.state),false);assert.equal(fs.existsSync(f.files.observatory),false);
});
test('explicit console memory has manual provenance and is retrieved by the real CLI',() => {
  const f = fixture();
  const run = f.ok(['codex','--json','run'],'/remember Ordinary local note\n/requirement Preserve public API\n/context\n/exit\n');
  const events = f.events(run),packet = events.find(e => e.event === 'context_preview').packet;
  assert.equal(events.filter(e => e.event === 'memory_saved').length,2);
  assert.equal(packet.records[0].kind,'requirement');assert.equal(packet.records[0].content,'Preserve public API');
  assert.equal(packet.records[0].origin.source,'managed-console');assert.equal(packet.records[0].origin.actor,'user');
  assert.equal(packet.coverage.requirements_returned,1);
  assert.equal(fs.existsSync(f.files.state),false);
});
test('secret-like console memory fails without disclosure or storing the value',() => {
  const f = fixture();
  const result = f.cli(['codex','--json','run'],'/remember password='+secret+'\n');
  assert.equal(result.status,1);assert.equal(JSON.parse(result.stderr).error.code,'memory_write_failed');
  assert.ok(!(result.stdout+result.stderr).includes(secret));assert.equal(context(f.options).packet.records.length,0);assert.equal(fs.existsSync(f.files.lock),false);
});
test('unknown console slash commands fail before any agent process or session state',() => {
  const f = fixture();const result = f.cli(['codex','--json','run'],'/comapct\n');
  assert.equal(result.status,1);assert.equal(JSON.parse(result.stderr).error.code,'unknown_console_command');assert.equal(fs.existsSync(f.files.state),false);assert.equal(fs.existsSync(f.files.lock),false);
});
test('local query validation and preview retain mandatory requirements',() => {
  const f = fixture();f.ok(['memory','add','Keep contract','--kind','requirement']);f.ok(['memory','add','Other note']);
  const events = f.events(f.ok(['codex','--json','run'],'/query unrelated\n/context\n/query\n/context\n/exit\n'));
  const previews = events.filter(e => e.event === 'context_preview');assert.equal(previews[0].packet.records.length,1);assert.equal(previews[1].packet.records.length,2);
});
test('status reports incomplete requirements locally without claiming delivery',() => {
  const f = fixture();f.ok(['memory','add','Old requirement','--kind','requirement']);
  const db = new DatabaseSync(path.join(f.dataDir,'swapctx.db'));try {db.exec('UPDATE memory_metadata SET expires_at=1');} finally {db.close();}
  const s = f.events(f.ok(['codex','--json','status']))[0];assert.equal(s.context.coverage.mandatory_context_complete,false);assert.equal(s.modelUseVerified,false);
  const fake = fakeSession(f);
  return assert.rejects(fake.session.perform('not sent'),e => e.code === 'mandatory_context_incomplete').then(() => {assert.equal(fake.instances.length,0);assert.equal(fake.calls.length,0);});
});
test('optional explicit Codex home preserves sandbox environment and legacy behavior',async() => {
  const source = {CODEX_HOME:'previous',CODEX_SANDBOX_NETWORK_DISABLED:'1',OTHER:'retained'};
  assert.strictEqual(codexEnv({},source),source);
  assert.deepEqual(codexEnv({codexHome:'chosen'},source),{...source,CODEX_HOME:'chosen'});assert.equal(source.CODEX_HOME,'previous');
  const f = fixture();f.settings.codexHome = f.project;atomicJson(f.files.settings,f.settings);
  assert.equal(f.events(f.ok(['codex','--json','status']))[0].explicitCodexHome,true);
  const fake = fakeSession(f,{expectedHome:f.project});await fake.session.perform('fixture prompt');assert.ok(fake.instances.every(a => a.closed));
});
test('actual initialized home must match explicit settings before auth or thread calls',async() => {
  const f = fixture();f.settings.codexHome = f.dataDir;
  const fake = fakeSession(f);await assert.rejects(fake.session.perform('not sent'),e => e.code === 'codex_home_mismatch');assert.equal(fake.calls.length,0);assert.ok(fake.instances[0].closed);
});
test('startup, unchanged resume, supersession, and compaction use fresh context with acknowledged injection',async() => {
  const f = fixture();f.ok(['memory','add','Old public contract','--kind','requirement']);
  const fake = fakeSession(f);
  await fake.session.perform('fixture startup');
  assert.equal(fake.calls.filter(c => c.method === 'thread/start').length,1);
  assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,0);
  await fake.session.perform('fixture resume');assert.equal(fake.calls.filter(c => c.method === 'thread/resume').length,1);assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,0);
  f.ok(['memory','supersede','1','Updated public contract']);
  await fake.session.perform('fixture changed context');
  const injections = fake.calls.filter(c => c.method === 'thread/inject_items');assert.equal(injections.length,1);
  assert.ok(injections[0].params.items[0].content[0].text.includes('Updated public contract'));assert.ok(!injections[0].params.items[0].content[0].text.includes('Old public contract'));
  const lastTurn = fake.calls.findLastIndex(c => c.method === 'turn/start'),injected = fake.calls.findLastIndex(c => c.method === 'thread/inject_items');assert.ok(injected < lastTurn);
  await fake.session.perform('',{compact:true});assert.ok(fake.events.some(e => e.event === 'compaction_completed'));assert.equal(fake.events.filter(e => e.event === 'turn_completed').length,4);
  assert.ok(fake.instances.every(a => a.closed));
  const status = f.events(f.ok(['codex','--json','status']))[0];assert.equal(status.savedSession.contextCurrent,true);
  f.ok(['memory','add','New requirement','--kind','requirement']);assert.equal(f.events(f.ok(['codex','--json','status']))[0].savedSession.contextCurrent,false);
});
test('changed native developer instructions are retained in the explicit history update',async() => {
  const f = fixture(),behavior = {policy:'Initial developer policy'},fake = fakeSession(f,behavior);
  await fake.session.perform('fixture startup');behavior.policy = 'Updated developer policy';await fake.session.perform('fixture resume');
  const text = fake.calls.find(c => c.method === 'thread/inject_items').params.items[0].content[0].text;assert.ok(text.startsWith('Updated developer policy\n'));assert.ok(text.includes('<swapctx_context_v1>'));
});
test('excluded history and query explanations do not reinject unchanged selected knowledge',async() => {
  const f = fixture();f.options.query = 'target';f.ok(['memory','add','Preserve target contract','--kind','requirement']);
  const fake = fakeSession(f);await fake.session.perform('fixture startup');const first = context(f.options);
  f.ok(['memory','add','Unrelated archived note']);f.ok(['memory','archive','2']);
  f.ok(['memory','add','target password='+secret,'--allow-sensitive']);
  const after = context(f.options);assert.equal(after.packet.omitted.inactive_or_invalid,1);assert.equal(after.packet.omitted.sensitive,1);assert.equal(first.hash,after.hash);assert.equal(after.hashVersion,2);
  await fake.session.perform('fixture unchanged');assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,0);
  f.options.query = 'elsewhere';assert.equal(context(f.options).hash,first.hash);await fake.session.perform('fixture other query');assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,0);
  f.options.query = 'target';f.ok(['memory','add','target current architecture']);await fake.session.perform('fixture new knowledge');assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,1);
  f.ok(['memory','review','1','--expires-in-days','30']);await fake.session.perform('fixture lifecycle update');assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,2);
});
test('legacy fingerprints and missing legacy policy hash refresh once without replacing the thread',async() => {
  const f = fixture(),fake = fakeSession(f);await fake.session.perform('fixture startup');
  const prior = JSON.parse(fs.readFileSync(f.files.state,'utf8'));prior.contextHash = 'f'.repeat(64);delete prior.contextHashVersion;delete prior.policyHash;atomicJson(f.files.state,prior);
  await fake.session.perform('fixture legacy refresh');await fake.session.perform('fixture unchanged');
  assert.equal(fake.calls.filter(c => c.method === 'thread/start').length,1);assert.equal(fake.calls.filter(c => c.method === 'thread/resume').length,2);assert.equal(fake.calls.filter(c => c.method === 'thread/inject_items').length,1);
  const state = JSON.parse(fs.readFileSync(f.files.state,'utf8'));assert.equal(state.threadId,prior.threadId);assert.equal(state.contextHashVersion,2);assert.equal(state.contextHash,context(f.options).hash);
});
test('damaged references refuse both resume and new before version checks or native startup',async() => {
  for (const value of [null,[],{version:2},{version:1,project:'foreign',home:'wrong'},'{']) {
    const f = fixture();const before = typeof value === 'string'?value:JSON.stringify(value);fs.writeFileSync(f.files.state,before);
    const fake = fakeSession(f);let versionChecks = 0;fake.session.checkVersion = () => {versionChecks++;};
    for (const fresh of [false,true]) await assert.rejects(fake.session.perform('not sent',{fresh}),e => e.code === 'invalid_session_state');
    assert.equal(versionChecks,0);assert.equal(fake.instances.length,0);assert.equal(fs.readFileSync(f.files.state,'utf8'),before);
    const status = f.cli(['codex','--json','status']);assert.equal(status.status,1);assert.equal(JSON.parse(status.stderr).error.code,'invalid_session_state');
  }
});
test('dangling session references fail locally and are preserved even with --new',async() => {
  const f = fixture(),target = path.join(root,'missing-session-target');fs.mkdirSync(target);fs.symlinkSync(target,f.files.state,process.platform === 'win32'?'junction':'dir');fs.rmdirSync(target);
  const fake = fakeSession(f);let checked = false;fake.session.checkVersion = () => {checked = true;};
  await assert.rejects(fake.session.perform('not sent',{fresh:true}),e => e.code === 'invalid_session_state');assert.equal(checked,false);assert.equal(fake.instances.length,0);assert.equal(fs.lstatSync(f.files.state).isSymbolicLink(),true);
  const status = f.cli(['codex','--json','status']);assert.equal(status.status,1);assert.equal(JSON.parse(status.stderr).error.code,'invalid_session_state');
});
test('foreign thread roots refuse before resume and preserve the saved reference',async() => {
  const f = fixture(),behavior = {},fake = fakeSession(f,behavior);await fake.session.perform('fixture startup');
  const before = fs.readFileSync(f.files.state);behavior.originalCwd = f.dataDir;
  await assert.rejects(fake.session.perform('not sent'),e => e.code === 'codex_scope_mismatch');assert.equal(fake.calls.filter(c => c.method === 'thread/resume').length,0);assert.deepEqual(fs.readFileSync(f.files.state),before);
});
test('failed turns retain safe native categories but discard provider messages and details',async() => {
  const f = fixture();const fake = fakeSession(f,{turn:{status:'failed',error:{message:secret,additionalDetails:secret,codexErrorInfo:{responseStreamDisconnected:{httpStatusCode:503,private:secret}}}}});
  await assert.rejects(fake.session.perform('fixture failed turn'),e => e.code === 'codex_turn_failed' && e.details.category === 'responseStreamDisconnected' && e.details.httpStatusCode === 503);
  assert.deepEqual(fake.events.find(e => e.event === 'turn_failed').details,{status:'failed',category:'responseStreamDisconnected',httpStatusCode:503});assert.ok(!JSON.stringify(fake.events).includes(secret));
  assert.equal(fake.events.at(-1).success,false);assert.equal(fake.events.at(-1).phase,'turn');assert.equal(fake.calls.filter(c => c.method === 'turn/start').length,1);assert.ok(fake.instances[0].closed);
});
test('unknown and malformed native errors never leak through failure diagnostics',() => {
  for (const info of [secret,{[secret]:{message:secret}},{serverOverloaded:{},unauthorized:{}},null,12]) assert.equal(failureDetails({status:secret,error:{codexErrorInfo:info}}).category,'unknown');
  assert.deepEqual(failureDetails({status:'interrupted',error:{codexErrorInfo:'usageLimitExceeded'}}),{status:'interrupted',category:'usageLimitExceeded'});
  assert.equal(failureDetails({error:{codexErrorInfo:{httpConnectionFailed:{httpStatusCode:-1}}}}).httpStatusCode,undefined);
  const events = [],session = new ManagedSession({}, {},'',e => events.push(e));
  session.observe({method:'error',params:{error:{message:secret,codexErrorInfo:'rateLimitExceeded'},willRetry:true}});
  assert.deepEqual(events,[{event:'runtime_error',details:{status:'failed',category:'rateLimitExceeded'},willRetry:true}]);
  session.observe({method:'hook/completed',params:{run:{status:secret}}});assert.deepEqual(events.at(-1),{event:'hook_observed',status:'unknown'});
});
test('observatory is opt-in, ignores all text deltas, coalesces events and bounds snapshots',() => {
  const file = path.join(root,'observable.json');let writes = 0,now = 1000;
  const observer = new Observatory(file,{publish:(p,v) => {writes++;atomicJson(p,v);},clock:() => now});
  observer.accept({event:'operation_started'});assert.equal(writes,0);assert.equal(fs.existsSync(file),false);
  observer.enable();assert.equal(writes,1);
  for (let i=0;i<10000;i++) observer.accept({event:'text',text:secret});
  assert.equal(writes,1);assert.equal(observer.timer,null);
  for (let i=0;i<100;i++) observer.accept({event:'token_usage',last:{inputTokens:i,cachedInputTokens:0,outputTokens:0,reasoningOutputTokens:0,totalTokens:i},total:{inputTokens:i,cachedInputTokens:0,outputTokens:0,reasoningOutputTokens:0,totalTokens:i},private:secret});
  assert.equal(writes,1);assert.ok(observer.timer);now = 2000;observer.flush();assert.equal(writes,2);assert.equal(readSnapshot(file).usage.last.inputTokens,99);
  observer.accept({event:'context_selected',hash:'a'.repeat(64),bytes:8192,records:1,requirements:1,private:secret});
  observer.accept({event:'tool_completed',status:'completed',exitCode:1,command:secret});
  observer.accept({event:'turn_failed',details:{status:'failed',category:'unauthorized',message:secret}});
  observer.accept({event:'operation_metrics',success:false,totalMs:3,adapterRssBytes:123,private:secret});
  const value = readSnapshot(file);assert.equal(value.state,'failed');assert.equal(value.failedTurns,1);assert.equal(value.toolFailures,1);assert.equal(value.failure.category,'unauthorized');assert.equal(value.context.bytes,8192);
  assert.ok(!fs.readFileSync(file,'utf8').includes(secret));assert.ok(fs.statSync(file).size<16384);
  observer.disable();assert.equal(readSnapshot(file).state,'disabled');const disabledWrites = writes;observer.accept({event:'operation_started'});observer.close();assert.equal(writes,disabledWrites);assert.equal(observer.timer,null);
});
test('snapshot writer failures disable observation and cannot break the managed operation',() => {
  let warnings = 0,writes = 0;
  const observer = new Observatory(path.join(root,'not-written'),{publish:() => {writes++;throw Error(secret);},onError:() => warnings++});
  assert.doesNotThrow(() => observer.enable());observer.accept({event:'operation_started'});observer.close();assert.equal(warnings,1);assert.equal(writes,1);assert.equal(observer.enabled,false);
});
test('snapshot ownership or schema changes disable writes while preserving the replacement',() => {
  for (const type of ['owner','version']) {
    const file = path.join(root,'changed-'+type+'.json');let warnings = 0;
    const observer = new Observatory(file,{publish:atomicJson,onError:() => warnings++});observer.enable();
    const replacement = JSON.parse(fs.readFileSync(file,'utf8'));
    if (type === 'owner') replacement.pid = process.pid+1;else replacement.version = 2;
    fs.writeFileSync(file,JSON.stringify(replacement));const before = fs.readFileSync(file);
    observer.accept({event:'operation_metrics',success:true,phase:'completed'});
    assert.deepEqual(fs.readFileSync(file),before);assert.equal(observer.enabled,false);assert.equal(warnings,1);observer.close();
  }
});
test('foreign, future, oversized and malformed observation files are refused without replacement',() => {
  for (const [name,text] of [['future',JSON.stringify({protocol:'swapctx.observatory',version:2})],['foreign','{}'],['oversize','x'.repeat(16385)],['utf8',Buffer.from([255])]]) {
    const file = path.join(root,name);fs.writeFileSync(file,text);const before = fs.readFileSync(file),observer = new Observatory(file,{publish:atomicJson});
    assert.throws(() => observer.enable());assert.equal(observer.enabled,false);assert.deepEqual(fs.readFileSync(file),before);assert.throws(() => readSnapshot(file));
  }
  assert.throws(() => snapshot({protocol:'swapctx.observatory',version:1,state:'idle',pid:1,startedUnixMs:1,updatedUnixMs:1,enabled:'yes'}));
});
test('watch once reads only local snapshots, with age and process liveness separated from state',() => {
  const f = fixture();assert.equal(f.events(f.ok(['codex','--json','watch','--once']))[0].event,'observatory_absent');
  const run = f.ok(['codex','--json','run','--observe'],'/status\n/exit\n');assert.ok(f.events(run).some(e => e.event === 'local_status'));
  const value = f.events(f.ok(['codex','--json','watch','--once']))[0];assert.equal(value.offline,true);assert.equal(value.snapshot.state,'closed');assert.equal(value.snapshot.enabled,true);assert.equal(value.writerRunning,false);assert.ok(value.ageMs>=0);
  const current = observation(f.files.observatory);assert.ok(render(current).includes('writer absent'));
  assert.ok(!fs.existsSync(f.files.state));assert.ok(!fs.existsSync(f.files.lock));
});
test('live local watcher reports snapshot changes and releases its timer on interruption',async() => {
  const f = fixture(),observer = new Observatory(f.files.observatory,{publish:atomicJson});observer.enable();
  // A Node fixture only: main(watch) cannot invoke Codex, and its configured executable does not exist.
  const program = `const {main}=require(process.argv[1]);process.stdin.once('data',()=>process.emit('SIGINT'));main(JSON.parse(process.argv[2])).catch(()=>{process.exitCode=1;});`;
  const args = ['watch','--json','--swapctx',exe,'--data-dir',f.dataDir,'--project',f.project];
  const child = spawn(process.execPath,['-e',program,path.resolve(__dirname,'../integrations/codex/session.cjs'),JSON.stringify(args)],{stdio:['pipe','pipe','pipe'],windowsHide:true,shell:false});
  let phase = 0,stderr = '';
  try {
    await new Promise((resolve,reject) => {
      const timer = setTimeout(() => {child.kill();reject(Error('Local watcher did not stop'));},10000);
      child.once('error',e => {clearTimeout(timer);reject(e);});
      child.stderr.on('data',chunk => {stderr += chunk;});child.stdin.on('error',() => {});
      require('node:readline').createInterface({input:child.stdout}).on('line',line => {
        try {
          const value = JSON.parse(line);assert.equal(value.event,'observatory');assert.equal(value.offline,true);
          if (phase === 0) {phase = 1;observer.accept({event:'operation_metrics',success:false,phase:'context'});}
          else if (phase === 1 && value.snapshot.failedTurns === 1) {phase = 2;child.stdin.end('stop\n');}
        } catch(e) {clearTimeout(timer);child.kill();reject(e);}
      });
      child.once('close',code => {clearTimeout(timer);try {assert.equal(code,130);assert.equal(phase,2);assert.equal(stderr,'');resolve();} catch(e) {reject(e);}});
    });
  } finally {child.kill();observer.close();}
});
test('observatory can be enabled/disabled within a local-only console without starting Codex',() => {
  const f = fixture();f.ok(['codex','--json','run'],'/observe on\n/observe off\n/exit\n');assert.equal(readSnapshot(f.files.observatory).state,'disabled');assert.equal(readSnapshot(f.files.observatory).enabled,false);
});
test('bad observatory metadata cannot overwrite user files and releases the console lock',() => {
  const f = fixture();fs.writeFileSync(f.files.observatory,'{"version":99}');
  const result = f.cli(['codex','--json','run','--observe'],'/exit\n');assert.equal(result.status,1);assert.equal(JSON.parse(result.stderr).error.code,'invalid_observatory_snapshot');assert.equal(fs.readFileSync(f.files.observatory,'utf8'),'{"version":99}');assert.equal(fs.existsSync(f.files.lock),false);
});
test('dangling junctions/symlinks are existing unsafe metadata, not absent files',() => {
  const f = fixture(),target = path.join(root,'dangling-target');fs.mkdirSync(target);
  fs.symlinkSync(target,f.files.observatory,process.platform === 'win32'?'junction':'dir');fs.rmdirSync(target);
  assert.equal(fs.existsSync(f.files.observatory),false);assert.equal(fs.lstatSync(f.files.observatory).isSymbolicLink(),true);
  assert.throws(() => atomicJson(f.files.observatory,{wrong:true}),e => e.code === 'unsafe_local_metadata');
  const observer = new Observatory(f.files.observatory,{publish:atomicJson});assert.throws(() => observer.enable());
  const run = f.cli(['codex','--json','run','--observe'],'/exit\n');assert.equal(run.status,1);assert.equal(JSON.parse(run.stderr).error.code,'invalid_observatory_snapshot');
  const watch = f.cli(['codex','--json','watch','--once']);assert.equal(watch.status,1);assert.equal(JSON.parse(watch.stderr).error.code,'invalid_observatory_snapshot');
  assert.equal(fs.lstatSync(f.files.observatory).isSymbolicLink(),true);assert.equal(fs.existsSync(f.files.lock),false);
});
test('offline evidence audit distinguishes incomplete pairs from retained behavior passes',() => {
  const turn = {round:0,behaviorPassed:true,automaticCompactions:0};
  const report = {protocol:'swapctx.codex-soak',version:1,status:'failed',requestedTurnsPerArm:2,arms:{managed:[turn],baseline:[turn,{...turn,round:1}]},error:{code:'codex_turn_failed',message:secret},comparison:{managed:{turns:1,behaviorPasses:1},baseline:{turns:2,behaviorPasses:2},automaticRecoveryObserved:false}};
  const audit = auditReport(report);assert.equal(audit.pairedTurnsComplete,false);assert.equal(audit.arms.managed.behaviorPasses,1);assert.equal(audit.arms.baseline.behaviorPasses,2);assert.equal(audit.arms.managed.reportedSummaryConsistent,true);assert.equal(audit.failure.rootCauseRecoverable,false);assert.ok(!JSON.stringify(audit).includes(secret));assert.equal(audit.comparativeBenefitEstablished,false);
  assert.throws(() => auditReport({...report,version:2}));assert.throws(() => auditReport({...report,arms:{managed:[{...turn,round:2}],baseline:[]}}));
});
after(() => {assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});});
