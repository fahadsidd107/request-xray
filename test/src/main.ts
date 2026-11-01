import 'reflect-metadata';
import { Body, Controller, Get, HttpException, Module, Param, Post, Query } from '@nestjs/common';
import { NestFactory } from '@nestjs/core';
import { randomUUID } from 'node:crypto';
import { remote, sleep, span } from './shared';
type Checkout = {items?:string[];total?:number;mode?:string};
@Controller()
class LabController {
  @Get('health') health(){return {ok:true,service:'checkout',mockData:true};}
  @Get('scenarios/fast') async fast(){return span('scenario.fast',async()=>{await sleep(8);return {ok:true};});}
  @Get('scenarios/slow') async slow(@Query('ms') ms='500') {return span('scenario.slow',async()=>{await sleep(Math.min(Math.max(Number(ms)||500,0),2500));return {ok:true};});}
  @Get('scenarios/error') async error(){return span('scenario.error',async()=>{throw new HttpException({code:'SYNTHETIC_FAILURE'},500);});}
  @Get('scenarios/validation') validation(@Query('count') count?:string) {
    const n=Number(count);if(!count||!Number.isInteger(n)||n<1)throw new HttpException('count must be a positive integer',400);
    return {count:n};
  }
  @Get('scenarios/parallel') async parallel(){return span('scenario.fanout',async()=>{
    const values=await Promise.all([remote('/internal/inventory/a?delay=120'),remote('/internal/inventory/b?delay=180'),remote('/internal/inventory/c?delay=80')]);
    return {statuses:values.map(v=>v.status)};
  });}
  @Get('scenarios/sequential') async sequential(){return span('scenario.sequential',async()=>{
    const result=[];for(const sku of ['a','b','c'])result.push((await remote(`/internal/inventory/${sku}?delay=80`)).status);
    return {statuses:result};
  });}
  @Get('scenarios/retry') async retry(){return span('scenario.retry',async()=>{
    const attempts=[];for(let i=1;i<=3;i++){
      const r=await span('retry.attempt',()=>remote(`/internal/unstable/${i}`),{'retry.attempt':i});
      attempts.push(r.status);if(r.status===200)break;await sleep(i*40);
    }
    return {attempts};
  });}
  @Get('scenarios/timeout') async timeout(){return span('scenario.timeout',async()=>{
    try {await remote('/internal/payment','POST',{amount:42,mode:'timeout'},120);return {unexpected:true};}
    catch(e){throw new HttpException({code:'DOWNSTREAM_TIMEOUT',detail:e instanceof Error?e.message:'timeout'},504);}
  });}
  @Get('scenarios/n-plus-one') async nPlusOne(@Query('count') count='12'){
    const n=Math.min(Math.max(Number(count)||12,1),40);
    return span('scenario.n_plus_one.mock',async()=>{for(let i=0;i<n;i++)await span('db.query.mock.select_orders',async()=>sleep(5),{'lab.mock':true,'db.statement.template':'SELECT orders WHERE customer_id = ?'});return {queries:n,simulated:true};});
  }
  @Get('scenarios/cache') async cache(){return span('scenario.cache.mock',async()=>{
    await span('redis.get.mock',async()=>sleep(8),{'lab.mock':true,'cache.hit':false});
    await span('db.query.mock',async()=>sleep(55),{'lab.mock':true});
    await span('redis.set.mock',async()=>sleep(6),{'lab.mock':true});
    return {hit:false,simulated:true};
  });}
  @Get('scenarios/queue') async queue(){return span('scenario.queue.mock',async()=>{
    await span('message.publish.mock',async()=>sleep(12),{'lab.mock':true,'messaging.destination.name':'orders'});
    await span('message.consume.mock',async()=>sleep(34),{'lab.mock':true});
    return {simulated:true,warning:'This is not a real Kafka/RabbitMQ integration'};
  });}
  @Get('scenarios/cpu') cpu(){const start=Date.now();let total=0;while(Date.now()-start<75)total+=Math.sqrt(total+1);return {blockedMs:Date.now()-start,computed:Number.isFinite(total)};}
  @Get('scenarios/privacy') privacy(){return {safe:true,message:'No sensitive input returned or recorded intentionally'};}
  @Get('scenarios/not-found') notFound(){throw new HttpException({code:'SIMULATED_NOT_FOUND'},404);}
  @Post('orders/checkout') async checkout(@Body() order:Checkout){
    if(!Array.isArray(order.items)||order.items.length===0||typeof order.total!=='number'||!Number.isFinite(order.total)||order.total<=0)throw new HttpException('items and positive numeric total required',400);
    return span('checkout.process',async()=>{
      const id=randomUUID();
      await span('order.insert.mock',async()=>sleep(18),{'lab.mock':true,'order.item_count':order.items!.length});
      const inventory=await remote('/internal/inventory/sku-1');
      if(inventory.status!==200)throw new HttpException({code:'INVENTORY_UNAVAILABLE'},503);
      const payment=await remote('/internal/payment','POST',{amount:order.total,mode:order.mode||'success'},3000);
      if(payment.status!==200)throw new HttpException({code:'PAYMENT_REJECTED',downstreamStatus:payment.status},502);
      await span('order.complete.mock',async()=>sleep(9),{'lab.mock':true});
      return {orderId:id,status:'completed',payment:payment.data,simulated:true};
    },{'checkout.item_count':order.items.length});
  }
}
@Module({controllers:[LabController]})class LabModule{}
async function bootstrap(){const app=await NestFactory.create(LabModule,{logger:['error','warn']});await app.listen(Number(process.env.LAB_PORT||3900),'127.0.0.1');console.log('Checkout app on '+await app.getUrl());}
void bootstrap().catch(e=>{console.error(e);process.exitCode=1;});
