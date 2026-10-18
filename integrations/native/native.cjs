'use strict';
// Read-only context handoff. Native providers own their UI, auth and history.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawn,spawnSync} = require('node:child_process');
const PACKET_BYTES = 7168, CONTEXT_BYTES = 8192;
class Failure extends Error { constructor(code) {super(code);this.code=code;} }
function requireThat(value, code) {if (!value) throw new Failure(code);}
function identity(value) {
  requireThat(typeof value === 'string' && path.isAbsolute(value),'invalid_scope');
  let result=fs.realpathSync.native(value);
  if (process.platform === 'win32') result=result.replace(/^\\\\\?\\UNC\\/i,'\\\\').replace(/^\\\\\?\\/,'').toLowerCase();
  return path.normalize(result);
}
function secret(text) {
  return /-----begin[\s\S]*private key-----/i.test(text)
    || /(?:api[_-]?key|password|passwd|client_secret|access_token|secret_key)[ \t"']*[:=][ \t"']*[^\r\n]{4}/i.test(text)
    || text.split(/[^A-Za-z0-9_-]/).some(word=>/^(?:AKIA|ASIA)[A-Z0-9]{16}$/.test(word)
      || (word.startsWith('sk-') && word.length>=24)
      || (/^(?:ghp_|gho_|github_pat_)/.test(word) && word.length>=32));
}
function validatePacket(raw, project, now=Math.floor(Date.now()/1000)) {
  // The protocol counts JSON bytes; CLI stdout adds an LF (CRLF is accepted
  // for compatible transports). Neither ending is part of the context budget.
  requireThat(Buffer.byteLength(raw.replace(/\r?\n$/,''))<=PACKET_BYTES,'context_budget_exceeded');
  let p;try {p=JSON.parse(raw);} catch {throw new Failure('context_json_invalid');}
  // JSON.parse keeps only the last duplicate member, whereas we forward the
  // original packet. Reject ambiguity before inspecting or emitting any fields.
  // Syntax is already validated above; quoted tokens consume their own braces.
  const objects=[];
  for(const token of raw.matchAll(/"(?:\\.|[^"\\])*"|[{}\[\]]/g)) {
    const text=token[0];
    if(text==='{' || text==='[') objects.push(text==='{'?new Set():null);
    else if(text==='}' || text===']') objects.pop();
    else if(/^\s*:/.test(raw.slice(token.index+text.length))) {
      const keys=objects.at(-1),key=JSON.parse(text);
      requireThat(!keys.has(key),'context_duplicate_key');keys.add(key);
    }
  }
  requireThat(p?.protocol==='swapctx.context' && p.version===1 && p.ok===true && p.schema_version===3,'context_protocol_invalid');
  requireThat(identity(p.project?.root)===identity(project),'context_scope_mismatch');
  requireThat(Number.isSafeInteger(p.generated_unix) && Math.abs(now-p.generated_unix)<=60,'context_snapshot_expired');
  requireThat(p.budget?.max_bytes===PACKET_BYTES && p.budget.bytes_used===Buffer.byteLength(raw.trimEnd())
    && p.budget.max_records===100 && p.budget.unit==='utf8_json_bytes','context_budget_invalid');
  requireThat(['fresh_only','conflicts_excluded','unknown_provenance_excluded','secret_like_records_excluded'].every(k=>p.policy?.[k]===true),'context_policy_invalid');
  requireThat(p.policy.ranking==='requirements_then_term_matches_then_recency' && p.policy.scan_limit===5000,'context_policy_invalid');
  requireThat(p.coverage?.mandatory_context_complete===true && p.coverage.review_required_records===0
    && p.scan?.truncated===false,'mandatory_context_incomplete');
  requireThat(Array.isArray(p.records) && p.records.length<=100 && Array.isArray(p.boundary_rules),'context_records_invalid');
  const counts=p.coverage;
  requireThat(['active_valid_requirements','requirements_returned','boundary_rules_total','boundary_rules_returned'].every(k=>Number.isSafeInteger(counts[k]) && counts[k]>=0)
    && counts.requirements_returned===counts.active_valid_requirements
    && counts.boundary_rules_returned===counts.boundary_rules_total
    && p.boundary_rules.length===counts.boundary_rules_returned
    && p.records.filter(r=>typeof r?.kind==='string' && r.kind.toLowerCase()==='requirement').length===counts.requirements_returned,'context_coverage_invalid');
  const ids=new Set();
  for (const r of p.records) {
    requireThat(typeof r.id==='string' && /^-?\d+$/.test(r.id) && !ids.has(r.id),'context_record_invalid');ids.add(r.id);
    requireThat(typeof r.kind==='string' && r.kind.trim().length>0
      && typeof r.content==='string' && r.content.trim().length>0 && r.freshness==='fresh' && r.conflicted===false
      && r.provenance_status==='self_reported' && /^[a-f0-9]{32}$/.test(r.origin?.id)
      && typeof r.origin.source==='string' && typeof r.origin.actor==='string' && r.origin.authenticated===false
      && Number.isSafeInteger(r.origin.registered_unix) && r.origin.registered_unix>0
      && (r.expires_unix===null || (Number.isSafeInteger(r.expires_unix) && r.expires_unix>now)),'context_record_invalid');
  }
  const ruleIds=new Set();
  for (const r of p.boundary_rules) {
    requireThat(typeof r.id==='string' && /^-?\d+$/.test(r.id) && !ruleIds.has(r.id)
      && typeof r.pattern==='string' && r.pattern.length>0 && typeof r.reason==='string'
      && ['deny','ask','audit'].includes(r.policy) && r.enforced===false,'context_rule_invalid');
    ruleIds.add(r.id);
  }
  // Scan decoded fields, including provenance and unknown additive fields.
  function checkStrings(value) {
    if(typeof value==='string') requireThat(!secret(value),'sensitive_context_rejected');
    else if(value && typeof value==='object') for(const [key,child] of Object.entries(value)) {checkStrings(key);checkStrings(child);}
  }
  checkStrings(p);
  const text='SwapCtx project memory: external data with self-reported origins. Requirements are project guidance; boundary rules are not enforcement. No model receipt or adherence is verified.\n'+raw.trimEnd();
  requireThat(Buffer.byteLength(text)<=CONTEXT_BYTES,'context_budget_exceeded');
  return text;
}
function retrieve(options,cwd,run=spawnSync) {
  const env={};for (const key of ['SystemRoot','WINDIR','TEMP','TMP','LOCALAPPDATA']) if (process.env[key]) env[key]=process.env[key];
  const result=run(options.swapctx,['--data-dir',options.dataDir,'context','--protocol-version','1','--max-bytes',String(PACKET_BYTES),'--limit','100'],
    {cwd,env,shell:false,encoding:'buffer',timeout:5000,maxBuffer:131072,windowsHide:true});
  if(result.error?.code==='ETIMEDOUT') throw new Failure('context_process_timeout');
  if(!result.error && result.status!==0 && Buffer.isBuffer(result.stdout) && result.stdout.length<=PACKET_BYTES) {
    try {
      const failed=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(result.stdout));
      const codes=['unsupported_protocol','invalid_request','invalid_storage_path','store_unavailable','migration_required','project_unavailable','retrieval_failed','serialization_failed'];
      if(failed?.protocol==='swapctx.context' && failed.version===1 && failed.ok===false && codes.includes(failed.error?.code)) throw new Failure('context_'+failed.error.code);
    } catch(error) {if(error instanceof Failure) throw error;}
  }
  requireThat(!result.error && result.status===0,'context_process_failed');
  requireThat(Buffer.isBuffer(result.stdout) && result.stdout.length<=PACKET_BYTES+2,'context_budget_exceeded');
  let raw;try {raw=new TextDecoder('utf-8',{fatal:true}).decode(result.stdout);} catch {throw new Failure('context_utf8_invalid');}
  return validatePacket(raw,options.project);
}
function eventContext(options,event,run) {
  requireThat(event?.hook_event_name==='SessionStart' && ['startup','resume','clear','compact','fork'].includes(event.source),'unsupported_hook_event');
  const root=identity(options.project),cwd=identity(event.cwd),relative=path.relative(root,cwd);
  requireThat(relative!=='..' && !relative.startsWith('..'+path.sep) && !path.isAbsolute(relative),'event_outside_project');
  // Retrieval from event cwd also catches a separately registered nested project.
  return {hookSpecificOutput:{hookEventName:'SessionStart',additionalContext:retrieve(options,event.cwd,run)}};
}
function readConfig(file) {
  const stat=fs.lstatSync(file);requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size<=16384,'invalid_adapter_config');
  const value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(fs.readFileSync(file)));
  requireThat(value.version===1 && ['codex','claude'].includes(value.provider),'invalid_adapter_config');
  for (const key of ['swapctx','dataDir','project']) identity(value[key]);
  requireThat(path.dirname(value.receipt)===path.dirname(file),'invalid_adapter_config');
  return value;
}
function receipt(options,source,ok,code) {
  // No records, prompts, transcript paths, session identifiers or provider output.
  if (fs.existsSync(options.receipt)) {
    const stat=fs.lstatSync(options.receipt);requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size<262144,'invalid_receipt');
  }
  fs.appendFileSync(options.receipt,JSON.stringify({source,ok,code})+'\n',{mode:0o600});
}
async function writeOutput(output,value) {
  let onError;
  try {
    await new Promise((resolve,reject)=>{
      onError=()=>reject(new Failure('hook_output_failed'));
      output.once('error',onError);
      output.write(JSON.stringify(value)+'\n',error=>error?onError():resolve());
    });
  } catch {throw new Failure('hook_output_failed');}
  finally {output.off('error',onError);}
}
async function hook(file,input=process.stdin,output=process.stdout) {
  let options,event;
  try {
    options=readConfig(file);
    const bytes=await new Promise((resolve,reject)=>{
      const chunks=[];let count=0;
      const timer=setTimeout(()=>{input.destroy();reject(new Failure('hook_input_timeout'));},3000);
      const fail=code=>{clearTimeout(timer);input.destroy();reject(new Failure(code));};
      input.on('data',chunk=>{count+=chunk.length;if(count>32768) fail('hook_input_too_large');else chunks.push(chunk);});
      input.once('error',()=>fail('hook_input_failed'));
      input.once('end',()=>{clearTimeout(timer);resolve(Buffer.concat(chunks));});
    });
    try {event=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));} catch {throw new Failure('hook_input_invalid');}
    const result=eventContext(options,event);
    // A local preparation record proves execution, never provider receipt.
    // Record it first so a metadata failure still withholds all context.
    receipt(options,event.source,true,'context_prepared');
    await writeOutput(output,result);
  } catch(error) {
    const code=error instanceof Failure?error.code:'adapter_failed';
    try {if(options) receipt(options,['startup','resume','clear','compact','fork'].includes(event?.source)?event.source:'invalid',false,code);} catch { /* Never emit memory after a receipt failure. */ }
    // A broken pipe may have accepted partial output. Do not retry it, report
    // success, or claim to know what the provider received.
    if(code==='hook_output_failed') throw error;
    // Context-only advisory failure. Never pretend this blocks provider actions.
    await writeOutput(output,{systemMessage:'SwapCtx context unavailable ('+code+'); no memory was supplied. Run swapctx context and swapctx doctor.'});
    process.stderr.write('SwapCtx: '+code+'; context was not supplied.\n');
  }
}
function probe(provider,exe,run=spawnSync) {
  const call=args=>run(exe,args,{shell:false,encoding:'utf8',timeout:5000,maxBuffer:65536,windowsHide:true});
  const version=call(['--version']);
  if (version.error || version.status!==0) return false;
  const help=call(['--help']);if (help.error || help.status!==0) return false;
  if(provider==='claude') {
    const match=/^2\.(\d+)\.(\d+)\s+\(Claude Code\)\s*$/.exec(version.stdout.trim());
    // Conservative SwapCtx compatibility floor for the current exec-form
    // contract; not a claim about when Anthropic introduced that interface.
    return !!match && (Number(match[1])>1 || (Number(match[1])===1 && Number(match[2])>=296)) && /--plugin-dir\b/.test(help.stdout);
  }
  // Documentation gives no trustworthy minimum version. Require the modern
  // hook trust capability, never bypass it, and still check actual callbacks.
  return /^codex-cli \d+\.\d+\.\d+\s*$/.test(version.stdout.trim())
    && /--dangerously-bypass-hook-trust\b/.test(help.stdout) && /--config\b/.test(help.stdout);
}
function bashQuote(value) {return "'"+value.replace(/\\/g,'/').replace(/'/g,"'\"'\"'")+"'";}
function psQuote(value) {return "'"+value.replace(/'/g,"''")+"'";}
function hookCommands(script,config) {
  const args=[process.execPath,script,'hook',config];
  const command=args.map(bashQuote).join(' ');
  const ps='& '+args.map(psQuote).join(' ')+'; exit $LASTEXITCODE';
  return {command,commandWindows:'powershell.exe -NoProfile -NonInteractive -EncodedCommand '+Buffer.from(ps,'utf16le').toString('base64')};
}
function toml(value) {
  if(typeof value==='string') return JSON.stringify(value);
  if(typeof value==='number') return String(value);
  if(Array.isArray(value)) return '['+value.map(toml).join(',')+']';
  return '{'+Object.entries(value).map(([key,v])=>key+'='+toml(v)).join(',')+'}';
}
function configure(options,directory,script) {
  const config=path.join(directory,'adapter.json');
  const settings={...options,version:1,receipt:path.join(directory,'receipt.jsonl')};
  fs.writeFileSync(config,JSON.stringify(settings),{flag:'wx',mode:0o600});
  const commands=hookCommands(script,config);
  const handler={type:'command',command:commands.command,timeout:10};
  const group={matcher:'^(startup|resume|clear|compact|fork)$',hooks:[handler]};
  if(options.provider==='claude') {
    // Official exec-form hooks avoid Git Bash/PowerShell quoting differences.
    handler.command=process.execPath;
    handler.args=[script,'hook',config];
    fs.mkdirSync(path.join(directory,'.claude-plugin'));
    fs.mkdirSync(path.join(directory,'hooks'));
    fs.writeFileSync(path.join(directory,'.claude-plugin','plugin.json'),JSON.stringify({name:'swapctx-context',version:'0.1.0',description:'Read-only project memory'}),{flag:'wx'});
    fs.writeFileSync(path.join(directory,'hooks','hooks.json'),JSON.stringify({hooks:{SessionStart:[group]}}),{flag:'wx'});
    return {args:['--plugin-dir',directory],receipt:settings.receipt};
  }
  handler.command_windows=commands.commandWindows;
  handler.additionalContextLimit=0; // Complete independently byte-bounded packet.
  return {args:['-c','hooks.SessionStart='+toml([group])],receipt:settings.receipt};
}
async function runProvider(exe,args,dependencies={}) {
  const child=(dependencies.spawn||spawn)(exe,args,{stdio:'inherit',shell:false});
  const forward=signal=>{child.kill(signal);};
  const interrupt=()=>forward('SIGINT'),terminate=()=>forward('SIGTERM');
  process.on('SIGINT',interrupt);process.on('SIGTERM',terminate);
  try {
    return await new Promise((resolve,reject)=>{child.once('error',()=>reject(new Failure('native_provider_launch_failed')));child.once('close',(code)=>resolve(Number.isInteger(code)&&code>=0&&code<=255?code:1));});
  } finally {process.off('SIGINT',interrupt);process.off('SIGTERM',terminate);}
}
async function launch(options,args,dependencies={}) {
  const warn=dependencies.warn||((text)=>process.stderr.write('SwapCtx: '+text+'\n'));
  const run=dependencies.runProvider||runProvider;
  if (!(dependencies.probe||probe)(options.provider,options.executable)) {
    warn('provider did not pass the native-hook compatibility probe; launching its original UI without SwapCtx context.');
    return run(options.executable,args);
  }
  // Prevent provider flags from relocating the initial workspace or silently
  // replacing the integration. Ordinary launching accepts all original flags.
  requireThat(args.every(arg=>! /^(?:-C|-w|--(?:cd|cwd|config|worktree)(?:=|$))/.test(arg)
    && !(options.provider==='codex' && /^-c/.test(arg))),'context_launch_argument_conflict');
  const parent=identity(dependencies.tempRoot||os.tmpdir());
  const directory=fs.mkdtempSync(path.join(parent,'swapctx-hooks-'));
  const owned=fs.lstatSync(directory);
  let primary;
  try {
    // Validate the first snapshot before opening a provider, without persisting it.
    (dependencies.retrieve||retrieve)(options,process.cwd());
    const setup=configure(options,directory,__filename);
    warn('context hooks requested; provider policy/trust may skip them. Adapter output does not prove model receipt or adherence.');
    if(options.provider==='codex') warn('review SwapCtx in /hooks; trust is never bypassed. After first approval, /compact requests a fresh SessionStart callback.');
    const code=await run(options.executable,[...setup.args,...args]);
    if(code!==0) primary=new Failure('native_provider_exit');
    if(!fs.existsSync(setup.receipt)) warn('no hook callback was recorded; context delivery is unverified (hooks may be missing, disabled or untrusted).');
    else {
      try {
        const stat=fs.lstatSync(setup.receipt);
        requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size<=262144,'invalid_receipt');
        const entries=fs.readFileSync(setup.receipt,'utf8').trim().split('\n').map(line=>JSON.parse(line));
        requireThat(entries.every(e=>typeof e.ok==='boolean' && typeof e.code==='string' && /^[a-z_]+$/.test(e.code)),'invalid_receipt');
        warn('hook adapter results: '+entries.filter(e=>e.ok).length+' contexts prepared, '+entries.filter(e=>!e.ok).length+' failures. Provider receipt and model adherence remain unknown.');
      } catch {warn('hook outcome metadata is invalid; context delivery remains unverified.');}
    }
    return code;
  } catch(error) {primary=error;throw error;}
  finally {
    try {
      const stat=fs.lstatSync(directory);
      requireThat(!stat.isSymbolicLink() && stat.dev===owned.dev && stat.ino===owned.ino
        && identity(path.dirname(directory))===parent,'unsafe_adapter_cleanup');
      fs.rmSync(directory,{recursive:true});
    } catch(error) {
      warn('temporary adapter cleanup failed; inspect swapctx-hooks-* in your temporary directory. No cleanup was attempted outside the owned directory.');
      if(!primary) throw new Failure('adapter_cleanup_failed');
    }
  }
}
function optionsFrom(args) {
  const options={};let i=0;
  for(;i<args.length && args[i]!=='--';i+=2) {
    const name={'--provider':'provider','--swapctx':'swapctx','--data-dir':'dataDir','--project':'project','--executable':'executable'}[args[i]];
    requireThat(name && args[i+1] && !Object.hasOwn(options,name),'invalid_adapter_arguments');options[name]=args[i+1];
  }
  requireThat(['codex','claude'].includes(options.provider) && typeof options.executable==='string','invalid_adapter_arguments');
  for(const key of ['swapctx','project']) identity(options[key]);
  requireThat(typeof options.dataDir==='string' && path.isAbsolute(options.dataDir),'invalid_adapter_arguments');
  return {options,args:args.slice(i+1)};
}
module.exports={validatePacket,retrieve,eventContext,hook,probe,hookCommands,toml,configure,runProvider,launch,optionsFrom,PACKET_BYTES};
if(require.main===module) (async()=>{
  requireThat(Number(process.versions.node.split('.')[0])>=24,'node_24_required');
  const [mode,...args]=process.argv.slice(2);
  if(mode==='hook' && args.length===1) await hook(args[0]);
  else if(mode==='launch') {const parsed=optionsFrom(args);process.exitCode=await launch(parsed.options,parsed.args);}
  else throw new Failure('invalid_adapter_arguments');
})().catch(error=>{process.stderr.write('SwapCtx: '+(error instanceof Failure?error.code:'native_adapter_failed')+'; no provider state was changed by SwapCtx.\n');process.exitCode=1;});
