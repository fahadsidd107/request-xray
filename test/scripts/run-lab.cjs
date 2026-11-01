const { spawn }=require('node:child_process');
const path=require('node:path');
const mode=process.argv.includes('--otel')?'otel':process.argv.includes('--xray')?'xray':(process.env.XRAY_MODE||'off');
const children=[];
for(const [service,file] of [['lab-downstream','downstream.js'],['lab-checkout','main.js']]){
 const child=spawn(process.execPath,['--require',path.resolve(__dirname,'../dist/instrumentation/register.js'),path.resolve(__dirname,'../dist',file)],{
  cwd:path.resolve(__dirname,'..'),env:{...process.env,XRAY_MODE:mode,OTEL_SERVICE_NAME:service},stdio:'inherit'
 });children.push(child);
 child.on('exit',(code)=>{if(code && !closing){console.error(`${service} exited: ${code}`);shutdown(code);}});
}
let closing=false;
function shutdown(code=0){if(closing)return;closing=true;for(const c of children)if(!c.killed)c.kill('SIGTERM');setTimeout(()=>process.exit(code),500).unref();}
process.on('SIGINT',()=>shutdown());process.on('SIGTERM',()=>shutdown());
console.log(`Request X-Ray lab mode=${mode}; upstream=${process.env.LAB_PORT||3900} downstream=${process.env.LAB_DOWNSTREAM_PORT||3901}`);
