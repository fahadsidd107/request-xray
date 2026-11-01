const {test,before,after}=require('node:test');
const assert=require('node:assert/strict');
const {spawn}=require('node:child_process');
const path=require('node:path');
const base=path.resolve(__dirname,'..');
const port=3950+Math.floor(Math.random()*100);
let proc;
async function wait(url,ms=12000){const start=Date.now();while(Date.now()-start<ms){try{const r=await fetch(url);if(r.ok)return;}catch{}await new Promise(r=>setTimeout(r,120));}throw new Error('Server did not start: '+url);}
before(async()=>{
 proc=spawn(process.execPath,['scripts/run-lab.cjs'],{cwd:base,env:{...process.env,XRAY_MODE:'off',LAB_PORT:String(port),LAB_DOWNSTREAM_PORT:String(port+1)},stdio:'pipe'});
 await wait(`http://127.0.0.1:${port}/health`);
 await wait(`http://127.0.0.1:${port+1}/internal/inventory/check`);
});
after(()=>{if(proc)proc.kill('SIGTERM');});
async function get(url){return fetch(`http://127.0.0.1:${port}${url}`);}
test('health and fast request',async()=>{assert.equal((await get('/health')).status,200);assert.equal((await get('/scenarios/fast')).status,200);});
test('expected errors and validation',async()=>{assert.equal((await get('/scenarios/error')).status,500);assert.equal((await get('/scenarios/validation?count=-1')).status,400);});
test('cross-process fanout, retry and simulated queue',async()=>{assert.equal((await get('/scenarios/parallel')).status,200);const r=await(await get('/scenarios/retry')).json();assert.deepEqual(r.attempts,[503,503,200]);assert.equal((await get('/scenarios/queue')).status,200);});
test('checkout success and declined payment',async()=>{
 const post=(mode)=>fetch(`http://127.0.0.1:${port}/orders/checkout`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({items:['sku-1'],total:49.99,mode})});
 assert.equal((await post('success')).status,201);assert.equal((await post('decline')).status,502);
});
test('downstream timeout is reported',async()=>{assert.equal((await get('/scenarios/timeout')).status,504);});
