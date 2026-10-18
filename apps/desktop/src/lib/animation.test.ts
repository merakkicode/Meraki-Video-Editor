import { expect,test } from 'vitest';
import {evaluateKeys,addTime,compareTime} from './animation';
import type {KeyframeDto} from '../../../../contracts/ipc';
test('Bezier UI evaluation and rational split offset agree at exact timestamps',()=>{
 const keys:KeyframeDto[]=[{at:{numerator:'0',denominator:1},value:1,interpolation:'smooth'},{at:{numerator:'2',denominator:1},value:1.5,interpolation:'linear'}];
 expect(evaluateKeys(keys,{numerator:'1',denominator:1},0)).toBe(1.25);
 expect(evaluateKeys(keys,{numerator:'1',denominator:2},0)).toBe(1.078125);
 const large={numerator:'9007199254740993',denominator:1};
 expect(compareTime(addTime(large,{numerator:'1',denominator:3}),large)).toBe(1);
});
