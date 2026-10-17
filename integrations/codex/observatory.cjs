'use strict';
// Local, bounded metadata only. No transcripts, prompts, command lines or file contents.
const fs = require('node:fs');
const STATES = new Set(['idle','preparing','ready','running','compacting','failed','closed','disabled']);
const CATEGORIES = new Set(['contextWindowExceeded','sessionBudgetExceeded','usageLimitExceeded','rateLimitExceeded','flexUnavailable','serverOverloaded','cyberPolicy','misalignmentPolicyViolation','tooManyDenials','internalServerError','unauthorized','badRequest','threadRollbackFailed','sandboxError','other','httpConnectionFailed','responseStreamConnectionFailed','responseStreamDisconnected','responseTooManyFailedAttempts','activeTurnNotSteerable']);
const HTTP_CATEGORIES = new Set(['httpConnectionFailed','responseStreamConnectionFailed','responseStreamDisconnected','responseTooManyFailedAttempts']);
const COUNTERS = ['inputTokens','cachedInputTokens','outputTokens','reasoningOutputTokens','totalTokens','cacheWriteInputTokens'];
const METRICS = ['contextMs','sessionMs','turnMs','totalMs','adapterCpuUserMs','adapterCpuSystemMs','adapterRssBytes','adapterHeapUsedBytes'];
const nonnegative = n => Number.isSafeInteger(n) && n >= 0;
function entryExists(file) {
  try {fs.lstatSync(file);return true;} catch(e) {if (e.code === 'ENOENT') return false;throw e;}
}
function failureDetails(turn) {
  const info = turn?.error?.codexErrorInfo;
  const key = typeof info === 'string' ? info : info && typeof info === 'object' && !Array.isArray(info) && Object.keys(info).length === 1 ? Object.keys(info)[0] : null;
  const result = {status:['failed','interrupted'].includes(turn?.status)?turn.status:'unknown',category:CATEGORIES.has(key)?key:'unknown'};
  const http = info?.[key]?.httpStatusCode;
  if (HTTP_CATEGORIES.has(key) && nonnegative(http) && http <= 65535) result.httpStatusCode = http;
  return result;
}
function usage(value) {
  if (!value || !['last','total'].every(k => value[k] && COUNTERS.slice(0,5).every(f => nonnegative(value[k][f])))) return null;
  const result = {};
  for (const key of ['last','total']) result[key] = Object.fromEntries(COUNTERS.filter(f => nonnegative(value[key][f])).map(f => [f,value[key][f]]));
  result.modelContextWindow = nonnegative(value.modelContextWindow)?value.modelContextWindow:null;
  return result;
}
function snapshot(value) {
  if (!value || value.protocol !== 'swapctx.observatory' || value.version !== 1 || !STATES.has(value.state) || !nonnegative(value.pid) || value.pid === 0 || !nonnegative(value.startedUnixMs) || !nonnegative(value.updatedUnixMs) || typeof value.enabled !== 'boolean') throw new Error('invalid_observatory_snapshot');
  const result = {protocol:'swapctx.observatory',version:1,pid:value.pid,startedUnixMs:value.startedUnixMs,updatedUnixMs:value.updatedUnixMs,enabled:value.enabled,state:value.state};
  result.completedTurns = nonnegative(value.completedTurns)?value.completedTurns:0;
  result.failedTurns = nonnegative(value.failedTurns)?value.failedTurns:0;
  result.toolFailures = nonnegative(value.toolFailures)?value.toolFailures:0;
  result.compactions = nonnegative(value.compactions)?value.compactions:0;
  if (value.context && /^[a-f0-9]{64}$/.test(value.context.hash)) {
    result.context = {hash:value.context.hash};
    if (value.context.hashVersion === 2) result.context.hashVersion = 2;
    for (const key of ['bytes','records','requirements','historyInjectionBytes','runtimeInstructionBytes']) if (nonnegative(value.context[key])) result.context[key] = value.context[key];
  }
  if (/^[a-f0-9-]{36}$/.test(value.threadId || '')) result.threadId = value.threadId;
  if (typeof value.configuredModel === 'string' && /^[A-Za-z0-9_.:/-]{1,128}$/.test(value.configuredModel)) result.configuredModel = value.configuredModel;
  if (usage(value.usage)) result.usage = usage(value.usage);
  if (value.failure) result.failure = failureDetails({status:value.failure.status,error:{codexErrorInfo:HTTP_CATEGORIES.has(value.failure.category)?{[value.failure.category]:{httpStatusCode:value.failure.httpStatusCode}}:value.failure.category}});
  if (value.metrics) {
    result.metrics = Object.fromEntries(METRICS.filter(k => Number.isFinite(value.metrics[k]) && value.metrics[k] >= 0).map(k => [k,value.metrics[k]]));
    if (['context','session','turn','completed'].includes(value.metrics.phase)) result.metrics.phase = value.metrics.phase;
    if (typeof value.metrics.success === 'boolean') result.metrics.success = value.metrics.success;
    if (['turn','compact'].includes(value.metrics.operation)) result.metrics.operation = value.metrics.operation;
  }
  return result;
}
function readSnapshot(file) {
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 16384) throw new Error('invalid_observatory_snapshot');
  return snapshot(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(fs.readFileSync(file))));
}
function observation(file, now = Date.now()) {
  const value = readSnapshot(file);
  let writerRunning = null;
  try {process.kill(value.pid,0);writerRunning = true;} catch(e) {if (e.code === 'ESRCH') writerRunning = false;}
  return {snapshot:value,ageMs:Math.max(0,now-value.updatedUnixMs),writerRunning};
}
function render(value) {
  const s = value.snapshot;
  return `SwapCtx: ${s.state}; turns ${s.completedTurns} completed/${s.failedTurns} failed; tool failures ${s.toolFailures}; compactions observed ${s.compactions}; context ${s.context?.bytes ?? '?'} bytes; last input ${s.usage?.last.inputTokens ?? '?'} tokens; snapshot age ${Math.round(value.ageMs/1000)}s; writer ${value.writerRunning === null?'unknown':value.writerRunning?'present':'absent'}${s.failure?'; last failure '+s.failure.category:''}${s.metrics?.phase?'; last phase '+s.metrics.phase:''}`;
}
class Observatory {
  constructor(file, {publish,clock = Date.now,onError = () => {},onDisplay = () => {}}) {
    this.file = file;this.publish = publish;this.clock = clock;this.onError = onError;this.onDisplay = onDisplay;this.timer = null;this.enabled = false;
    const now = clock();
    this.value = {protocol:'swapctx.observatory',version:1,pid:process.pid,startedUnixMs:now,updatedUnixMs:now,enabled:false,state:'idle',completedTurns:0,failedTurns:0,toolFailures:0,compactions:0};
  }
  enable() {
    if (this.enabled) return;
    if (entryExists(this.file)) readSnapshot(this.file); // Includes dangling links.
    this.enabled = true;this.value.enabled = true;this.value.state = 'idle';this.flush();
  }
  disable() {
    if (!this.enabled) return;
    this.value.enabled = false;this.value.state = 'disabled';this.flush();this.enabled = false;
  }
  accept(event) {
    if (!this.enabled) return;
    let immediate = false;
    switch (event.event) {
      case 'operation_started': this.value.state = 'preparing';delete this.value.failure;delete this.value.metrics;delete this.value.usage;break;
      case 'context_selected': this.value.context = event;break;
      case 'session_ready': this.value.threadId = event.threadId;this.value.configuredModel = event.configuredModel;this.value.state = 'ready';break;
      case 'turn_started': this.value.state = 'running';break;
      case 'token_usage': this.value.usage = event;break;
      case 'compaction_started': this.value.state = 'compacting';break;
      case 'compaction_completed': this.value.compactions++;this.value.state = 'running';break;
      case 'tool_completed': if (event.status === 'failed' || event.status === 'declined' || (Number.isInteger(event.exitCode) && event.exitCode !== 0)) this.value.toolFailures++;break;
      case 'turn_failed': this.value.failure = event.details;this.value.state = 'failed';immediate = true;break;
      case 'operation_metrics': this.value.metrics = event;this.value.state = event.success?'idle':'failed';if (event.success) this.value.completedTurns++;else this.value.failedTurns++;immediate = true;break;
      default: return; // Assistant deltas must never schedule writes.
    }
    if (immediate) this.flush();
    else if (!this.timer) this.timer = setTimeout(() => this.flush(),250);
  }
  flush() {
    clearTimeout(this.timer);this.timer = null;
    if (!this.enabled) return;
    this.value.updatedUnixMs = this.clock();
    try {
      if (entryExists(this.file)) {
        const prior = readSnapshot(this.file);
        if (this.published && prior.pid !== process.pid) throw new Error('observatory_owner_changed');
      }
      const value = snapshot(this.value);this.publish(this.file,value);this.published = true;this.onDisplay(value);
    }
    catch {this.enabled = false;this.value.enabled = false;this.onError();}
  }
  close() {if (this.enabled) {this.value.state = 'closed';this.flush();}clearTimeout(this.timer);this.timer = null;}
}
module.exports = {entryExists,failureDetails,snapshot,readSnapshot,observation,render,Observatory};
