import { trace, SpanStatusCode } from '@opentelemetry/api';
import * as http from 'node:http';
const tracer = trace.getTracer('request-xray-lab', '0.1.0');
export const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
export async function span<T>(name: string, operation: () => Promise<T>, attributes: Record<string,string|number|boolean>={}): Promise<T> {
  return tracer.startActiveSpan(name, { attributes }, async s => {
    try { return await operation(); }
    catch(e) { s.recordException(e instanceof Error ? e : String(e)); s.setStatus({code:SpanStatusCode.ERROR}); throw e; }
    finally { s.end(); }
  });
}
/** Use Node http to exercise standard incoming/outgoing HTTP auto-instrumentation. */
export function remote(path: string, method='GET', payload?: unknown, timeoutMs=4000): Promise<{status:number;data:unknown}> {
  const base = `http://127.0.0.1:${Number(process.env.LAB_DOWNSTREAM_PORT||3901)}`;
  return new Promise((resolve,reject) => {
    const data = payload === undefined ? undefined : JSON.stringify(payload);
    const req=http.request(new URL(path,base),{method,timeout:timeoutMs,headers:data?{'content-type':'application/json','content-length':Buffer.byteLength(data)}:undefined},res=>{
      const chunks:Buffer[]=[];res.on('data',(c:Buffer)=>chunks.push(c));res.on('end',()=>{
        const raw=Buffer.concat(chunks).toString('utf8');let parsed:unknown=raw;
        try { parsed=JSON.parse(raw); } catch { /* strings are valid */ }
        resolve({status:res.statusCode||0,data:parsed});
      });
    });
    req.on('timeout',()=>req.destroy(new Error(`Downstream HTTP timeout after ${timeoutMs}ms`)));
    req.on('error',reject);if(data)req.write(data);req.end();
  });
}
