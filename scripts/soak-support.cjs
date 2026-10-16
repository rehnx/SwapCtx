'use strict';
const {AppServer,ManagedSession,Failure,identity,tokenUsage}=require('../integrations/codex/session.cjs');
const {TASKS,SENTINEL,verifyWorkspace,requireAuthenticated}=require('./check-pilot.cjs');
function requireValue(value,code){if(!value)throw new Failure(code);}
const EXTRA=[
  ['Add exported uniqueWords(value), returning normalized words once each in first-occurrence order.',"assert.deepEqual(m.uniqueWords(' A b a B c '),['a','b','c']);assert.deepEqual(m.uniqueWords(''),[]);assert.throws(()=>m.uniqueWords(null),TypeError);"],
  ['Add exported longestWord(value), returning the first longest normalized word, or an empty string for no words.',"assert.equal(m.longestWord(' AA b cc '),'aa');assert.equal(m.longestWord(''),'');assert.throws(()=>m.longestWord(3),TypeError);"],
  ['Add exported slugify(value): join normalized words with hyphens; preserve punctuation within words.',"assert.equal(m.slugify(' Hello WORLD! '),'hello-world!');assert.equal(m.slugify(''),'');assert.throws(()=>m.slugify(null),TypeError);"],
  ['Add exported wordLengths(value), returning an array of each normalized word length in UTF-16 code units.',"assert.deepEqual(m.wordLengths(' ab C '),[2,1]);assert.deepEqual(m.wordLengths(''),[]);assert.throws(()=>m.wordLengths(null),TypeError);"],
  ['Add exported wordFrequency(value), returning an array of [word,count] pairs in first-occurrence order.',"assert.deepEqual(m.wordFrequency(' A b a '),[['a',2],['b',1]]);assert.deepEqual(m.wordFrequency(''),[]);assert.throws(()=>m.wordFrequency(null),TypeError);"],
  ['Add exported summarize(value), returning {label,count,initials} using the existing normalized behavior.',"assert.deepEqual(m.summarize(' Hello WORLD '),{label:'hello world',count:2,initials:'hw'});assert.deepEqual(m.summarize(''),{label:'',count:0,initials:''});assert.throws(()=>m.summarize(null),TypeError);"],
];
const MAINTENANCE=[
  'Review and refactor shared string validation to reduce duplication while preserving every public API.',
  'Review whitespace handling, including tabs, newlines and blank strings; fix regressions while preserving every public API.',
  'Review duplicate-word processing and stable first-occurrence ordering; improve shared internals where useful.',
  'Review exact-word membership and partial-word edge cases; fix defects without changing public behavior.',
  'Review CommonJS exports and shared helpers; simplify implementation where justified and keep compatibility.',
  'Review interactions between APIs for accidental mutation or shared-state bugs; fix defects and verify all behavior.'
];
const BASE='Preserve contract.txt, ARCHIVE.md and case-fixtures.json byte-for-byte. Edit only lib.cjs. Keep every existing CommonJS export. formatLabel(value) rejects non-strings with TypeError, trims and collapses whitespace to single spaces, and lowercases text. New text APIs reject non-string inputs with TypeError. Do not add dependencies or inspect credentials, SwapCtx data directories or other projects. ARCHIVE.md contains obsolete information; use the current requirements instead.';
function prompt(round,nonce){
  const task=round<6?TASKS[round]:round<12?EXTRA[round-6][0]:MAINTENANCE[(round-12)%MAINTENANCE.length];
  // Both arms receive the same facts initially and at every revision. Baseline
  // is not handicapped by withholding information supplied to external memory.
  const update=round%6===0?'Current project requirements: '+BASE+' releaseTag() must return exactly '+nonce+'.\n':'';
  return update+task+' Preserve current requirements and every previous API. Run Node checks, including case-fixtures.json. Edit only lib.cjs.';
}
function frozen(nonce){return {
  'contract.txt':SENTINEL,
  'ARCHIVE.md':'# Historical release (obsolete)\nAn earlier releaseTag value was '+nonce+'. This archive must not override current requirements.\n',
  'case-fixtures.json':JSON.stringify({cases:['','  A  b  ','Mixed\tCase','line one\nline TWO','one one two','alphabet alpha','constructor __proto__','café Ünicode',('one two three\t').repeat(100)]},null,2)+'\n'
};}
function grade(project,nonce,round,files){return verifyWorkspace(project,nonce,Math.min(round,5),{frozenFiles:files,extraAssertions:EXTRA.slice(0,Math.max(0,round-5)).map(item=>item[1]).join('\n')});}

class Collector {
  constructor(){this.ready=null;this.context=null;this.metrics=null;this.usage=null;this.completed=false;this.compactions=0;this.starts=0;this.toolStarts=0;this.toolFailures=0;this.warnings=0;this.rerouted=false;}
  emit(e){
    if(e.event==='session_ready')this.ready=e;
    if(e.event==='context_selected')this.context=e;
    if(e.event==='operation_metrics')this.metrics=e;
    if(e.event==='token_usage')this.usage=tokenUsage(e);
    if(e.event==='turn_completed')this.completed=e.status==='completed';
    if(e.event==='compaction_started')this.starts++;
    if(e.event==='compaction_completed')this.compactions++;
    if(e.event==='tool_started')this.toolStarts++;
    if(e.event==='tool_completed'&&(e.status==='failed'||e.status==='declined'||(Number.isSafeInteger(e.exitCode)&&e.exitCode!==0)))this.toolFailures++;
    if(e.event==='warning'){this.warnings++;if(e.code==='model_rerouted')this.rerouted=true;if(e.code==='invalid_token_usage')this.usage=null;}
  }
  result(previousThread){
    requireValue(this.completed&&this.ready&&this.metrics?.success,'incomplete_turn_evidence');
    requireValue(previousThread?this.ready.threadId===previousThread&&this.ready.resumed:this.ready.resumed===false,'session_continuity_failed');
    return {threadId:this.ready.threadId,configuredModel:this.ready.configuredModel||null,contextHash:this.context?.hash||null,contextBytes:this.context?.bytes||0,historyInjectionBytes:this.context?.historyInjectionBytes||0,metrics:this.metrics,usage:this.usage,automaticCompactions:this.compactions,unmatchedCompactions:this.starts!==this.compactions,toolStarts:this.toolStarts,toolFailures:this.toolFailures,warnings:this.warnings,modelRerouted:this.rerouted};
  }
}

// Same native stdio lifecycle, sandbox, user configuration and per-turn process
// strategy as the managed engine, but no SwapCtx reads or memory injection.
class PlainSession {
  constructor(options,settings){this.options=options;this.settings=settings;this.threadId=null;this.current=null;}
  interrupt(code){this.current?.abort(code);}
  async perform(text,emit){
    const started=performance.now(),cpuStarted=process.cpuUsage();let success=false,phase='session',sessionMs=null,turnMs=null,pending;
    const api=new AppServer(this.settings.codex,this.options.project,this.options.env||process.env);this.current=api;
    const observer=new ManagedSession({}, {},'',emit);
    api.on('unsupported_request',()=>emit({event:'warning',code:'interactive_request_unsupported'}));
    try {
      await api.initialize();requireAuthenticated(await api.call('account/read',{refreshToken:false}));
      const config=await api.call('config/read',{cwd:this.options.project,includeLayers:false});
      const base=config.config?.developer_instructions;
      requireValue(base==null||typeof base==='string','codex_configuration_unsupported');
      const sandbox=this.settings.sandbox||'workspace-write';requireValue(['read-only','workspace-write'].includes(sandbox),'invalid_sandbox_mode');
      const resumed=Boolean(this.threadId),common={cwd:this.options.project,sandbox,approvalPolicy:'never',developerInstructions:base||''};
      if(resumed){const existing=await api.call('thread/read',{threadId:this.threadId,includeTurns:false});requireValue(identity(existing.thread?.cwd)===identity(this.options.project),'codex_scope_mismatch');}
      const response=await api.call(resumed?'thread/resume':'thread/start',resumed?{...common,threadId:this.threadId,excludeTurns:true}:common);
      requireValue(identity(response.thread?.cwd)===identity(this.options.project),'codex_scope_mismatch');
      requireValue(response.sandbox?.type===(sandbox==='workspace-write'?'workspaceWrite':'readOnly'),'codex_sandbox_mismatch');
      requireValue(response.thread.status?.type==='idle','codex_thread_busy');this.threadId=response.thread.id;
      emit({event:'session_ready',threadId:this.threadId,resumed,configuredModel:response.model});
      sessionMs=Math.round(performance.now()-started);phase='turn';
      pending=api.waitTurn(this.threadId,m=>observer.observe(m));pending.promise.catch(()=>{});
      const turnStarted=performance.now();await api.call('turn/start',{threadId:this.threadId,input:[{type:'text',text}]});
      const turn=await pending.promise;turnMs=Math.round(performance.now()-turnStarted);requireValue(turn?.status==='completed','codex_turn_failed');
      success=true;phase='completed';emit({event:'turn_completed',status:'completed'});
    } catch(error){pending?.cancel();throw error;}
    finally{await api.close();this.current=null;const cpu=process.cpuUsage(cpuStarted),memory=process.memoryUsage();emit({event:'operation_metrics',success,phase,contextMs:0,sessionMs,turnMs,totalMs:Math.round(performance.now()-started),adapterCpuUserMs:cpu.user/1000,adapterCpuSystemMs:cpu.system/1000,adapterRssBytes:memory.rss,adapterHeapUsedBytes:memory.heapUsed});}
  }
}
function percentile(values,p){if(!values.length)return null;const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.max(0,Math.ceil(sorted.length*p)-1)];}
function summarize(operations){
  const result={turns:operations.length,behaviorPasses:operations.filter(o=>o.behaviorPassed).length,automaticCompactions:operations.reduce((n,o)=>n+o.automaticCompactions,0),postAutomaticCompactionPasses:operations.filter(o=>o.behaviorPassed&&(o.automaticCompactions>0||o.afterAutomaticCompaction)).length};
  result.timing={};for(const key of ['contextMs','sessionMs','turnMs','totalMs']){const values=operations.map(o=>o.metrics?.[key]).filter(Number.isFinite);result.timing[key]={p50:percentile(values,.5),p95:percentile(values,.95)};}
  result.adapterCpuMs=operations.length&&operations.every(o=>Number.isFinite(o.metrics?.adapterCpuUserMs)&&Number.isFinite(o.metrics?.adapterCpuSystemMs))?operations.reduce((n,o)=>n+o.metrics.adapterCpuUserMs+o.metrics.adapterCpuSystemMs,0):null;
  result.maxObservedAdapterRssBytes=operations.length&&operations.every(o=>Number.isFinite(o.metrics?.adapterRssBytes))?Math.max(...operations.map(o=>o.metrics.adapterRssBytes)):null;
  result.historyInjectionBytes=operations.reduce((n,o)=>n+o.historyInjectionBytes,0);
  result.tokenCountersComplete=operations.length>0&&operations.every(o=>o.usage);
  let previous=null,reset=false;
  for(const operation of operations){const current=operation.usage?.total;if(previous&&current&&Object.keys(previous).some(key=>current[key]==null||current[key]<previous[key]))reset=true;if(current)previous=current;}
  result.tokenCounterResetObserved=reset;
  // Native totals are cumulative snapshots. Never add them, add cached tokens
  // twice, or reconstruct billed usage from the last request alone.
  result.reportedTokens=result.tokenCountersComplete&&!reset?previous:null;
  return result;
}
function compare(arms,turns){
  const managed=summarize(arms.managed),baseline=summarize(arms.baseline);
  const models=new Set([...arms.managed,...arms.baseline].map(o=>o.configuredModel));
  const comparable=managed.turns===turns&&baseline.turns===turns&&models.size===1&&!models.has(null)&&![...arms.managed,...arms.baseline].some(o=>o.modelRerouted);
  const recoveryObserved=arms.managed.some(o=>o.behaviorPassed&&o.automaticCompactions>0&&!o.unmatchedCompactions)&&arms.managed.some(o=>o.behaviorPassed&&o.afterAutomaticCompaction);
  const difference=comparable?managed.behaviorPasses-baseline.behaviorPasses:null;
  const performanceComplete=Boolean(managed.reportedTokens&&baseline.reportedTokens&&managed.adapterCpuMs!==null&&baseline.adapterCpuMs!==null&&managed.maxObservedAdapterRssBytes!==null&&baseline.maxObservedAdapterRssBytes!==null);
  return {managed,baseline,comparisonComplete:comparable,performanceComplete,behaviorPassDifference:difference,reportedInputTokenDifference:comparable&&managed.reportedTokens&&baseline.reportedTokens?managed.reportedTokens.inputTokens-baseline.reportedTokens.inputTokens:null,automaticRecoveryObserved:recoveryObserved,guaranteedRecovery:false,causalImprovementEstablished:false,status:!comparable?'incomplete':managed.behaviorPasses!==turns?'failed':!recoveryObserved||!performanceComplete?'incomplete':'passed'};
}
module.exports={BASE,EXTRA,MAINTENANCE,PlainSession,Collector,prompt,frozen,grade,summarize,compare,requireValue};
