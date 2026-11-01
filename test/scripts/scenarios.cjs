const http=require('node:http');
const base=`http://127.0.0.1:${process.env.LAB_PORT||3900}`;
const cases={
 fast:['GET','/scenarios/fast'],slow:['GET','/scenarios/slow?ms=400'],error:['GET','/scenarios/error'],
 validation:['GET','/scenarios/validation?count=-1'],parallel:['GET','/scenarios/parallel'],
 sequential:['GET','/scenarios/sequential'],retry:['GET','/scenarios/retry'],timeout:['GET','/scenarios/timeout'],
 nplusone:['GET','/scenarios/n-plus-one?count=12'],cache:['GET','/scenarios/cache'],queue:['GET','/scenarios/queue'],
 cpu:['GET','/scenarios/cpu'],privacy:['GET','/scenarios/privacy'],notfound:['GET','/scenarios/not-found'],
 checkout:['POST','/orders/checkout',{items:['sku-1'],total:49.99}],
 declined:['POST','/orders/checkout',{items:['sku-1'],total:49.99,mode:'decline'}],
 paymenterror:['POST','/orders/checkout',{items:['sku-1'],total:49.99,mode:'error'}]
};
function request([method,route,body]){return new Promise((resolve,reject)=>{
 const payload=body?JSON.stringify(body):null;
 const r=http.request(new URL(route,base),{method,headers:payload?{'content-type':'application/json','content-length':Buffer.byteLength(payload)}:undefined},res=>{
  let text='';res.on('data',c=>text+=c);res.on('end',()=>resolve({status:res.statusCode,text:text.slice(0,140)}));
 });r.on('error',reject);if(payload)r.write(payload);r.end();
});}
(async()=>{const arg=process.argv[2]||'all';const names=arg==='all'?Object.keys(cases):[arg];for(const name of names){if(!cases[name])throw new Error(`Unknown scenario ${name}; available: ${Object.keys(cases).join(', ')}`);const start=Date.now();const r=await request(cases[name]);console.log(`${name.padEnd(15)} HTTP ${r.status} ${Date.now()-start}ms ${r.text}`);}})().catch(e=>{console.error(e);process.exitCode=1;});
