'use strict';
// Explicit x64 MSVC/SDK selection. No shell, developer-script execution or env dump.
const fs=require('node:fs');
const path=require('node:path');
const {spawnSync}=require('node:child_process');
const {hash}=require('./release-inputs.cjs');
function selectVersion(versions,requested) {
  const valid=versions.filter(v=>/^\d+(?:\.\d+){2,3}$/.test(v));
  if(requested) {
    if(!valid.includes(requested)) throw new Error('Requested native toolchain version is not installed');
    return requested;
  }
  if(!valid.length) throw new Error('No complete native toolchain version is installed');
  return valid.sort((a,b)=> {
    const x=a.split('.').map(Number),y=b.split('.').map(Number);
    for(let i=0;i<Math.max(x.length,y.length);i++) if((x[i]||0)!==(y[i]||0)) return (y[i]||0)-(x[i]||0);
    return 0;
  })[0];
}
function select(baseEnv=process.env) {
  if(process.platform!=='win32') throw new Error('Native toolchain selection requires Windows');
  const programFiles=baseEnv['ProgramFiles(x86)'];
  if(!programFiles || !path.isAbsolute(programFiles)) throw new Error('Missing trusted ProgramFiles(x86)');
  const vswhere=path.join(programFiles,'Microsoft Visual Studio/Installer/vswhere.exe');
  const located=spawnSync(vswhere,['-latest','-products','*','-requires','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','-property','installationPath'],{encoding:'utf8',timeout:30000,maxBuffer:16384});
  if(located.error || located.status!==0) throw new Error('Cannot locate installed MSVC using trusted vswhere');
  const installation=located.stdout.trim();
  if(!path.isAbsolute(installation) || !fs.statSync(installation).isDirectory()) throw new Error('Invalid MSVC installation path');
  const vc=path.join(installation,'VC'),msvcRoot=path.join(vc,'Tools/MSVC');
  const msvcVersion=selectVersion(fs.readdirSync(msvcRoot),baseEnv.SWAPCTX_MSVC_VERSION);
  const msvc=path.join(msvcRoot,msvcVersion),bin=path.join(msvc,'bin/Hostx64/x64');
  const sdk=path.join(programFiles,'Windows Kits/10');
  const sdkVersion=selectVersion(fs.readdirSync(path.join(sdk,'Lib')).filter(version=>
    fs.existsSync(path.join(sdk,'Lib',version,'um/x64/kernel32.lib')) &&
    fs.existsSync(path.join(sdk,'Lib',version,'ucrt/x64/libucrt.lib')) &&
    fs.existsSync(path.join(sdk,'Include',version,'um/Windows.h'))),baseEnv.SWAPCTX_WINDOWS_SDK_VERSION);
  const include=path.join(sdk,'Include',sdkVersion),lib=path.join(sdk,'Lib',sdkVersion);
  const linker=path.join(bin,'link.exe'),compiler=path.join(bin,'cl.exe'),archiver=path.join(bin,'lib.exe');
  const artifacts=[];
  for(const relative of ['cl.exe','link.exe','lib.exe','c1.dll','c1xx.dll','c2.dll','mspdbcore.dll']) {
    const file=path.join(bin,relative);
    if(fs.existsSync(file)) artifacts.push({role:'msvc/'+relative,sha256:hash(fs.readFileSync(file))});
    else if(relative.endsWith('.exe')) throw new Error('Incomplete native toolchain: '+relative);
  }
  for(const [role,file] of [
    ['msvc/libcmt.lib',path.join(msvc,'lib/x64/libcmt.lib')],
    ['msvc/libvcruntime.lib',path.join(msvc,'lib/x64/libvcruntime.lib')],
    ['sdk/kernel32.lib',path.join(lib,'um/x64/kernel32.lib')],
    ['sdk/libucrt.lib',path.join(lib,'ucrt/x64/libucrt.lib')],
  ]) artifacts.push({role,sha256:hash(fs.readFileSync(file))});
  const identity={msvc_version:msvcVersion,windows_sdk_version:sdkVersion,artifacts};
  const env={...baseEnv,
    PATH:[bin,path.join(sdk,'bin',sdkVersion,'x64'),baseEnv.PATH].join(path.delimiter),
    INCLUDE:[path.join(msvc,'include'),...['ucrt','shared','um','winrt'].map(p=>path.join(include,p))].join(path.delimiter),
    LIB:[path.join(msvc,'lib/x64'),path.join(lib,'ucrt/x64'),path.join(lib,'um/x64')].join(path.delimiter),
    LIBPATH:path.join(msvc,'lib/x64'),VCINSTALLDIR:vc+path.sep,VCToolsInstallDir:msvc+path.sep,
    VCToolsVersion:msvcVersion,WindowsSdkDir:sdk+path.sep,WindowsSDKVersion:sdkVersion+path.sep,
    UniversalCRTSdkDir:sdk+path.sep,UCRTVersion:sdkVersion,
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER:linker,
    CC_x86_64_pc_windows_msvc:compiler,AR_x86_64_pc_windows_msvc:archiver,
  };
  return {env,receipt:{identity,identity_sha256:hash(Buffer.from(JSON.stringify(identity))),
    paths:{compiler,linker,archiver},host:{image:baseEnv.ImageOS||null,image_version:baseEnv.ImageVersion||null,node:process.version},
    limits:['Selected tool identities and key runtime libraries, not an exhaustive SDK/header inventory','Local unsigned observations; not authenticated provenance']}};
}
module.exports={select,selectVersion};
