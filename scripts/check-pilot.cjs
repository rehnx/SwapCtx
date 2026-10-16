'use strict';
// Opt-in real-provider pilot. No fixture provider and no existing SwapCtx store.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawn,spawnSync} = require('node:child_process');
const {AppServer,Failure,SUPPORTED} = require('../integrations/codex/session.cjs');

const SENTINEL = 'Original public contract. Preserve these exact bytes.\r\n';
const TASKS = [
  'Implement the current saved project requirements in lib.cjs. Run Node checks. Do not add dependencies.',
  'Add an exported wordCount(value) function: count the normalized words, returning zero for an empty string. Preserve all saved requirements and existing APIs. Run checks.',
  'Add exported words(value): return the normalized words as an array, or [] for empty input. Preserve existing behavior and saved requirements. Run checks.',
  'Add exported hasWord(value, word): test exact normalized word membership, case insensitive. Reject non-string arguments with TypeError. Preserve existing behavior and current saved requirements. Run checks.',
  'Add exported initials(value): return the first character of each normalized word joined without spaces. Preserve existing behavior and current saved requirements. Run checks.',
  'Add exported reverseWords(value): return normalized words in reverse order joined with one space. Preserve existing behavior and current saved requirements. Run checks.'
];
function check(condition, code) {if (!condition) throw new Failure(code);}
function hash(value) {return crypto.createHash('sha256').update(value).digest('hex');}
function localEnv() {
  const env = {};
  for (const key of ['PATH','SystemRoot','WINDIR','COMSPEC','TEMP','TMP','LOCALAPPDATA']) if (process.env[key]) env[key] = process.env[key];
  return env;
}
// Independent executable checks, supplied to a bounded child process rather
// than trusting assistant prose or a test file the assistant can rewrite.
function oracleSource(nonce, stage) {
  return `const assert=require('node:assert/strict');const m=require(process.argv[1]);
assert.equal(m.formatLabel('  HeLLo \\t WORLD  '),'hello world');
assert.equal(m.formatLabel(''),'');assert.equal(m.formatLabel('MiXeD'),'mixed');
for(const value of [null,undefined,42,{},[]])assert.throws(()=>m.formatLabel(value),TypeError);
assert.equal(m.releaseTag(),${JSON.stringify(nonce)});
${stage>=1?"assert.equal(m.wordCount(' a  b '),2);assert.equal(m.wordCount(''),0);assert.throws(()=>m.wordCount(null),TypeError);":''}
${stage>=2?"assert.deepEqual(m.words(' A  b '),['a','b']);assert.deepEqual(m.words(''),[]);assert.throws(()=>m.words(3),TypeError);":''}
${stage>=3?"assert.equal(m.hasWord(' A  b ','B'),true);assert.equal(m.hasWord('alphabet','alpha'),false);assert.equal(m.hasWord('','a'),false);assert.throws(()=>m.hasWord('a',3),TypeError);":''}
${stage>=4?"assert.equal(m.initials(' Hello  WORLD '),'hw');assert.equal(m.initials(''),'');assert.throws(()=>m.initials(null),TypeError);":''}
${stage>=5?"assert.equal(m.reverseWords(' Hello  WORLD '),'world hello');assert.equal(m.reverseWords(''),'');assert.throws(()=>m.reverseWords(null),TypeError);":''}
process.stdout.write('swapctx-pilot-oracle-completed\\n');
`;
}
function verifyWorkspace(project, nonce, stage) {
  const names = fs.readdirSync(project).sort();
  check(JSON.stringify(names)===JSON.stringify(['contract.txt','lib.cjs']),'unexpected_workspace_paths');
  for (const name of names) {
    const stat = fs.lstatSync(path.join(project,name));
    check(stat.isFile()&&!stat.isSymbolicLink()&&stat.nlink===1&&stat.size<=1024*1024,'unsafe_workspace_artifact');
  }
  check(fs.readFileSync(path.join(project,'contract.txt'),'utf8')===SENTINEL,'saved_requirement_violated');
  const result = spawnSync(process.execPath,['--permission','--allow-fs-read='+project,'-e',oracleSource(nonce,stage),path.join(project,'lib.cjs')],{cwd:project,env:localEnv(),encoding:'utf8',timeout:5000,maxBuffer:65536,windowsHide:true});
  check(!result.error&&result.status===0&&result.stdout==='swapctx-pilot-oracle-completed\n','workspace_behavior_failed');
  return {implementationHash:hash(fs.readFileSync(path.join(project,'lib.cjs'))),contractHash:hash(SENTINEL)};
}
function turnEvidence(events, previousThread, compact = false) {
  const ready = events.find(e=>e.event==='session_ready'),selected = events.find(e=>e.event==='context_selected');
  check(Boolean(ready&&selected&&events.some(e=>e.event==='turn_completed'&&e.status==='completed'&&e.operation===(compact?'compact':'turn'))),'incomplete_turn_evidence');
  check(previousThread?ready.threadId===previousThread&&ready.resumed===true:ready.resumed===false,'session_continuity_failed');
  if (compact) check(events.some(e=>e.event==='compaction_completed'),'compaction_not_observed');
  const metrics = events.find(e=>e.event==='operation_metrics');
  check(metrics?.success===true&&['contextMs','sessionMs','turnMs','totalMs'].every(k=>Number.isSafeInteger(metrics[k])&&metrics[k]>=0),'operation_metrics_missing');
  return {threadId:ready.threadId,contextHash:selected.hash,contextBytes:selected.bytes,metrics,toolStarts:events.filter(e=>e.event==='tool_started').length,toolFailures:events.filter(e=>e.event==='tool_completed'&&(e.status==='failed'||e.status==='declined'||(Number.isSafeInteger(e.exitCode)&&e.exitCode!==0))).length};
}
function resultStatus(error) {return error?['authentication_required','provider_authentication_unverified','codex_startup_permission_denied'].includes(error.code)?'blocked':'failed':'passed';}
function requireAuthenticated(account) {
  check(typeof account?.requiresOpenaiAuth==='boolean'&&Object.hasOwn(account,'account'),'codex_protocol_invalid');
  // A no-auth fixture/custom provider cannot count as this authenticated pilot.
  check(account.requiresOpenaiAuth,'provider_authentication_unverified');
  check(Boolean(account.account),'authentication_required');
  check(['apiKey','chatgpt'].includes(account.account.type),'provider_authentication_unverified');
}

async function main(argv = process.argv.slice(2)) {
  check(argv.length>=2&&argv.length<=3,'invalid_arguments');
  const [codex,exe,output] = argv.map(value=>path.resolve(value));
  if (output) check(!fs.existsSync(output)&&fs.statSync(path.dirname(output)).isDirectory(),'report_destination_exists_or_invalid');
  const root = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-real-pilot-'));
  const project = path.join(root,'project ü space'),data = path.join(root,'data');fs.mkdirSync(project);
  const report = {protocol:'swapctx.codex-pilot',version:1,date:new Date().toISOString(),codexVersion:SUPPORTED,nodeVersion:process.versions.node,disposableProject:true,provider:'configured_codex_provider',status:'running',checks:[],operations:[],modelEvaluationStarted:false,codingWorkflowStarted:false,contextUseObserved:false,workspaceAdherenceObserved:false,guaranteedAdherence:false,automaticCompactionVerified:false,longRunningSessionVerified:false};
  let failure,active,activeApi,interrupted = false;
  const interrupt = ()=>{interrupted = true;activeApi?.abort('pilot_interrupted');stopChild();};
  function stopChild() {
    if (!active||!Number.isSafeInteger(active.pid)||active.exitCode!==null||active.signalCode!==null) return;
    // Only the still-running process tree created by this runner is targeted.
    if (process.platform==='win32') spawnSync('taskkill.exe',['/PID',String(active.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});
    else {try {process.kill(-active.pid,'SIGKILL');} catch {active.kill();}}
  }
  function cli(args) {
    check(!interrupted,'pilot_interrupted');
    const r = spawnSync(exe,['--data-dir',data,...args],{cwd:project,encoding:'utf8',timeout:20000,maxBuffer:65536,windowsHide:true});
    check(!r.error&&r.status===0,'pilot_cli_failed');return r;
  }
  async function command(executable,args,input) {
    check(!interrupted,'pilot_interrupted');
    const child = spawn(executable,args,{cwd:project,stdio:['pipe','pipe','pipe'],windowsHide:true,detached:process.platform!=='win32'});active = child;
    let stdout = '',stderr = '',tooLarge = false,timedOut = false;
    for (const [stream,key] of [[child.stdout,'out'],[child.stderr,'err']]) stream.on('data',chunk=>{
      if (key==='out') stdout+=chunk;else stderr+=chunk;
      if (Buffer.byteLength(stdout)+Buffer.byteLength(stderr)>2*1024*1024) {tooLarge=true;stopChild();}
    });
    child.stdin.on('error',()=>{});child.stdin.end(input);
    const code = await new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{timedOut=true;stopChild();},660000);
      child.once('error',()=>{clearTimeout(timer);reject(new Failure('pilot_launch_failed'));});
      child.once('close',value=>{clearTimeout(timer);resolve(value);});
    });active = null;
    check(!interrupted,'pilot_interrupted');
    check(!timedOut&&!tooLarge,timedOut?'pilot_operation_timeout':'pilot_output_oversized');
    return {code,stdout,stderr};
  }
  async function managed(prompt) {
    const {code,stdout,stderr} = await command(exe,['--data-dir',data,'codex','--json','run'],prompt+'\n');
    let events;
    try {events=stdout.trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));}
    catch {throw new Failure('pilot_event_protocol_invalid');}
    check(events.every(e=>e.protocol==='swapctx.codex'&&e.version===1),'pilot_event_protocol_invalid');
    if (code!==0) {
      const metrics=events.find(e=>e.event==='operation_metrics');
      if(metrics)report.operations.push({stage:'failed_operation',metrics});
      // Only forward this adapter's bounded, identifier-only error code.
      let code = 'pilot_managed_operation_failed';
      try {const error=JSON.parse(stderr);if(error.protocol==='swapctx.codex'&&/^[a-z_]{1,80}$/.test(error.error?.code))code=error.error.code;} catch {}
      throw new Failure(code);
    }
    return events;
  }
  process.on('SIGINT',interrupt);process.on('SIGTERM',interrupt);
  try {
    const version=spawnSync(codex,['--version'],{encoding:'utf8',timeout:5000,maxBuffer:8192,windowsHide:true});
    check(!version.error&&version.status===0&&version.stdout.trim()==='codex-cli '+SUPPORTED,'unsupported_codex_version');
    activeApi = new AppServer(codex,project);
    try {
      await activeApi.initialize();const account=await activeApi.call('account/read',{refreshToken:false});
      requireAuthenticated(account);
    } finally {await activeApi.close();activeApi=null;}
    // Reuse the existing no-context control and unpredictable-nonce evaluation.
    // No coding result is credited unless this real-provider control passes too.
    report.modelEvaluationStarted=true;
    const control = await command(process.execPath,[path.join(__dirname,'check-model-context.cjs'),codex,exe],'');
    let controlReport;try {controlReport=JSON.parse(control.stdout);} catch {throw new Failure('control_report_invalid');}
    if(control.code===2&&controlReport.error?.code==='authentication_required')throw new Failure('authentication_required');
    check(control.code===0,'context_use_control_failed');
    check(controlReport.contextUseObserved===true&&controlReport.ok===true,'context_use_control_failed');
    report.checks.push({name:'real_provider_context_use_control',status:'passed'});report.contextUseObserved=true;
    fs.writeFileSync(path.join(project,'contract.txt'),SENTINEL);
    fs.writeFileSync(path.join(project,'lib.cjs'),"'use strict';\nexports.formatLabel=value=>value.trim();\nexports.releaseTag=()=> 'UNSET';\n");
    cli(['init']);
    cli(['memory','add','Preserve contract.txt byte-for-byte. Edit only lib.cjs. Keep CommonJS exports formatLabel(value) and releaseTag(). formatLabel rejects non-strings with TypeError, trims and collapses whitespace to single spaces, and lowercases text. Do not add dependencies, read SwapCtx data files or inspect credentials.','--kind','requirement']);
    let nonce=crypto.randomBytes(16).toString('hex');
    cli(['memory','add','releaseTag() must return exactly '+nonce,'--kind','requirement','--key','release-tag']);
    cli(['codex','setup','--codex',codex,'--workspace-write']);cli(['codex','doctor']);
    let thread,previousHash;
    report.codingWorkflowStarted=true;
    for(let stage=0;stage<TASKS.length;stage++) {
      if(stage===3) {
        const compactEvents=await managed('/compact');
        const evidence=turnEvidence(compactEvents,thread,true);report.operations.push({stage:'manual_compaction',...evidence});
        report.checks.push({name:'native_manual_compaction',status:'passed'});
        nonce=crypto.randomBytes(16).toString('hex');cli(['memory','supersede','2','releaseTag() must return exactly '+nonce]);
      }
      const prompt=TASKS[stage]+' New text APIs must reject non-string arguments with TypeError. Use only the disposable project files and delivered context; do not inspect SwapCtx data directories, credentials or other projects. Edit only lib.cjs.';
      const evidence=turnEvidence(await managed(prompt),thread);thread=evidence.threadId;
      if(stage===3)check(evidence.contextHash!==previousHash,'context_refresh_not_observed');
      const workspace=verifyWorkspace(project,nonce,stage);
      previousHash=evidence.contextHash;report.operations.push({stage,...evidence,...workspace});
      report.checks.push({name:stage===0?'startup_workspace_requirements':stage===3?'post_compaction_superseded_requirement':'resumed_coding_stage_'+stage,status:'passed'});
    }
    report.workspaceAdherenceObserved=true;
  } catch(e) {failure=interrupted?new Failure('pilot_interrupted'):e instanceof Failure?e:new Failure('pilot_local_failure');report.error={code:failure.code};}
  finally {
    stopChild();activeApi?.abort('pilot_finished');process.removeListener('SIGINT',interrupt);process.removeListener('SIGTERM',interrupt);
    // Remove only this runner's known disposable directory. Refuse a replaced root.
    try {check(path.dirname(root)===os.tmpdir()&&!fs.lstatSync(root).isSymbolicLink(),'unsafe_pilot_cleanup');fs.rmSync(root,{recursive:true,force:true});report.cleanup='completed';}
    catch {report.cleanup='failed';if(!failure){failure=new Failure('pilot_cleanup_failed');report.error={code:failure.code};}}
  }
  report.status=resultStatus(failure);report.ok=!failure;
  // Do not persist raw assistant output, commands, credentials or source content.
  const json=JSON.stringify(report,null,2)+'\n';
  if(output)fs.writeFileSync(output,json,{flag:'wx',mode:0o600});
  process.stdout.write(json);return failure?report.status==='blocked'?2:1:0;
}
module.exports={SENTINEL,TASKS,oracleSource,verifyWorkspace,turnEvidence,resultStatus,requireAuthenticated,main};
if(require.main===module)main().then(code=>{process.exitCode=code;}).catch(e=>{
  console.error(JSON.stringify({protocol:'swapctx.codex-pilot',version:1,ok:false,status:'failed',error:{code:e instanceof Failure?e.code:'pilot_local_failure'}}));process.exitCode=1;
});
