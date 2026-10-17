'use strict';
// Read retained evidence only. This script cannot start an agent or a provider.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
function auditReport(report) {
  if (report?.protocol !== 'swapctx.codex-soak' || report.version !== 1 || !Number.isInteger(report.requestedTurnsPerArm) || report.requestedTurnsPerArm < 1 || report.requestedTurnsPerArm > 96) throw Error('invalid_phase4_evidence');
  const result = {status:['running','passed','failed','blocked','incomplete'].includes(report.status)?report.status:'unknown',requestedTurnsPerArm:report.requestedTurnsPerArm,arms:{}};
  for (const name of ['managed','baseline']) {
    const turns = report.arms?.[name];
    if (!Array.isArray(turns) || turns.length > report.requestedTurnsPerArm || !turns.every((t,i) => t?.round === i && typeof t.behaviorPassed === 'boolean' && Number.isSafeInteger(t.automaticCompactions) && t.automaticCompactions >= 0)) throw Error('invalid_phase4_evidence');
    result.arms[name] = {retainedTurns:turns.length,behaviorPasses:turns.filter(t => t.behaviorPassed).length,automaticCompactions:turns.reduce((sum,t) => sum+t.automaticCompactions,0),reportedSummaryConsistent:report.comparison?.[name]?.turns === turns.length && report.comparison[name].behaviorPasses === turns.filter(t => t.behaviorPassed).length};
  }
  result.pairedTurnsComplete = Object.values(result.arms).every(a => a.retainedTurns === result.requestedTurnsPerArm);
  result.automaticRecoveryReported = report.comparison?.automaticRecoveryObserved === true;
  result.comparativeBenefitEstablished = false; // Retained runs alone do not establish causality.
  if (report.error) result.failure = {code:['codex_turn_failed','codex_connection_closed','codex_turn_timeout','soak_interrupted','soak_time_budget_exceeded'].includes(report.error.code)?report.error.code:'unknown',rootCauseRecoverable:false};
  return result;
}
function readEvidence(file) {
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 2*1024*1024) throw Error('invalid_phase4_evidence');
  const bytes = fs.readFileSync(file);
  return {source:path.basename(file),sha256:crypto.createHash('sha256').update(bytes).digest('hex'),...auditReport(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)))};
}
module.exports = {auditReport,readEvidence};
if (require.main === module) {
  try {
    if (process.argv.length < 3 || process.argv.length > 12) throw Error('invalid_arguments');
    console.log(JSON.stringify({protocol:'swapctx.phase4-audit',version:1,offline:true,evidence:process.argv.slice(2).map(readEvidence)},null,2));
  } catch {console.error(JSON.stringify({ok:false,error:{code:'phase4_evidence_unavailable_or_invalid'}}));process.exitCode = 1;}
}
