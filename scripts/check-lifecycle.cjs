'use strict';
// Real native Codex app-server + real SwapCtx CLI, synthetic loopback provider.
// Fixture behavior is NOT evidence of AI model reasoning or adherence.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const {spawn,spawnSync} = require('node:child_process');
const {DatabaseSync} = require('node:sqlite');
const {AppServer} = require('../integrations/codex/session.cjs');
const codex = path.resolve(process.argv[2]);
const exe = path.resolve(process.argv[3] || 'target/debug/swapctx.exe');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-lifecycle-'));
const project = path.join(root,'project ü space'), data = path.join(root,'data'), home = path.join(root,'home');
fs.mkdirSync(project);fs.mkdirSync(home);fs.mkdirSync(path.join(project,'nested'));
const env = {CODEX_HOME:home};
for (const key of ['PATH','SystemRoot','WINDIR','USERPROFILE','COMSPEC','TEMP','TMP','LOCALAPPDATA','APPDATA']) if (process.env[key]) env[key] = process.env[key];
const requests = [];let sequence = 0,checks = 0,mode = 'normal',autoStage = 0,serverFailure;
function developer(body) {
  return (body.input || []).filter(item => item.role === 'developer').flatMap(item => typeof item.content === 'string'?[item.content]:(item.content || []).map(part => part.text || '')).join('\n');
}
function latestContext(body) {
  const text = developer(body),start = text.lastIndexOf('<swapctx_context_v1>');
  assert.ok(start >= 0,'Current context block missing');
  return JSON.parse(text.slice(start+'<swapctx_context_v1>'.length,text.indexOf('</swapctx_context_v1>',start)));
}
function sse(res, output, tokens = 2) {
  const id = 'fixture_'+(++sequence);
  output = output.map((item,i) => ({id:id+'_'+i,...item}));
  const response = {id:'resp_'+id,object:'response',created_at:1,status:'completed',model:'gpt-6.1-sol',output,usage:{input_tokens:tokens-1,output_tokens:1,total_tokens:tokens}};
  res.writeHead(200,{'content-type':'text/event-stream'});
  const events = [{type:'response.created',response:{...response,status:'in_progress',output:[]}}];
  output.forEach((item,i) => {
    events.push({type:'response.output_item.added',output_index:i,item:{...item,status:'in_progress'}});
    if (item.type === 'message') events.push({type:'response.output_text.delta',item_id:item.id,output_index:i,content_index:0,delta:item.content[0].text});
    events.push({type:'response.output_item.done',output_index:i,item});
  });
  events.push({type:'response.completed',response});
  for (const event of events) res.write('event: '+event.type+'\ndata: '+JSON.stringify(event)+'\n\n');
  res.end();
}
const server = http.createServer((req,res) => {
  let raw = '';req.on('data',d => raw += d);req.on('end',() => {try {
    const body = JSON.parse(raw);requests.push({url:req.url,body});
    if (req.url.endsWith('/compact')) {
      res.writeHead(200,{'content-type':'application/json'});
      res.end(JSON.stringify({id:'cmp_fixture',object:'response.compaction',created_at:1,output:[{type:'compaction',id:'cmp_item',encrypted_content:'fixture-summary-with-no-knowledge'}],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}));return;
    }
    if (mode === 'auto' && autoStage++ === 0) {
      // Deliberately unsupported: exercise a tool error followed by native
      // automatic compaction, without granting permissions or touching files.
      sse(res,[{type:'function_call',call_id:'call_fixture_error',name:'deliberately_unavailable_fixture_tool',arguments:'{}'}],200000);return;
    }
    const text = mode === 'compact' || (mode === 'auto' && autoStage === 2)
      ? 'Fixture summary deliberately contains no saved project requirements.'
      : 'Fixture assistant output; no model inference.';
    sse(res,[{type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text,annotations:[]}]}]);
  } catch (e) {serverFailure = e;res.writeHead(500);res.end('Fixture assertion failed');}});
});
function cli(args, status = 0, cwd = project) {
  const r = spawnSync(exe,['--data-dir',data,...args],{cwd,env,encoding:'utf8',timeout:20000});
  assert.equal(r.status,status,r.stderr+'\n'+r.stdout);return r;
}
function events(stdout) {return stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));}
function check(name, fn) {fn();checks++;console.log('PASS '+name);}
function state() {const file = fs.readdirSync(path.join(data,'codex')).find(name => name.endsWith('.session.json'));return JSON.parse(fs.readFileSync(path.join(data,'codex',file),'utf8'));}
function add(text, args = []) {cli(['memory','add',text,...args]);}
async function run(input, status = 0, flags = [], cwd = project) {
  const start = requests.length;
  const p = spawn(exe,['--data-dir',data,'codex','--json','run',...flags],{cwd,env,stdio:['pipe','pipe','pipe'],windowsHide:true});
  let stdout = '',stderr = '';p.stdout.on('data',d => stdout += d);p.stderr.on('data',d => stderr += d);p.stdin.end(input);
  const result = await new Promise((resolve,reject) => {
    const timer = setTimeout(() => {p.kill();reject(Error('Managed CLI timed out'));},30000);
    p.once('error',e => {clearTimeout(timer);reject(e);});p.once('close',code => {clearTimeout(timer);resolve({code,stdout,stderr});});
  });
  assert.equal(result.code,status,stderr+'\n'+stdout);
  return {...result,events:events(stdout),sent:requests.slice(start)};
}
(async () => {
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  fs.writeFileSync(path.join(home,'config.toml'),`model="gpt-6.1-sol"\nmodel_provider="fixture"\ndeveloper_instructions="BASELINE_POLICY_MUST_REMAIN"\nmodel_auto_compact_token_limit=100000\n[analytics]\nenabled=false\n[model_providers.fixture]\nname="SwapCtx local fixture"\nbase_url="http://127.0.0.1:${server.address().port}/v1"\nwire_api="responses"\nrequires_openai_auth=false\nsupports_websockets=false\nrequest_max_retries=0\nstream_max_retries=0\n`);
  cli(['init']);add('REQUIREMENT_START_49F20',['--kind','requirement']);add('password=WITHHOLD_SECRET_123456',['--allow-sensitive']);
  check('native setup is idempotent, no model calls and no Codex config edits',() => {
    const before = fs.readFileSync(path.join(home,'config.toml'));
    for (let i=0;i<2;i++) cli(['codex','--json','setup','--codex',codex]);
    cli(['codex','--json','doctor']);assert.equal(requests.length,0);assert.deepEqual(fs.readFileSync(path.join(home,'config.toml')),before);
  });
  const first = await run('Explain the current requirement\n');const thread = state().threadId;
  check('startup sends selected context with existing developer instructions preserved',() => {
    assert.ok(first.sent.length > 0);const text = developer(first.sent.at(-1).body);assert.ok(text.includes('REQUIREMENT_START_49F20'));assert.ok(text.includes('BASELINE_POLICY_MUST_REMAIN'));
    assert.ok(!JSON.stringify(first.sent).includes('WITHHOLD_SECRET'));assert.ok(first.events.some(e => e.event === 'turn_completed'));assert.equal(first.events.find(e => e.event === 'session_ready').resumed,false);
  });
  cli(['memory','supersede','1','REQUIREMENT_RESUME_49F20']);
  const second = await run('Explain the updated requirement\n',0,[],path.join(project,'nested'));
  check('fresh-process resume keeps thread history and replaces the obsolete context block',() => {
    assert.equal(state().threadId,thread);const body = second.sent.at(-1).body;const packet = JSON.stringify(latestContext(body));assert.ok(packet.includes('REQUIREMENT_RESUME_49F20'));assert.ok(!packet.includes('REQUIREMENT_START_49F20'));assert.ok(developer(body).includes('BASELINE_POLICY_MUST_REMAIN'));assert.equal(second.events.find(e => e.event === 'session_ready').resumed,true);
  });
  mode = 'compact';const compacted = await run('/compact\n');mode = 'normal';
  check('actual Codex manual compaction completes and is observable',() => {
    assert.ok(compacted.events.some(e => e.event === 'compaction_completed'));assert.ok(compacted.events.some(e => e.event === 'turn_completed' && e.operation === 'compact'));
  });
  cli(['memory','supersede','3','REQUIREMENT_AFTER_COMPACT_49F20']);
  const after = await run('Explain the current requirement after compaction\n');
  check('post-compaction continuation receives current knowledge absent from fixture summary',() => {
    const text = JSON.stringify(latestContext(after.sent.at(-1).body));assert.ok(text.includes('REQUIREMENT_AFTER_COMPACT_49F20'));assert.ok(!text.includes('REQUIREMENT_RESUME_49F20'));assert.equal(state().threadId,thread);
  });
  mode = 'auto';autoStage = 0;const automatic = await run('Continue after a fixture tool call\n');mode = 'normal';
  check('mid-turn tool-error compaction retains pinned context in the immediate continuation',() => {
    assert.ok(automatic.events.some(e => e.event === 'compaction_started'));assert.ok(automatic.events.some(e => e.event === 'compaction_completed'));assert.ok(automatic.sent.length >= 3);
    assert.ok(JSON.stringify(latestContext(automatic.sent.at(-1).body)).includes('REQUIREMENT_AFTER_COMPACT_49F20'));
    assert.ok(automatic.sent.some(r => (r.body.input || []).some(item => item.type === 'function_call_output' && /unknown|unsupported|unavailable|not found/i.test(String(item.output)))),'Actual runtime must report the unsupported fixture tool');
  });
  const newThread = await run('Start another thread\n',0,['--new']);
  check('explicit new sessions preserve previous thread references',() => {
    assert.notEqual(state().threadId,thread);assert.ok(fs.readdirSync(path.join(data,'codex')).some(n => n.includes('.previous-')));assert.equal(newThread.events.find(e => e.event === 'session_ready').resumed,false);
  });
  check('configuration changes require explicit replacement and retain a backup',() => {
    const before = requests.length;
    cli(['codex','--json','setup','--codex',codex,'--workspace-write'],1);
    cli(['codex','--json','setup','--codex',codex,'--workspace-write','--replace']);
    assert.ok(fs.readdirSync(path.join(data,'codex')).some(n=>n.includes('.settings.json.previous-')));
    cli(['codex','--json','setup','--codex',codex,'--replace']);assert.equal(requests.length,before);
  });
  check('unsupported executables cannot silently replace a working configuration',() => {
    const r=cli(['codex','--json','setup','--codex',process.execPath,'--replace'],1);
    assert.equal(JSON.parse(r.stderr).error.code,'unsupported_codex_version');
  });
  const directory=path.join(data,'codex');
  const lock=path.join(directory,fs.readdirSync(directory).find(n=>n.endsWith('.settings.json')).replace('.settings.json','.lock'));
  fs.mkdirSync(lock);fs.writeFileSync(path.join(lock,'owner.json'),JSON.stringify({pid:process.pid}));
  const busy=await run('Must not start concurrently\n',1);
  check('concurrent consoles and recovery of a live lock are refused',()=>{
    assert.equal(busy.sent.length,0);assert.equal(JSON.parse(busy.stderr).error.code,'session_busy_or_stale_lock');
    const r=cli(['codex','--json','recover-lock'],1);assert.equal(JSON.parse(r.stderr).error.code,'lock_owner_still_running');assert.equal(fs.existsSync(lock),true);
  });
  fs.writeFileSync(path.join(lock,'owner.json'),JSON.stringify({pid:2147483647}));
  check('explicit dead-process lock recovery removes only validated lock metadata',()=>{
    cli(['codex','--json','recover-lock']);assert.equal(fs.existsSync(lock),false);
  });
  for(const [input,code] of [[Buffer.from([255,10]),'invalid_prompt_encoding'],['x'.repeat(16385)+'\n','invalid_prompt'],['/context\n'.repeat(33),'too_many_queued_prompts']]) {
    const invalid=await run(input,1);
    check('invalid console input is bounded and rejected: '+code,()=>{assert.equal(invalid.sent.length,0);assert.equal(JSON.parse(invalid.stderr).error.code,code);assert.equal(fs.existsSync(lock),false);});
  }
  add('topic_alpha saved architecture');add('topic_beta independent note');
  const filtered=await run('/query topic_alpha\n/context\n/exit\n');
  check('query and preview are local, scoped and leave unrelated notes out',()=>{
    assert.equal(filtered.sent.length,0);const packet=filtered.events.find(e=>e.event==='context_preview').packet;const text=JSON.stringify(packet);assert.ok(text.includes('topic_alpha'));assert.ok(!text.includes('topic_beta'));assert.ok(packet.records.some(r=>r.kind==='requirement'));
  });
  const foreignProject=path.join(root,'other-project');fs.mkdirSync(foreignProject);
  const foreignApi=new AppServer(codex,foreignProject,env);let foreignThread;
  try {
    await foreignApi.initialize();foreignThread=(await foreignApi.call('thread/start',{cwd:foreignProject,sandbox:'read-only',approvalPolicy:'never'})).thread.id;
    // Codex does not persist an unused thread. Complete a fixture turn so the
    // next process can read its original scope before attempting resumption.
    const pending=foreignApi.waitTurn(foreignThread);pending.promise.catch(()=>{});
    await foreignApi.call('turn/start',{threadId:foreignThread,input:[{type:'text',text:'Persist the isolated foreign-thread fixture'}]});
    await pending.promise;
  } finally {await foreignApi.close();}
  const stateFile=path.join(directory,fs.readdirSync(directory).find(n=>n.endsWith('.session.json')));
  const savedState=fs.readFileSync(stateFile);const altered=JSON.parse(savedState);altered.threadId=foreignThread;fs.writeFileSync(stateFile,JSON.stringify(altered));
  const wrongScope=await run('Must not retarget another project thread\n',1);fs.writeFileSync(stateFile,savedState);
  check('foreign thread IDs are rejected before resumption can retarget their cwd',()=>{
    assert.equal(wrongScope.sent.length,0);assert.equal(JSON.parse(wrongScope.stderr).error.code,'codex_scope_mismatch');
  });
  const db = new DatabaseSync(path.join(data,'swapctx.db'));db.exec('UPDATE memory_metadata SET expires_at=1 WHERE state=\'active\' AND memory_id IN(SELECT id FROM memories WHERE kind=\'requirement\')');db.close();
  const failed = await run('Should never reach provider\n',1);
  check('incomplete requirements refuse before a provider request and leave context data unchanged',() => {
    assert.equal(failed.sent.length,0);assert.equal(JSON.parse(failed.stderr).error.code,'mandatory_context_incomplete');assert.ok(!fs.readdirSync(path.join(data,'codex')).some(n => n.endsWith('.lock')));
  });
  console.log(`${checks} native lifecycle checks passed (synthetic provider, model behavior unverified)`);
})().catch(e => {if (serverFailure) console.error('Fixture failure',serverFailure);console.error(e);process.exitCode = 1;}).finally(() => {
  server.close();assert.equal(path.dirname(root),os.tmpdir());fs.rmSync(root,{recursive:true,force:true});
});
