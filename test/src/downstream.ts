import 'reflect-metadata';
import { Controller, Get, Param, Query, Module, Post, Body, HttpException, HttpCode } from '@nestjs/common';
import { NestFactory } from '@nestjs/core';
import { sleep, span } from './shared';
@Controller('internal')
class DownstreamController {
  @Get('inventory/:id') async inventory(@Param('id') id:string,@Query('delay') delay='25') {
    return span('inventory.lookup.mock', async()=>{await sleep(Math.min(Number(delay)||0,2000));return {sku:id,available:true};},{'lab.mock':true});
  }
  @Post('payment') @HttpCode(200) async payment(@Body() payload: { amount:number;mode?:string }) {
    const mode=payload.mode||'success';
    return span('payment.gateway.mock',async()=>{
      if(mode==='slow')await sleep(650);
      else if(mode==='timeout')await sleep(2800);
      else await sleep(35);
      if(mode==='decline')throw new HttpException({code:'PAYMENT_DECLINED'},402);
      if(mode==='error')throw new HttpException({code:'PROVIDER_UNAVAILABLE'},503);
      return {status:'authorized',reference:'synthetic-payment-ref',amount:payload.amount};
    },{'lab.mock':true,'payment.mode':mode});
  }
  @Get('unstable/:n') async unstable(@Param('n') n:string) {
    if(Number(n)<3)throw new HttpException({code:'TEMPORARY_FAILURE',attempt:Number(n)},503);
    return {ok:true,attempt:Number(n)};
  }
}
@Module({controllers:[DownstreamController]})class DownstreamModule{}
async function bootstrap(){const app=await NestFactory.create(DownstreamModule,{logger:['error','warn']});await app.listen(Number(process.env.LAB_DOWNSTREAM_PORT||3901),'127.0.0.1');console.log('Downstream on '+await app.getUrl());}
void bootstrap().catch(e=>{console.error(e);process.exitCode=1;});
