'use strict';
// Read-only context handoff. Native providers own their UI, auth and history.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {createHash,createHmac,randomBytes} = require('node:crypto');
const {spawn,spawnSync} = require('node:child_process');
const PACKET_BYTES = 7168, CONTEXT_BYTES = 8192;
const RECEIPT_BYTES=262144;
// Fixed-size, launch-local observations. Never store event data or context here.
const STARTUP_BYTES=8, STARTUP_MAGIC='SCX1';
function startupFile(config) {return path.join(path.dirname(config),'startup.bin');}
function startupState(config) {
  const file=startupFile(config),stat=fs.lstatSync(file);
  requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size===STARTUP_BYTES,'startup_metadata_invalid');
  const fd=fs.openSync(file,'r');
  try {
    const actual=fs.fstatSync(fd),bytes=Buffer.alloc(STARTUP_BYTES+1);
    requireThat(actual.dev===stat.dev && actual.ino===stat.ino && actual.size===STARTUP_BYTES,'startup_metadata_invalid');
    requireThat(fs.readSync(fd,bytes,0,bytes.length,0)===STARTUP_BYTES,'startup_metadata_invalid');
    const value=bytes.subarray(0,STARTUP_BYTES).toString('latin1');
    requireThat(/^SCX1[01]{4}$/.test(value),'startup_metadata_invalid');
    return {bootstrap:value[4]==='1',node:value[5]==='1',config:value[6]==='1',receiptFailed:value[7]==='1'};
  } finally {fs.closeSync(fd);}
}
function markStartup(config,index) {
  let fd;
  try {
    // Only update a pre-created observation file; never create a path derived
    // from unvalidated config. Missing files retain legacy hook compatibility.
    startupState(config);
    const file=startupFile(config),before=fs.lstatSync(file);
    fd=fs.openSync(file,'r+');const actual=fs.fstatSync(fd);
    requireThat(actual.dev===before.dev && actual.ino===before.ino && actual.size===STARTUP_BYTES,'startup_metadata_invalid');
    fs.writeSync(fd,Buffer.from('1'),0,1,4+index);
  } catch(error) {
    if(error.code!=='ENOENT') process.stderr.write('SwapCtx: hook_startup_metadata_unavailable; early observations are incomplete.\n');
  } finally {if(fd!==undefined) {try {fs.closeSync(fd);} catch { /* Diagnostic only. */ }}}
}
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
function validatePacket(raw, project, now=Math.floor(Date.now()/1000),prepared=()=>{}) {
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
  prepared(p);
  return text;
}
function packetFingerprint(packet,key) {
  // Only called after validation. Exclude transport/snapshot-clock noise, but
  // retain every semantic field (including omissions, expiry and provenance).
  const value={...packet,project:{...packet.project,root:identity(packet.project.root)},budget:{...packet.budget}};
  delete value.generated_unix;delete value.budget.bytes_used;
  function canonical(v) {
    if(Array.isArray(v)) return '['+v.map(canonical).join(',')+']';
    if(v && typeof v==='object') return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+canonical(v[k])).join(',')+'}';
    return JSON.stringify(v);
  }
  return createHmac('sha256',Buffer.from(key,'hex')).update('swapctx.native.snapshot.v1\0'+canonical(value)).digest('hex');
}
function fingerprint(raw,project,key,now) {
  requireThat(typeof key==='string' && /^[a-f0-9]{64}$/.test(key),'invalid_fingerprint_key');
  let result;validatePacket(raw,project,now,p=>{result=packetFingerprint(p,key);});return result;
}
function retrieve(options,cwd,run=spawnSync,prepared) {
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
  return validatePacket(raw,options.project,undefined,prepared);
}
function eventContext(options,event,run,observe=()=>{}) {
  requireThat(event?.hook_event_name==='SessionStart' && ['startup','resume','clear','compact','fork'].includes(event.source),'unsupported_hook_event');
  const root=identity(options.project),cwd=identity(event.cwd),relative=path.relative(root,cwd);
  requireThat(relative!=='..' && !relative.startsWith('..'+path.sep) && !path.isAbsolute(relative),'event_outside_project');
  // Retrieval from event cwd also catches a separately registered nested project.
  observe('retrieval_attempted');
  const text=retrieve(options,event.cwd,run,p=>observe('context_prepared',options.fingerprintKey?packetFingerprint(p,options.fingerprintKey):undefined));
  return {hookSpecificOutput:{hookEventName:'SessionStart',additionalContext:text}};
}
function readConfig(file) {
  const stat=fs.lstatSync(file);requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size<=16384,'invalid_adapter_config');
  const value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(fs.readFileSync(file)));
  requireThat(value.version===1 && ['codex','claude'].includes(value.provider),'invalid_adapter_config');
  requireThat(value.fingerprintKey===undefined || (typeof value.fingerprintKey==='string' && /^[a-f0-9]{64}$/.test(value.fingerprintKey)),'invalid_adapter_config');
  for (const key of ['swapctx','dataDir','project']) identity(value[key]);
  requireThat(path.dirname(value.receipt)===path.dirname(file),'invalid_adapter_config');
  return value;
}
function receipt(options,entry) {
  // Short, non-waiting append lock makes the byte cap strict across callbacks.
  // Logging never gates recovery; saturation, contention or a stale crash lock
  // marks observations incomplete and leaves the current snapshot path active.
  let lock;
  try {
    lock=fs.openSync(options.receipt+'.lock','wx',0o600);
    let size=0;
    if(fs.existsSync(options.receipt)) {
      const stat=fs.lstatSync(options.receipt);
      requireThat(stat.isFile() && !stat.isSymbolicLink(),'invalid_receipt');size=stat.size;
    }
    const line=JSON.stringify(entry)+'\n';
    requireThat(size+Buffer.byteLength(line)<=RECEIPT_BYTES,'receipt_limit');
    fs.appendFileSync(options.receipt,line,{mode:0o600});return true;
  } catch {
    try {fs.writeFileSync(options.receipt+'.incomplete','',{flag:'wx',mode:0o600});} catch { /* best effort only */ }
    return false;
  } finally {
    if(lock!==undefined) {
      try {fs.closeSync(lock);} catch { /* Best-effort diagnostics must not gate context. */ }
      try {fs.unlinkSync(options.receipt+'.lock');} catch { /* Later callbacks report incomplete observations. */ }
    }
  }
}
function summarizeReceipts(file) {
  const stat=fs.lstatSync(file);
  requireThat(stat.isFile() && !stat.isSymbolicLink() && stat.size<=RECEIPT_BYTES,'invalid_receipt');
  const entries=fs.readFileSync(file,'utf8').trim().split('\n').map(line=>JSON.parse(line));
  const stages=['callback_requested','retrieval_attempted','context_prepared','output_written','context_withheld','output_failed'];
  const callbacks=new Map(),fingerprints=new Set();let prepared=0,written=0,failures=0,repeated=0;
  for(const e of entries) {
    requireThat(e.v===2 && typeof e.callback==='string' && /^[a-f0-9]{16}$/.test(e.callback) && stages.includes(e.stage)
      && ['startup','resume','clear','compact','fork','invalid'].includes(e.source)
      && (e.fingerprint===undefined || (typeof e.fingerprint==='string' && /^[a-f0-9]{64}$/.test(e.fingerprint)))
      && (e.code===undefined || (typeof e.code==='string' && /^[a-z_]{1,80}$/.test(e.code))),'invalid_receipt');
    callbacks.set(e.callback,e.stage);
    if(e.stage==='context_prepared') {prepared++;if(e.fingerprint) {if(fingerprints.has(e.fingerprint)) repeated++;fingerprints.add(e.fingerprint);}}
    if(e.stage==='output_written') written++;
    if(e.stage==='context_withheld' || e.stage==='output_failed') failures++;
  }
  const unfinished=[...callbacks.values()].filter(s=>!['output_written','context_withheld','output_failed'].includes(s)).length;
  return {callbacks:callbacks.size,prepared,written,failures,repeated,unfinished};
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
async function hook(file,input=process.stdin,output=process.stdout,expectedScope) {
  markStartup(file,1); // Node entry, before config loading or event input.
  let options,event,logging=true,outputStarted=false;
  const callback=randomBytes(8).toString('hex'); // Local correlation, never a provider session ID.
  function record(stage,extra={}) {
    if(!options || !logging) return;
    const source=['startup','resume','clear','compact','fork'].includes(event?.source)?event.source:'invalid';
    logging=receipt(options,{v:2,callback,source,stage,...extra});
    if(!logging) {markStartup(file,3);process.stderr.write('SwapCtx: continuity metadata unavailable; observations are incomplete. Context validation remains required.\n');}
  }
  try {
    try {options=readConfig(file);}
    catch {throw new Failure('hook_config_invalid');}
    if(expectedScope!==undefined) requireThat(options.provider==='codex' && launchScope(options)===expectedScope,'hook_session_scope_mismatch');
    markStartup(file,2);
    record('callback_requested');
    const bytes=await new Promise((resolve,reject)=>{
      const chunks=[];let count=0;
      const timer=setTimeout(()=>{input.destroy();reject(new Failure('hook_input_timeout'));},3000);
      const fail=code=>{clearTimeout(timer);input.destroy();reject(new Failure(code));};
      input.on('data',chunk=>{count+=chunk.length;if(count>32768) fail('hook_input_too_large');else chunks.push(chunk);});
      input.once('error',()=>fail('hook_input_failed'));
      input.once('end',()=>{clearTimeout(timer);resolve(Buffer.concat(chunks));});
    });
    try {event=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));} catch {throw new Failure('hook_input_invalid');}
    const result=eventContext(options,event,undefined,(stage,fingerprint)=>record(stage,fingerprint?{fingerprint}:{}));
    outputStarted=true;
    await writeOutput(output,result);
    record('output_written'); // Local stream completion only, not acknowledgement.
  } catch(error) {
    const code=error instanceof Failure?error.code:'adapter_failed';
    record(outputStarted?'output_failed':'context_withheld',{code});
    // A broken pipe may have accepted partial output. Do not retry it, report
    // success, or claim to know what the provider received.
    if(outputStarted || code==='hook_output_failed') throw error;
    // Context-only advisory failure. Never pretend this blocks provider actions.
    try {await writeOutput(output,{systemMessage:'SwapCtx context unavailable ('+code+'); no memory was supplied. Run swapctx context and swapctx doctor.'});}
    catch(error) {record('output_failed',{code:'hook_output_failed'});throw error;}
    process.stderr.write('SwapCtx: '+code+'; context was not supplied.\n');
  }
}
function probe(provider,exe,run=spawnSync,prefix=[]) {
  const call=args=>run(exe,[...prefix,...args],{shell:false,encoding:'utf8',timeout:5000,maxBuffer:65536,windowsHide:true});
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
function launchScope(options) {
  return createHash('sha256').update('swapctx.native.scope.v1\0'+JSON.stringify(
    [options.provider,...['project','dataDir','swapctx'].map(key=>identity(options[key]))])).digest('hex');
}
// Serialized into the reviewed command, not written to a persistent cache.
// Only the exact verified adapter bytes execute; no check-then-require race.
async function stableBootstrap(adapterHash,scope) {
  let code='hook_session_environment_invalid',fd;
  try {
    const fs=require('node:fs'),path=require('node:path');
    const script=process.env.SWAPCTX_NATIVE_SCRIPT,config=process.env.SWAPCTX_NATIVE_CONFIG;
    if(Number(process.versions.node.split('.')[0])<24) throw Error();
    if(![script,config].every(p=>typeof p==='string' && p.length<=32768 && path.isAbsolute(p))
      || path.basename(config)!=='adapter.json') throw Error();
    code='hook_adapter_unavailable';
    const before=fs.lstatSync(script);
    if(!before.isFile() || before.isSymbolicLink() || before.size>131072) throw Error();
    fd=fs.openSync(script,'r');const stat=fs.fstatSync(fd);
    if(stat.dev!==before.dev || stat.ino!==before.ino || stat.size!==before.size) throw Error();
    const bytes=Buffer.alloc(131073);let length=0,count;
    while(length<bytes.length && (count=fs.readSync(fd,bytes,length,bytes.length-length,null))>0) length+=count;
    fs.closeSync(fd);fd=undefined;
    code='hook_adapter_integrity_failed';
    if(length!==stat.size || require('node:crypto').createHash('sha256').update(bytes.subarray(0,length)).digest('hex')!==adapterHash) throw Error();
    code='hook_adapter_load_failed';
    const module={exports:{}};
    const builtinRequire=name=>{if(!require('node:module').isBuiltin(name)) throw Error();return require(name);};
    require('node:vm').compileFunction(new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(0,length)),
      ['require','module','__filename'],{filename:script})(builtinRequire,module,script);
    code='hook_callback_failed';
    await module.exports.hook(config,process.stdin,process.stdout,scope);
  } catch(error) {
    if(error?.code==='hook_output_failed') code='hook_output_failed';
    process.stderr.write('SwapCtx: '+code+'; context delivery is unverified.\n');process.exitCode=1;
  } finally {if(fd!==undefined) {try {require('node:fs').closeSync(fd);} catch { /* best effort */ }}}
}
function hookCommands(script,config,stable) {
  // Compress only our own pinned bootstrap to remain below cmd's 8191-character
  // command limit after UTF-16 encoding. No untrusted compressed input is read.
  const entry=stable && 'eval(require(\'node:zlib\').inflateRawSync(Buffer.from(\''+
    require('node:zlib').deflateRawSync(Buffer.from('('+stableBootstrap.toString()+')('+JSON.stringify(stable.hash)+','+JSON.stringify(stable.scope)+')')).toString('base64')+
    '\',\'base64\')).toString(\'utf8\'))';
  const args=stable?[process.execPath,'-e',entry]:[process.execPath,script,'hook',config];
  const command=args.map(bashQuote).join(' ');
  if(process.platform!=='win32') return {command};
  // Never search a repository or PATH for the Windows hook interpreter.
  const systemRoot=process.env.SystemRoot;
  requireThat(typeof systemRoot==='string' && path.isAbsolute(systemRoot)
    && !/["\r\n%]/.test(systemRoot),'windows_hook_interpreter_unavailable');
  const shell=path.join(systemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe');
  requireThat(fs.existsSync(shell) && fs.statSync(shell).isFile(),'windows_hook_interpreter_unavailable');
  // Codex can dispatch through cmd /C or PowerShell -Command. A quoted
  // executable alone is not a PowerShell invocation; legacy cmd argv escaping
  // also corrupts embedded quotes (#32402). Use only a shell-neutral absolute
  // token, never PATH lookup or an unquoted user-controlled shell expression.
  requireThat(/^[A-Za-z]:\\[A-Za-z0-9_.\\-]+$/.test(shell),'windows_hook_interpreter_path_unsupported');
  // Best-effort update of an existing, fixed-size marker. Its failure must not
  // gate context or leak exception/path details into our diagnostics.
  const marker=stable?"([IO.Path]::Combine([IO.Path]::GetDirectoryName([Environment]::GetEnvironmentVariable('SWAPCTX_NATIVE_CONFIG')),'startup.bin'))":psQuote(startupFile(config));
  const observation="try { if (([IO.File]::GetAttributes("+marker+") -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'invalid marker' }; "+
    "$s=[IO.File]::Open("+marker+",[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite); try { "+
    "$b=[byte[]]::new(8); if ($s.Length -eq 8 -and $s.Read($b,0,8) -eq 8 -and [Text.Encoding]::ASCII.GetString($b) -match '^SCX1[01]{4}$') { $s.Position=4; $s.WriteByte(49) } "+
    "} finally { $s.Dispose() } } catch { [Console]::Error.WriteLine('SwapCtx: hook_startup_metadata_unavailable; early observations are incomplete.') }; ";
  const ps="$ErrorActionPreference='Stop'; "+observation+"try { "+
    (stable?'':"if (![IO.File]::Exists("+psQuote(script)+")) { [Console]::Error.WriteLine('SwapCtx: hook_adapter_unavailable; adapter did not start.'); exit 1 }; ")+
    '& '+args.map(psQuote).join(' ')+'; exit $LASTEXITCODE'+
    " } catch { [Console]::Error.WriteLine('SwapCtx: hook_node_start_failed; adapter did not start.'); exit 1 }";
  const commandWindows=shell+' -NoProfile -NonInteractive -EncodedCommand '+Buffer.from(ps,'utf16le').toString('base64');
  requireThat(commandWindows.length<=8000,'windows_hook_command_too_long');
  return {command,commandWindows};
}
function toml(value) {
  if(typeof value==='string') return JSON.stringify(value);
  if(typeof value==='number') return String(value);
  if(Array.isArray(value)) return '['+value.map(toml).join(',')+']';
  return '{'+Object.entries(value).map(([key,v])=>key+'='+toml(v)).join(',')+'}';
}
function configure(options,directory,script) {
  const config=path.join(directory,'adapter.json');
  fs.writeFileSync(startupFile(config),STARTUP_MAGIC+'0000',{flag:'wx',mode:0o600});
  const settings={...options,version:1,fingerprintKey:randomBytes(32).toString('hex'),receipt:path.join(directory,'receipt.jsonl')};
  fs.writeFileSync(config,JSON.stringify(settings),{flag:'wx',mode:0o600});
  const handler={type:'command',command:process.execPath,timeout:10};
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
  const commands=hookCommands(script,config,{hash:createHash('sha256').update(fs.readFileSync(script)).digest('hex'),scope:launchScope(options)});
  handler.command=commands.command;
  if(commands.commandWindows) handler.command_windows=commands.commandWindows;
  handler.additionalContextLimit=0; // Complete independently byte-bounded packet.
  return {args:['-c','hooks.SessionStart='+toml([group])],receipt:settings.receipt,
    env:{SWAPCTX_NATIVE_SCRIPT:script,SWAPCTX_NATIVE_CONFIG:config}};
}
async function runProvider(exe,args,dependencies={}) {
  const child=(dependencies.spawn||spawn)(exe,args,{stdio:'inherit',shell:false,...(dependencies.env?{env:dependencies.env}:{})});
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
  const prefix=options.providerScript?[options.providerScript]:[];
  if (!(dependencies.probe||probe)(options.provider,options.executable,undefined,prefix)) {
    warn('provider did not pass the native-hook compatibility probe; launching its original UI without SwapCtx context.');
    return run(options.executable,[...prefix,...args]);
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
    if(options.provider==='codex') warn('review SwapCtx in /hooks when new or changed. The definition is stable for this adapter, Node path and project/store/executable scope; trust persistence is provider-owned and remains unverified. Hooks disabled by settings/policy will not run. Trust is never bypassed. Do not submit a model prompt to diagnose loading; capture installed/active/trust status and hook errors first.');
    // Windows environment names are case-insensitive: remove inherited aliases
    // before supplying these launch-only values. Never mutate the parent's env.
    const env=setup.env && {...Object.fromEntries(Object.entries(process.env).filter(([key])=>!/^SWAPCTX_NATIVE_(SCRIPT|CONFIG)$/i.test(key))),...setup.env};
    const code=await run(options.executable,[...prefix,...setup.args,...args],{env});
    if(code!==0) primary=new Failure('native_provider_exit');
    try {
      const s=startupState(path.join(directory,'adapter.json'));
      warn('local hook stages (observed at least once): Windows bootstrap='+s.bootstrap+', Node entry='+s.node+', config validated='+s.config+', receipt write failed='+s.receiptFailed+'. Provider receipt and model adherence remain unknown.');
      if(!s.node) warn('hook entry not observed: capture /hooks registration, active/trust status and hook exit/stderr. Absent local observations cannot distinguish skipped loading/trust/policy from shell/Node failure.');
      else if(!s.config) warn('hook Node entry observed but configuration validation not observed: inspect hook_config_invalid or interrupted config loading.');
      else if(!fs.existsSync(setup.receipt)) warn('hook config validated but no callback receipt exists: logging failed or metadata was removed; this is not evidence that the hook was skipped.');
    } catch {warn('early hook observations unavailable or invalid; no conclusion about execution can be drawn.');}
    if(!fs.existsSync(setup.receipt)) warn('no hook callback was recorded; context delivery is unverified. Hooks may be missing, disabled or untrusted, or fail during shell launch, Node startup, adapter/config loading or receipt writing. Inspect the provider hook exit status/stderr (hook_node_start_failed, hook_adapter_unavailable, hook_config_invalid, or continuity metadata unavailable); no receipt alone cannot identify the failing layer. Run swapctx native '+options.provider+' --check for local prerequisites; it does not test provider shell dispatch.');
    else {
      try {
        const s=summarizeReceipts(setup.receipt);
        warn('hook adapter results: '+s.prepared+' contexts prepared, '+s.written+' output writes completed, '+s.failures+' failure observations, '+s.unfinished+' unfinished callbacks; '+s.repeated+' repeated snapshots (not suppressed). Provider receipt and model adherence remain unknown.');
      } catch {warn('hook outcome metadata is invalid; context delivery remains unverified.');}
    }
    if(fs.existsSync(setup.receipt+'.incomplete') || fs.existsSync(setup.receipt+'.lock')) warn('continuity observations are incomplete; recorded counts are not session totals.');
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
function check(options) {
  process.stdout.write('Node runtime: ready ('+process.versions.node+')\n');
  process.stdout.write('Provider receipt: unknown. Model adherence: unknown. No provider was started.\n');
  const parent=identity(os.tmpdir()),directory=fs.mkdtempSync(path.join(parent,'swapctx-hooks-check-'));
  const owned=fs.lstatSync(directory);
  try {
    retrieve(options,process.cwd());
    process.stdout.write('Context: current snapshot validated; mandatory requirements/rules complete (local preparation only)\n');
    configure(options,directory,__filename);
    process.stdout.write('Hook configuration: generated locally; provider loading, version compatibility and trust remain unverified\n');
    if(options.provider==='codex') process.stdout.write('Hook identity: stable command bound to adapter bytes and project/store/executable scope; trust remains provider-owned\n');
    return 0;
  } catch(error) {
    process.stderr.write('SwapCtx: local health check failed: '+(error instanceof Failure?error.code:'native_check_failed')+
      '; no memory contents displayed. Inspect swapctx doctor and memory freshness/conflicts locally.\n');
    return 1;
  } finally {
    const actual=fs.lstatSync(directory);
    requireThat(!actual.isSymbolicLink() && actual.dev===owned.dev && actual.ino===owned.ino
      && identity(path.dirname(directory))===parent,'unsafe_adapter_cleanup');
    fs.rmSync(directory,{recursive:true});
  }
}
function optionsFrom(args) {
  const options={};let i=0;
  for(;i<args.length && args[i]!=='--';i+=2) {
    const name={'--provider':'provider','--swapctx':'swapctx','--data-dir':'dataDir','--project':'project','--executable':'executable','--provider-script':'providerScript'}[args[i]];
    requireThat(name && args[i+1] && !Object.hasOwn(options,name),'invalid_adapter_arguments');options[name]=args[i+1];
  }
  requireThat(['codex','claude'].includes(options.provider) && typeof options.executable==='string','invalid_adapter_arguments');
  for(const key of ['swapctx','project']) identity(options[key]);
  if(options.providerScript) {
    identity(options.providerScript);
    requireThat(fs.statSync(options.providerScript).isFile() && /\.(?:c?js|mjs)$/i.test(options.providerScript),'invalid_provider_script');
  }
  requireThat(typeof options.dataDir==='string' && path.isAbsolute(options.dataDir),'invalid_adapter_arguments');
  return {options,args:args.slice(i+1)};
}
module.exports={validatePacket,retrieve,eventContext,hook,probe,hookCommands,toml,configure,runProvider,launch,check,optionsFrom,fingerprint,summarizeReceipts,startupState,PACKET_BYTES,RECEIPT_BYTES};
if(require.main===module) (async()=>{
  requireThat(Number(process.versions.node.split('.')[0])>=24,'node_24_required');
  const [mode,...args]=process.argv.slice(2);
  if(mode==='hook' && args.length===1) await hook(args[0]);
  else if(mode==='launch') {const parsed=optionsFrom(args);process.exitCode=await launch(parsed.options,parsed.args);}
  else if(mode==='check') {const parsed=optionsFrom(args);requireThat(parsed.args.length===0,'invalid_adapter_arguments');process.exitCode=check(parsed.options);}
  else throw new Failure('invalid_adapter_arguments');
})().catch(error=>{process.stderr.write('SwapCtx: '+(error instanceof Failure?error.code:'native_adapter_failed')+'; no provider state was changed by SwapCtx.\n');process.exitCode=1;});
