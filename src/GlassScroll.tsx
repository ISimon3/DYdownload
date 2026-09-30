import { useEffect, useRef, useState } from 'react';
import { ArrowUp, CornerDownLeft } from 'lucide-react';

export function GlassScroll({ scope, jumpButtons }: { scope: string; jumpButtons: boolean }) {
  const [metrics,setMetrics]=useState({top:0,max:0,height:0});
  const [returnTo,setReturnTo]=useState<number|null>(null);
  const drag=useRef<{y:number;top:number;max:number;travel:number}|null>(null);
  useEffect(()=>setReturnTo(null),[scope]);
  useEffect(()=>{
    let frame=0;
    const update=()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(()=>setMetrics({top:window.scrollY,max:Math.max(0,document.documentElement.scrollHeight-window.innerHeight),height:window.innerHeight}));};
    const observer=new ResizeObserver(update);observer.observe(document.body);
    window.addEventListener('scroll',update,{passive:true});window.addEventListener('resize',update);update();
    return()=>{observer.disconnect();cancelAnimationFrame(frame);window.removeEventListener('scroll',update);window.removeEventListener('resize',update);};
  },[]);
  const trackHeight=Math.max(0,metrics.height-72);
  const thumbHeight=Math.min(trackHeight,Math.max(48,trackHeight*metrics.height/(metrics.height+metrics.max)));
  const travel=Math.max(0,trackHeight-thumbHeight);
  const thumbTop=metrics.max?metrics.top/metrics.max*travel:0;
  const smooth=()=>matchMedia('(prefers-reduced-motion: reduce)').matches?'instant' as const:'smooth' as const;
  return <>
    {metrics.max>1&&<div className="glass-scroll-track" role="scrollbar" aria-label="页面滚动" aria-controls="workspace-content" aria-orientation="vertical" aria-valuemin={0} aria-valuemax={Math.round(metrics.max)} aria-valuenow={Math.round(metrics.top)} tabIndex={0}
      onKeyDown={e=>{const moves:Record<string,number>={ArrowDown:60,ArrowUp:-60,PageDown:metrics.height*.8,PageUp:-metrics.height*.8};if(e.key in moves){e.preventDefault();window.scrollBy({top:moves[e.key]});}else if(e.key==='Home'||e.key==='End'){e.preventDefault();window.scrollTo({top:e.key==='Home'?0:metrics.max});}}}
      onPointerDown={e=>{if(e.target!==e.currentTarget)return;const rect=e.currentTarget.getBoundingClientRect();window.scrollTo({top:Math.max(0,Math.min(1,(e.clientY-rect.top-thumbHeight/2)/Math.max(1,travel)))*metrics.max,behavior:smooth()});}}>
      <div className="glass-scroll-thumb" style={{height:thumbHeight,transform:`translateY(${thumbTop}px)`}}
        onPointerDown={e=>{e.preventDefault();e.stopPropagation();e.currentTarget.setPointerCapture(e.pointerId);drag.current={y:e.clientY,top:window.scrollY,max:metrics.max,travel};}}
        onPointerMove={e=>{const d=drag.current;if(d)window.scrollTo({top:d.top+(e.clientY-d.y)/Math.max(1,d.travel)*d.max,behavior:'instant'});}}
        onPointerUp={e=>{drag.current=null;if(e.currentTarget.hasPointerCapture(e.pointerId))e.currentTarget.releasePointerCapture(e.pointerId);}}
        onPointerCancel={()=>{drag.current=null;}} onLostPointerCapture={()=>{drag.current=null;}}/>
    </div>}
    {jumpButtons&&(metrics.max>1||returnTo!==null)&&<div className="scroll-jumps glass">
      <button aria-label="回到顶部" title="回到顶部" disabled={metrics.top<80} onClick={()=>{setReturnTo(window.scrollY);window.scrollTo({top:0,behavior:smooth()});}}><ArrowUp size={19}/></button>
      <button aria-label="返回刚刚位置" title="返回刚刚位置" disabled={returnTo===null} onClick={()=>{if(returnTo!==null)window.scrollTo({top:Math.min(returnTo,metrics.max),behavior:smooth()});}}><CornerDownLeft size={19}/></button>
    </div>}
  </>;
}
