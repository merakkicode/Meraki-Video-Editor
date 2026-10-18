import { Show } from 'solid-js';
import type { AnimatedProperty } from '../../../../contracts/ipc';
import { editor, selectedClip } from '../state/editor';
import { rawVisual, changeMotion, changeTitle, toggleKey, hasKey, canKey, keyInterpolation, setKeyInterpolation, volumeValue, changeVolume } from '../state/visual';
import { rational, seconds } from '../lib/time';
export function KeyButton(props:{property:AnimatedProperty}) {
  return <button type="button" class="key-button" classList={{active:hasKey(props.property)}} disabled={!canKey()||editor.busy} aria-label={`Keyframe ${props.property}`} title="Inserir/remover keyframe na agulha" onClick={e=>{e.preventDefault();toggleKey(props.property);}}>◆</button>;
}
export default function AnimationControls(){
  const m=()=>rawVisual().motion;
  const duration=(field:'fadeIn'|'fadeOut'|'dissolveIn',text:string)=>{const value=Number(text);if(Number.isFinite(value)&&value>=0)changeMotion({[field]:rational(BigInt(Math.round(value*1000)),1000n)});};
  const color=()=>`#${m().title!.color.slice(0,3).map(v=>v.toString(16).padStart(2,'0')).join('')}`;
  return <Show when={selectedClip()}><div class="animation-controls">
    <strong>Animação</strong>
    <label>Interpolação dos novos keyframes<select value={keyInterpolation()} onChange={e=>setKeyInterpolation(e.currentTarget.value as 'linear'|'smooth')}><option value="linear">Linear</option><option value="smooth">Suave · Bézier</option></select></label>
    <p>Posicione a agulha dentro do clipe e use ◆. Com uma faixa animada, alterar o valor grava um keyframe naquele instante.</p>
    <label><span>Volume do clipe <KeyButton property="volume"/> {Math.round(volumeValue()*100)}%</span><input aria-label="Volume animado" type="range" min="0" max="200" value={volumeValue()*100} disabled={!canKey()} onInput={e=>changeVolume(Number(e.currentTarget.value)/100)}/></label>
    <Show when={selectedClip()?.source.kind==='video'}>
      <strong>Transições</strong>
      <label>Fade de entrada (s)<input aria-label="Fade de entrada" type="number" step="0.1" min="0" value={seconds(m().fadeIn)} onChange={e=>duration('fadeIn',e.currentTarget.value)}/></label>
      <label>Fade de saída (s)<input aria-label="Fade de saída" type="number" step="0.1" min="0" value={seconds(m().fadeOut)} onChange={e=>duration('fadeOut',e.currentTarget.value)}/></label>
      <label>Dissolvência com anterior (s)<input aria-label="Dissolvência" type="number" step="0.1" min="0" value={seconds(m().dissolveIn)} onChange={e=>duration('dissolveIn',e.currentTarget.value)}/></label>
      <p>Dissolvência centrada no corte. Requer clipes adjacentes e alças disponíveis em ambas as fontes. Use 0 para remover.</p>
    </Show>
    <Show when={m().title}>
      <strong>Título</strong>
      <label>Conteúdo<textarea aria-label="Conteúdo do título" maxLength={1024} value={m().title!.text} onInput={e=>changeTitle({text:e.currentTarget.value})}/></label>
      <label>Fonte<select value={m().title!.font} onChange={e=>changeTitle({font:e.currentTarget.value})}><option>Segoe UI</option><option>Arial</option><option>Georgia</option></select></label>
      <label>Tamanho (px)<input type="number" min="8" max="300" value={m().title!.size} onChange={e=>changeTitle({size:Number(e.currentTarget.value)})}/></label>
      <label>Cor<input aria-label="Cor do título" type="color" value={color()} onInput={e=>{const h=e.currentTarget.value;changeTitle({color:[parseInt(h.slice(1,3),16),parseInt(h.slice(3,5),16),parseInt(h.slice(5,7),16),m().title!.color[3]]});}}/></label>
      <label>Alfa<input aria-label="Alfa do título" type="range" min="0" max="255" value={m().title!.color[3]} onInput={e=>changeTitle({color:[...m().title!.color.slice(0,3),Number(e.currentTarget.value)] as [number,number,number,number]})}/></label>
      <label>Alinhamento<select value={m().title!.align} onChange={e=>changeTitle({align:e.currentTarget.value})}><option value="left">Esquerda</option><option value="center">Centro</option><option value="right">Direita</option></select></label>
      <button aria-pressed={m().title!.shadow} onClick={()=>changeTitle({shadow:!m().title!.shadow})}>Sombra: {m().title!.shadow?'Ativa':'Desativada'}</button>
    </Show>
  </div></Show>;
}
