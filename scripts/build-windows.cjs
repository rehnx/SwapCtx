'use strict';
// Offline release recipe. Two fresh source/target directories; no agent processes.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
const {inspect} = require('./inspect-pe.cjs');
const {inputs:sourceInputs,tools} = require('./release-inputs.cjs');
const root = path.resolve(__dirname,'..');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
// Bounded diagnostics only. Equality still compares the complete file hashes.
function describeBinary(bytes) {
  const pe = inspect(bytes), header = bytes.readUInt32LE(0x3c);
  const optional = header+24, sections = [], table = optional+bytes.readUInt16LE(header+20);
  for (let index=0; index<bytes.readUInt16LE(header+6); index++) {
    const entry=table+index*40, size=bytes.readUInt32LE(entry+16), offset=bytes.readUInt32LE(entry+20);
    if (offset+size>bytes.length) throw new Error('Invalid diagnostic section bounds');
    sections.push({name:bytes.toString('ascii',entry,entry+8).replace(/\0/g,''),offset,bytes:size,
      sha256:hash(bytes.subarray(offset,offset+size))});
  }
  return {sha256:hash(bytes),bytes:bytes.length,coff_timestamp:bytes.readUInt32LE(header+8),
    checksum:bytes.readUInt32LE(optional+64),sections,pe};
}
function describeDifference(first,second) {
  const ranges=[];let differingBytes=0,start;
  for (let index=0;index<=Math.max(first.length,second.length);index++) {
    const different=index<Math.max(first.length,second.length) && first[index]!==second[index];
    if(different) {differingBytes++;if(start===undefined) start=index;}
    else if(start!==undefined) {if(ranges.length<32) ranges.push({offset:start,bytes:index-start});start=undefined;}
  }
  return {first:describeBinary(first),second:describeBinary(second),differingBytes,firstDifferenceRanges:ranges};
}
function build(destination, test = false) {
  if (process.platform !== 'win32') throw new Error('Windows release builds require Windows');
  const output = path.resolve(destination), parent = path.dirname(output);
  if (!fs.statSync(parent).isDirectory() || fs.existsSync(output)) throw new Error('Choose a NEW destination with an existing parent');
  const version = fs.readFileSync(path.join(root,'rust-toolchain.toml'),'utf8').match(/channel\s*=\s*"([^"]+)"/)[1];
  const homes = [version+'-x86_64-pc-windows-msvc','stable-x86_64-pc-windows-msvc'];
  const rustup = process.env.RUSTUP_HOME || path.join(process.env.USERPROFILE,'.rustup');
  const rustBin = homes.map(home=>path.join(rustup,'toolchains',home,'bin')).find(dir=> {
    const result = spawnSync(path.join(dir,'rustc.exe'),['--version'],{encoding:'utf8'});
    return result.status === 0 && result.stdout.startsWith(`rustc ${version} `);
  });
  if (!rustBin) throw new Error(`Install the pinned Rust ${version} Windows MSVC toolchain`);
  for (const key of ['CL','_CL_','LINK','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER']) if (process.env[key]) throw new Error(`Unset build override ${key} for this release recipe`);
  const localCache = path.join(root,'.build/cargo-home');
  const cache = path.resolve(process.env.CARGO_HOME || (fs.existsSync(localCache) ? localCache : path.join(process.env.USERPROFILE,'.cargo')));
  const env = {...process.env,PATH:rustBin+path.delimiter+process.env.PATH,CARGO_HOME:cache,CARGO_NET_OFFLINE:'true',CARGO_INCREMENTAL:'0',
    RUSTC:path.join(rustBin,'rustc.exe'),RUSTDOC:path.join(rustBin,'rustdoc.exe'),SOURCE_DATE_EPOCH:'946684800',CFLAGS:'/Brepro'};
  delete env.RUSTFLAGS;
  const inputs = sourceInputs(root), buildTools = tools(root);
  const stage = fs.mkdtempSync(path.join(parent,'.swapctx-build-'));
  const stageIdentity = fs.lstatSync(stage);
  let work, workIdentity, failure;
  const binaryHashes = [], binaries = [];
  function run(args, cwd, buildEnv = env) {
    console.log('cargo '+args.join(' '));
    const result = spawnSync(path.join(rustBin,'cargo.exe'),args,{cwd,env:buildEnv,stdio:'inherit'});
    if (result.error || result.status !== 0) throw new Error(`Offline build failed (${result.status}): ${result.error?.message || 'see Cargo diagnostic'}`);
  }
  try {
    // MSVC's C compiler can reject long /Fo paths even when Node/.NET accept
    // them. Keep fresh compilation trees out of nested preview staging paths.
    // Final assembly remains beside the destination for same-volume publication.
    work = fs.mkdtempSync(path.join(os.tmpdir(),'swapctx-compile-'));
    workIdentity = fs.lstatSync(work);
    for (const name of ['a','b']) {
      const sourceAlias = path.join(work,'source-'+name), targetAlias = path.join(work,'target-'+name);
      fs.mkdirSync(sourceAlias);fs.mkdirSync(targetAlias);
      // Cargo/rustc can resolve Windows 8.3 paths to their long spelling.
      // Compile from canonical paths and remap both spellings, including OUT_DIR.
      const source=fs.realpathSync.native(sourceAlias),target=fs.realpathSync.native(targetAlias);
      for (const input of inputs) {
        const bytes = fs.readFileSync(path.join(root,input.path));
        if (hash(bytes) !== input.sha256) throw new Error(`Source changed during build: ${input.path}`);
        const file = path.join(source,input.path);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,bytes);
      }
      const flags = ['-Ctarget-feature=+crt-static','-Clink-arg=/Brepro','-Cdebuginfo=0'];
      for(const [aliases,destination] of [
        [[sourceAlias,source],'/swapctx'],[[cache,fs.realpathSync.native(cache)],'/cargo'],
        [[targetAlias,target],'/target'],
      ]) for(const prefix of new Set(aliases.flatMap(value=>[value,path.toNamespacedPath(value)]))) {
        flags.push(`--remap-path-prefix=${prefix}=${destination}`);
      }
      const buildEnv = {...env,CARGO_ENCODED_RUSTFLAGS:flags.join('\x1f')};
      const common = ['--offline','--locked','--release','--target','x86_64-pc-windows-msvc','--target-dir',target];
      run(['build',...common],source,buildEnv);
      const binary = path.join(target,'x86_64-pc-windows-msvc/release/swapctx.exe');
      const beforeTests=fs.readFileSync(binary);
      console.log(`Build ${name} before native tests: ${hash(beforeTests)}`);
      if (test && name === 'a') run(['test',...common,'--all-targets'],source,buildEnv);
      const afterTests=fs.readFileSync(binary);
      console.log(`Build ${name} after native tests: ${hash(afterTests)}`);
      if(hash(beforeTests)!==hash(afterTests)) {
        console.error('Native test build changed the production executable: '+JSON.stringify(describeDifference(beforeTests,afterTests)));
      }
      binaries.push(binary);binaryHashes.push(hash(fs.readFileSync(binary)));
    }
    if (binaryHashes[0] !== binaryHashes[1]) {
      try {console.error('Independent build diagnostics: '+JSON.stringify(describeDifference(fs.readFileSync(binaries[0]),fs.readFileSync(binaries[1]))));}
      catch {console.error('Build mismatch diagnostics unavailable; full-hash comparison still failed.');}
      throw new Error(`Builds differ: ${binaryHashes.join(' != ')}; no candidate published`);
    }
    const pe = inspect(fs.readFileSync(binaries[0]));
    if (pe.dynamic_crt_imports.length) throw new Error('Release still imports a dynamic C runtime: '+pe.dynamic_crt_imports.join(', '));
    const payload = path.join(stage,'output');fs.mkdirSync(payload);
    fs.copyFileSync(binaries[0],path.join(payload,'swapctx.exe'));
    const compiler = spawnSync(env.RUSTC,['-vV'],{encoding:'utf8'});
    if (compiler.status !== 0) throw new Error('Cannot record compiler identity');
    const receipt = {format_version:1,target:'x86_64-pc-windows-msvc',compiler:compiler.stdout.trim().split(/\r?\n/),
      recipe:'static-crt-brepro-v1',recipe_sha256:hash(fs.readFileSync(__filename)),tools:buildTools,inputs,
      source_fingerprint:hash(Buffer.from(JSON.stringify(inputs))),binary_sha256:binaryHashes[0],
      independent_binary_hashes:binaryHashes,reproducible_on_this_toolchain:true,native_release_tests_run:test,pe,
      limits:['Local unsigned build receipt, not authenticated provenance','Same machine, compiler, cached dependencies and native toolchain; cross-machine reproducibility unverified','PE import inspection does not prove clean-machine compatibility']};
    fs.writeFileSync(path.join(payload,'build-info.json'),JSON.stringify(receipt,null,2)+'\n');
    fs.renameSync(payload,output);
    console.log(`Verified matching offline builds: ${binaryHashes[0]}\nOutput: ${output}`);
    return receipt;
  } catch (error) {
    failure = error;
    throw error;
  } finally {
    for (const [directory,owner,expectedParent,prefix] of [
      [work,workIdentity,path.resolve(os.tmpdir()),'swapctx-compile-'],
      [stage,stageIdentity,parent,'.swapctx-build-'],
    ]) {
      if (!directory) continue;
      try {
        const actual = fs.lstatSync(directory);
        if (path.dirname(directory) !== expectedParent || !path.basename(directory).startsWith(prefix)
            || actual.isSymbolicLink() || actual.dev !== owner.dev || actual.ino !== owner.ino) throw new Error('Unsafe cleanup path');
        fs.rmSync(directory,{recursive:true,force:true});
      } catch (error) {
        if (!failure) failure = error;
        else console.error('Additional build cleanup failure; originating error retained.');
      }
    }
    // Preserve a build error; cleanup failures after success must also fail closed.
    if (failure) throw failure;
  }
}
if (require.main === module) {
  if (!process.argv[2] || process.argv.slice(3).some(arg=>arg!=='--test')) throw new Error('Usage: node scripts/build-windows.cjs NEW_OUTPUT_DIRECTORY [--test]');
  build(process.argv[2],process.argv.includes('--test'));
}
module.exports = {build,describeBinary,describeDifference};
