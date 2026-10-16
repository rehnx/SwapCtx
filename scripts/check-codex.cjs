// Real Codex CLI, local HTTP fixture only. No OpenAI credentials or model inference.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const {spawn,spawnSync} = require('node:child_process');
const {DatabaseSync} = require('node:sqlite');
const codex = path.resolve(process.argv[2]);
const swapctx = path.resolve(process.argv[3] || 'target/debug/swapctx.exe');
const adapter = path.resolve(__dirname,'../integrations/codex/session-context.cjs');
const launcher = path.resolve(__dirname,'../integrations/codex/exec-context.cjs');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-codex-'));
const project = path.join(tmp,'project space ü'), data = path.join(tmp,'data');
fs.mkdirSync(project);
const marker = 'SWAPCTX_VERIFIED_REQUIREMENT_49F20';
const secret = 'password=NeverTransmitSecret193847';
function cli(args) {const r=spawnSync(swapctx,['--data-dir',data,...args],{cwd:project,encoding:'utf8'});assert.equal(r.status,0,r.stderr);}
function quote(arg) {
  // Hook commands use a shell. The harness only accepts arguments safe to quote.
  if (/["%\r\n!`$]/.test(arg)) throw Error('Unsafe fixture command path');
  return '"'+arg+'"';
}
const command=[process.execPath,adapter,'--swapctx',swapctx,'--data-dir',data,'--project',project].map(quote).join(' ');
let requests=[];
const server=http.createServer((req,res)=>{
  let body=''; req.on('data',d=>body+=d);req.on('end',()=>{
    requests.push({url:req.url,body:JSON.parse(body)});
    const message={id:'msg_fixture',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'Fixture response.',annotations:[]}]};
    const response={id:'resp_fixture',object:'response',created_at:1,status:'completed',model:'swapctx-fixture',output:[message],usage:{input_tokens:1,output_tokens:1,total_tokens:2}};
    res.writeHead(200,{'content-type':'text/event-stream'});
    for(const event of [
      {type:'response.created',response:{...response,status:'in_progress',output:[]}},
      {type:'response.output_item.added',output_index:0,item:{...message,status:'in_progress',content:[]}},
      {type:'response.content_part.added',item_id:message.id,output_index:0,content_index:0,part:{type:'output_text',text:'',annotations:[]}},
      {type:'response.output_text.delta',item_id:message.id,output_index:0,content_index:0,delta:'Fixture response.'},
      {type:'response.output_text.done',item_id:message.id,output_index:0,content_index:0,text:'Fixture response.'},
      {type:'response.output_item.done',output_index:0,item:message},
      {type:'response.completed',response}
    ]) res.write('event: '+event.type+'\ndata: '+JSON.stringify(event)+'\n\n');
    res.end();
  });
});
async function run(name, mode) {
  const home=path.join(tmp,name);fs.mkdirSync(home);
  const port=server.address().port;
  fs.writeFileSync(path.join(home,'config.toml'),`model = "swapctx-fixture"\nmodel_provider = "fixture"\n[analytics]\nenabled = false\n[features]\nhooks = true\n[model_providers.fixture]\nname = "SwapCtx local test fixture"\nbase_url = "http://127.0.0.1:${port}/v1"\nwire_api = "responses"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n`);
  fs.writeFileSync(path.join(home,'hooks.json'),JSON.stringify({hooks:{SessionStart:[{matcher:'startup|resume|clear|compact',hooks:[{type:'command',command,timeout:10,additionalContextLimit:16384}]}]}}));
  const env={CODEX_HOME:home};for(const key of ['PATH','SystemRoot','WINDIR','USERPROFILE','COMSPEC','TEMP','TMP','LOCALAPPDATA','APPDATA']) if(process.env[key]) env[key]=process.env[key];
  if(mode==='untrusted') {
    const p=spawn(codex,['app-server'],{env,stdio:['pipe','pipe','pipe'],windowsHide:true});
    let seq=0;const pending=new Map();const lines=require('node:readline').createInterface({input:p.stdout});
    lines.on('line',line=>{const m=JSON.parse(line);if(pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id);}});
    p.stderr.resume();
    function call(method,params){return new Promise(resolve=>{const id=++seq;pending.set(id,resolve);p.stdin.write(JSON.stringify({id,method,params})+'\n');});}
    const close=new Promise(resolve=>p.once('close',resolve));
    let timer;
    try {
      await Promise.race([(async()=>{
        const init=await call('initialize',{clientInfo:{name:'swapctx_boundary_test',version:'0.1'}});assert.ok(init.result);
        p.stdin.write(JSON.stringify({method:'initialized',params:{}})+'\n');
        const hooks=await call('hooks/list',{cwds:[project]});assert.equal(hooks.result.data[0].hooks.length,1);assert.equal(hooks.result.data[0].hooks[0].trustStatus,'untrusted');
      })(),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('app-server probe timeout')),5000);})]);
    } finally {clearTimeout(timer);p.kill();await close;}
    console.log('PASS app-server initialization and hook discovery without model requests');
  }
  const args=['exec','--skip-git-repo-check','--json','--sandbox','read-only','-C',project];
  if(mode==='hook') args.push('--dangerously-bypass-hook-trust'); // Isolated, pre-vetted fixture ONLY.
  args.push('Respond with the fixture greeting. Do not use tools.');
  const start=requests.length;
  const launching=mode==='launcher';
  const child=spawn(launching?process.execPath:codex,launching?[launcher,'--swapctx',swapctx,'--codex',codex,'--data-dir',data,'--project',project]:args,{env,stdio:[launching?'pipe':'ignore','pipe','pipe'],windowsHide:true});
  if(launching) child.stdin.end('Respond with the fixture greeting. Do not use tools.');
  let stdout='',stderr='';child.stdout.on('data',d=>stdout+=d);child.stderr.on('data',d=>stderr+=d);
  const result=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{child.kill();reject(Error('Codex fixture timed out'));},30000);
    child.on('error',e=>{clearTimeout(timer);reject(e);});
    child.on('close',code=>{clearTimeout(timer);resolve({code,stdout,stderr});});
  });
  const events=stdout.trim().split('\n').filter(Boolean).map(s=>JSON.parse(s));
  const sent=requests.slice(start);
  // Only harmless fixture diagnostics are printed on failure. Never user configs/auth.
  if(process.env.SWAPCTX_TEST_DEBUG) console.log(name,result,sent.map(r=>({url:r.url,bodyBytes:JSON.stringify(r.body).length})));
  return {...result,sent,events};
}
(async()=>{
  cli(['init']);cli(['memory','add',marker,'--kind','requirement']);cli(['memory','add',secret,'--allow-sensitive']);
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const version=spawnSync(codex,['--version'],{encoding:'utf8'});assert.equal(version.status,0);console.log(version.stdout.trim());
  assert.equal(version.stdout.trim(),'codex-cli 0.162.1','This baseline includes a version-specific hook limitation; revalidate before updating it');
  const untrusted=await run('untrusted-home','untrusted');
  assert.ok(untrusted.sent.length>0,'Untrusted fixture should reach provider without running bridge');
  assert.ok(!JSON.stringify(untrusted.sent).includes(marker),'Untrusted hooks must not deliver context');
  console.log('PASS untrusted execution does not deliver hook context');
  const hook=await run('vetted-hook-home','hook');
  assert.ok(hook.sent.length>0);
  assert.ok(!JSON.stringify(hook.sent).includes(marker));
  console.log('LIMITATION confirmed: SessionStart delivery absent in tested Codex exec 0.162.1');
  const trusted=await run('launcher-home','launcher');
  assert.equal(trusted.code,0,trusted.stderr);
  assert.ok(JSON.stringify(trusted.sent).includes(marker),'Real Codex outbound input must contain requirement');
  assert.ok(!JSON.stringify(trusted.sent).includes(secret),'Secret-like memory must not reach provider');
  assert.ok(trusted.events.some(e=>e.type==='turn.completed'),'Structured completed-turn observation');
  console.log('PASS real stdin context delivery, secret withholding, structured completion');
  const db=new DatabaseSync(path.join(data,'swapctx.db'));db.exec('UPDATE memory_metadata SET expires_at=1 WHERE memory_id=1');db.close();
  const incomplete=await run('incomplete-home','launcher');
  assert.equal(incomplete.sent.length,0,'Incomplete mandatory context must stop before provider request');
  assert.ok(!incomplete.stdout.includes(marker));
  console.log('PASS incomplete mandatory context stops before provider request');
  assert.equal(incomplete.code,1);
  console.log('5 real-Codex boundary checks passed (fixture provider; no model behavior assertion)');
})().catch(e=>{console.error(e);process.exitCode=1;}).finally(()=>{server.close();assert.equal(path.dirname(tmp),os.tmpdir());fs.rmSync(tmp,{recursive:true,force:true});});
