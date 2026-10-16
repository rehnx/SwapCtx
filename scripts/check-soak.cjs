'use strict';
// Explicit, bounded real-provider comparison. No forced compaction/threshold.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),crypto=require('node:crypto');
const {spawnSync}=require('node:child_process');
const {AppServer,ManagedSession,Failure,atomicJson,SUPPORTED}=require('../integrations/codex/session.cjs');
const {requireAuthenticated,resultStatus}=require('./check-pilot.cjs');
const {BASE,PlainSession,Collector,prompt,frozen,grade,compare,requireValue}=require('./soak-support.cjs');
function parse(argv){
  requireValue(argv.length>=3,'invalid_arguments');
  const options={codex:argv.shift(),swapctx:argv.shift(),output:argv.shift(),turns:24,minutes:120,first:null};
  requireValue([options.codex,options.swapctx,options.output].every(value=>path.isAbsolute(value)),'absolute_paths_required');
  const seen=new Set();
  while(argv.length){const flag=argv.shift(),value=argv.shift();requireValue(value&&!seen.has(flag),'invalid_arguments');seen.add(flag);
    if(flag==='--turns'){options.turns=Number(value);requireValue(Number.isInteger(options.turns)&&options.turns>=6&&options.turns<=96,'invalid_turn_budget');}
    else if(flag==='--minutes'){options.minutes=Number(value);requireValue(Number.isInteger(options.minutes)&&options.minutes>=1&&options.minutes<=720,'invalid_time_budget');}
    else if(flag==='--first'){requireValue(['managed','baseline'].includes(value),'invalid_order');options.first=value;}
    else throw new Failure('invalid_arguments');
  }
  return options;
}
function save(output,report,create=false){
  if(!create){const stat=fs.lstatSync(output);requireValue(stat.isFile()&&!stat.isSymbolicLink()&&stat.size<2*1024*1024,'unsafe_report_file');requireValue(JSON.parse(fs.readFileSync(output,'utf8')).runId===report.runId,'report_ownership_changed');}
  atomicJson(output,report,!create);
}
async function main(argv=process.argv.slice(2)){
  const options=parse(argv);
  const report={protocol:'swapctx.codex-soak',version:1,runId:crypto.randomUUID(),date:new Date().toISOString(),codexVersion:SUPPORTED,nodeVersion:process.versions.node,status:'running',requestedTurnsPerArm:options.turns,timeBudgetMinutes:options.minutes,firstArm:options.first||(crypto.randomInt(2)?'managed':'baseline'),modelEvaluationStarted:false,automaticCompactionForced:false,guaranteedRecovery:false,arms:{managed:[],baseline:[]}};
  // Claim a new report before any model request; checkpoint only that report.
  save(options.output,report,true);
  let root,current,preflight,stopReason,timer,error;
  const stop=code=>{stopReason ||= code;preflight?.abort(stopReason);current?.interrupt(stopReason);};
  const interrupted=()=>stop('soak_interrupted');process.on('SIGINT',interrupted);process.on('SIGTERM',interrupted);
  try{
    const version=spawnSync(options.codex,['--version'],{encoding:'utf8',timeout:5000,maxBuffer:8192,windowsHide:true});
    requireValue(!version.error&&version.status===0&&version.stdout.trim()==='codex-cli '+SUPPORTED,'unsupported_codex_version');
    root=fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-soak-'));
    const projects={managed:path.join(root,'managed ü space'),baseline:path.join(root,'baseline ü space')},dataDir=path.join(root,'data');
    for(const project of Object.values(projects))fs.mkdirSync(project);
    timer=setTimeout(()=>stop('soak_time_budget_exceeded'),options.minutes*60000);
    preflight=new AppServer(options.codex,projects.managed);
    try{await preflight.initialize();requireAuthenticated(await preflight.call('account/read',{refreshToken:false}));}finally{await preflight.close();preflight=null;}
    requireValue(!stopReason,stopReason);
    const old=crypto.randomBytes(16).toString('hex');let nonce=crypto.randomBytes(16).toString('hex'),releaseId=3;
    const files=frozen(old);
    for(const project of Object.values(projects)){
      for(const [name,contents] of Object.entries(files))fs.writeFileSync(path.join(project,name),contents,{flag:'wx'});
      fs.writeFileSync(path.join(project,'lib.cjs'),"'use strict';\nexports.formatLabel=value=>value.trim();\nexports.releaseTag=()=> 'UNSET';\n",{flag:'wx'});
    }
    const cli=args=>{requireValue(!stopReason,stopReason);const result=spawnSync(options.swapctx,['--data-dir',dataDir,...args],{cwd:projects.managed,encoding:'utf8',timeout:15000,maxBuffer:65536,windowsHide:true});requireValue(!result.error&&result.status===0,'soak_memory_operation_failed');};
    cli(['init']);cli(['memory','add',BASE,'--kind','requirement']);
    cli(['memory','add','releaseTag() must return exactly '+old,'--kind','requirement','--key','release-tag']);
    cli(['memory','supersede','2','releaseTag() must return exactly '+nonce]);
    const managed=new ManagedSession({swapctx:options.swapctx,dataDir,project:projects.managed},{codex:options.codex,sandbox:'workspace-write'},path.join(root,'managed-session.json'));
    const baseline=new PlainSession({project:projects.baseline},{codex:options.codex});
    const sessions={managed,baseline},threads={},automaticSeen={managed:false,baseline:false};
    report.modelEvaluationStarted=true;save(options.output,report);
    for(let round=0;round<options.turns;round++){
      requireValue(!stopReason,stopReason);
      if(round>0&&round%6===0){nonce=crypto.randomBytes(16).toString('hex');cli(['memory','supersede',String(releaseId++),'releaseTag() must return exactly '+nonce]);}
      const order=round%2===0?[report.firstArm,report.firstArm==='managed'?'baseline':'managed']:[report.firstArm==='managed'?'baseline':'managed',report.firstArm];
      const text=prompt(round,nonce);
      for(const arm of order){
        requireValue(!stopReason,stopReason);const collector=new Collector();current=sessions[arm];
        if(arm==='managed'){current.emit=e=>collector.emit(e);await current.perform(text);}else await current.perform(text,e=>collector.emit(e));
        const operation=collector.result(threads[arm]);threads[arm]=operation.threadId;
        requireValue(arm!=='managed'||Boolean(collector.context),'missing_managed_context_event');
        operation.round=round;operation.afterAutomaticCompaction=automaticSeen[arm];
        if(operation.automaticCompactions>0&&!operation.unmatchedCompactions)automaticSeen[arm]=true;
        try{Object.assign(operation,grade(projects[arm],nonce,round,files));operation.behaviorPassed=true;}
        catch(failure){if(!(failure instanceof Failure))throw failure;operation.behaviorPassed=false;operation.behaviorError=failure.code;}
        report.arms[arm].push(operation);report.comparison=compare(report.arms,options.turns);save(options.output,report);
        process.stderr.write(JSON.stringify({event:'soak_progress',round,arm,behaviorPassed:operation.behaviorPassed,automaticCompactions:operation.automaticCompactions})+'\n');
        current=null;
      }
    }
  }catch(failure){error=new Failure(stopReason||(failure instanceof Failure?failure.code:'soak_local_failure'));report.error={code:error.code};}
  finally{
    clearTimeout(timer);current?.interrupt('soak_finished');preflight?.abort('soak_finished');
    process.removeListener('SIGINT',interrupted);process.removeListener('SIGTERM',interrupted);
    if(root){try{requireValue(path.dirname(root)===os.tmpdir()&&!fs.lstatSync(root).isSymbolicLink(),'unsafe_soak_cleanup');fs.rmSync(root,{recursive:true,force:true});report.cleanup='completed';}catch{report.cleanup='failed';error ||= new Failure('soak_cleanup_failed');report.error={code:error.code};}}
  }
  report.comparison=compare(report.arms,options.turns);
  report.status=error?resultStatus(error):report.comparison.status;
  if(error?.code==='soak_time_budget_exceeded')report.status='incomplete';
  report.ok=report.status==='passed';report.finished=new Date().toISOString();save(options.output,report);
  process.stdout.write(JSON.stringify(report,null,2)+'\n');
  return {passed:0,failed:1,blocked:2,incomplete:3}[report.status];
}
module.exports={parse,save,main};
if(require.main===module)main().then(code=>{process.exitCode=code;}).catch(error=>{console.error(JSON.stringify({protocol:'swapctx.codex-soak',version:1,status:'failed',error:{code:error instanceof Failure?error.code:'soak_local_failure'}}));process.exitCode=1;});
