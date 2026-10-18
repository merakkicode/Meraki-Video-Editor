import type { MotionDto, KeyframeDto, TimeDto } from '../../../../contracts/ipc';
import { rational, seconds } from './time';
export const zeroTime = ():TimeDto => ({numerator:'0',denominator:1});
export const defaultMotion = ():MotionDto => ({offset:zeroTime(),keys:{},fadeIn:zeroTime(),fadeOut:zeroTime(),dissolveIn:zeroTime(),title:null});
export const addTime=(a:TimeDto,b:TimeDto,sign=1n)=>rational(BigInt(a.numerator)*BigInt(b.denominator)+sign*BigInt(b.numerator)*BigInt(a.denominator),BigInt(a.denominator)*BigInt(b.denominator));
export function compareTime(a:TimeDto,b:TimeDto){const d=BigInt(a.numerator)*BigInt(b.denominator)-BigInt(b.numerator)*BigInt(a.denominator);return d<0n?-1:d>0n?1:0;}
export function evaluateKeys(keys:KeyframeDto[],at:TimeDto,base:number){
  if(!keys.length)return base;
  if(compareTime(at,keys[0].at)<=0)return keys[0].value;
  const i=keys.findIndex(k=>compareTime(k.at,at)>0);if(i<0)return keys[keys.length-1].value;
  const a=keys[i-1],b=keys[i];let u=seconds(addTime(at,a.at,-1n))/seconds(addTime(b.at,a.at,-1n));
  if(a.interpolation==='smooth')u=u*u*(3-2*u);return a.value+(b.value-a.value)*u;
}
