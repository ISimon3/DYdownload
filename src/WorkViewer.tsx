import { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { ArrowDownToLine, ChevronLeft, ChevronRight, Image, LoaderCircle, Music2, Pause, Play, X } from 'lucide-react';

type Work = { id: string; title: string; author: string; cover: string; kind: string; created: number; likes: number };
type Details = { imageCount: number; mediaKinds: string[]; hasAudio: boolean; hasMusic: boolean; musicTitle: string };
export function WorkViewer({ work, initialMode, directory, onClose, onDownload }: { work: Work; initialMode: 'play'|'details'; directory: string; onClose: () => void; onDownload: () => void }) {
  const mode = initialMode;
  const [details, setDetails] = useState<Details|null>(null);
  const [index, setIndex] = useState(0);
  const [cover, setCover] = useState(false);
  const [imageSrc, setImageSrc] = useState('');
  const [videoSrc, setVideoSrc] = useState('');
  const [audioSrc, setAudioSrc] = useState('');
  const [imageBusy, setImageBusy] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState('');
  const [audioError, setAudioError] = useState('');
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);
  const [retry, setRetry] = useState(0);
  const dialog = useRef<HTMLElement>(null);
  const audio = useRef<HTMLAudioElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const count = details?.mediaKinds?.length ?? details?.imageCount ?? 0;
  const album = work.kind === '图集' || work.kind === 'Live';
  const currentVideo = !album || details?.mediaKinds?.[index] === 'video';
  const showVideo = currentVideo && !cover && (mode === 'play' || work.kind === 'Live');
  const mediaActive = !cover && (mode === 'play' || work.kind === 'Live');
  const hasMusic = album && details?.hasMusic;
  const move = (step: number) => { setCover(false); setIndex(current => (current + step + count) % Math.max(1, count)); };
  useEffect(() => {
    const previous = document.activeElement as HTMLElement|null;
    const overflow = document.body.style.overflow; document.body.style.overflow = 'hidden'; dialog.current?.focus();
    return () => { document.body.style.overflow = overflow; previous?.focus(); };
  }, []);
  useEffect(() => {
    let disposed = false; setError('');
    invoke<Details>('work_details', { id: work.id }).then(value => { if (!disposed) setDetails(value); }).catch(e => { if (!disposed) setError(String(e)); });
    return () => { disposed = true; };
  }, [work.id, retry]);
  useEffect(() => {
    if (!album || currentVideo || !count || cover) return;
    let disposed = false; setImageBusy(true); setError(''); setImageSrc('');
    invoke<string>('prepare_work_media', { id: work.id, kind: 'image', index }).then(path => { if (!disposed) setImageSrc(convertFileSrc(path)); })
      .catch(e => { if (!disposed) setError(String(e)); }).finally(() => { if (!disposed) setImageBusy(false); });
    return () => { disposed = true; };
  }, [work.id, album, currentVideo, count, index, cover, retry]);
  useEffect(() => {
    if (!showVideo) return;
    let disposed = false; setError(''); setVideoSrc('');
    invoke<string>('prepare_work_media', { id: work.id, kind: 'video', index }).then(path => { if (!disposed) setVideoSrc(convertFileSrc(path)); }).catch(e => { if (!disposed) setError(String(e)); });
    return () => { disposed = true; };
  }, [work.id, showVideo, index, retry]);
  useEffect(() => {
    if (!hasMusic) return;
    let disposed = false; setAudioError(''); setAudioSrc('');
    invoke<string>('prepare_work_media', { id: work.id, kind: 'audio', index: 0 }).then(path => { if (!disposed) setAudioSrc(convertFileSrc(path)); }).catch(e => { if (!disposed) setAudioError(String(e)); });
    return () => { disposed = true; };
  }, [work.id, hasMusic, retry]);
  useEffect(() => {
    if (!mediaActive) { audio.current?.pause(); setPlaying(false); return; }
    if (audioSrc && (!showVideo || (video.current && !video.current.paused))) playMusic();
  }, [mediaActive, audioSrc, showVideo]);
  useEffect(() => {
    if (mode !== 'play' || currentVideo || !playing || count < 2 || imageBusy || !imageSrc) return;
    const timer = setTimeout(() => setIndex(value => (value + 1) % count), 3500);
    return () => clearTimeout(timer);
  }, [mode, currentVideo, playing, count, imageBusy, imageSrc, index]);
  function playMusic() {
    audio.current?.play().catch(() => { setPlaying(false); setMessage('点击配乐播放按钮开启声音'); });
  }
  async function saveAudio() {
    if (!directory) { setMessage('请先在偏好设置中选择保存目录'); return; }
    setSaving(true); setMessage('正在下载音频…');
    try { setMessage(await invoke<string>('download_work_audio', { id: work.id, directory })); }
    catch (e) { setMessage(String(e)); }
    finally { setSaving(false); }
  }
  function keys(event: React.KeyboardEvent) {
    if (event.key === 'Escape') { event.stopPropagation(); onClose(); }
    if (!['INPUT', 'VIDEO', 'AUDIO'].includes((event.target as HTMLElement).tagName) && album && count > 1) { if (event.key === 'ArrowLeft') move(-1); if (event.key === 'ArrowRight') move(1); }
    if (event.key === 'Tab') {
      const items = Array.from(dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled),audio[controls],video[controls],[tabindex="0"]') ?? []).filter(item => item.getClientRects().length > 0);
      const first = items[0], last = items[items.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
  }
  return <div className={`modal-backdrop ${mode === 'play' ? 'player-backdrop' : ''}`} onClick={onClose}><section ref={dialog} tabIndex={-1} className={`work-viewer ${mode === 'play' ? `fullscreen-player ${currentVideo ? 'current-video' : ''}` : 'glass'}`} role="dialog" aria-modal="true" aria-label={mode === 'play' ? '媒体播放' : '作品详情'} onClick={e => e.stopPropagation()} onKeyDown={keys}>
    <header className="viewer-header"><div><h2>{mode === 'play' ? '媒体播放' : '作品详情'}</h2><span>{work.kind}</span></div><button className="icon-button" aria-label="关闭媒体窗口" onClick={onClose}><X size={21}/></button></header>
    <div className="viewer-stage">
      {showVideo ? videoSrc ? <video ref={video} key={`${work.id}-${index}`} src={videoSrc} controls autoPlay playsInline loop={mode === 'details' || count <= 1} muted={Boolean(audioSrc && !audioError && playing)} onPlay={() => { if (audioSrc) playMusic(); }} onPause={() => { if (!video.current?.ended) audio.current?.pause(); }} onEnded={() => { if (count > 1) move(1); }} poster={work.cover || undefined} onError={() => setError('视频无法播放，请重新加载或下载后播放')}/> : <div className="viewer-loading"><LoaderCircle size={28} className="spin"/><span>正在准备视频…</span></div> : album && !currentVideo && !cover ? imageSrc ? <img src={imageSrc} alt={`图片 ${index + 1}`} onError={() => setError('图片加载失败，请重试')}/> : <div className="viewer-loading">{imageBusy || !details ? <LoaderCircle size={28} className="spin"/> : <Image size={32}/>}<span>{count || !details ? '正在加载图片…' : '此作品没有可用的图片'}</span></div> : work.cover ? <img src={work.cover} alt="作品封面" referrerPolicy="no-referrer"/> : <Image size={38}/>}
      {album && !cover && count > 1 && <><button className="viewer-arrow prev" aria-label="上一张" onClick={() => move(-1)}><ChevronLeft size={26}/></button><button className="viewer-arrow next" aria-label="下一张" onClick={() => move(1)}><ChevronRight size={26}/></button><span className="viewer-counter">{index + 1} / {count}</span></>}
    </div>
    {error && <div className="viewer-message" role="alert">{error}<button onClick={() => setRetry(value => value + 1)}>重新加载</button></div>}
    {album && <div className="viewer-gallery-controls">{mode === 'details' ? <div className="segmented"><button className={cover ? 'chosen' : ''} onClick={() => setCover(true)}>封面</button><button className={!cover ? 'chosen' : ''} onClick={() => setCover(false)}>{work.kind === 'Live' ? 'Live / 图片' : '图片'} {count > 0 && `(${count})`}</button></div> : !currentVideo && <button className="secondary" onClick={() => { if (audio.current) { if (playing) audio.current.pause(); else audio.current.play().catch(() => setMessage('音频无法播放，请重试')); } else setPlaying(value => !value); }}>{playing ? <Pause size={15}/> : <Play size={15}/>} {playing ? '暂停播放' : '开始播放'}</button>}<div className="viewer-dots">{Array.from({ length: count }, (_, position) => <button key={position} className={position === index && !cover ? 'current' : ''} aria-label={`媒体 ${position + 1}`} aria-pressed={position === index && !cover} onClick={() => { setCover(false); setIndex(position); }}/>)}</div></div>}
    {album && <div className="viewer-audio"><span><Music2 size={15}/>{details?.musicTitle || '作品配乐'}</span>{audioSrc ? <audio ref={audio} src={audioSrc} controls loop onPlay={() => { setPlaying(true); setMessage(''); if (showVideo && video.current?.paused) video.current.play().catch(() => setMessage('点击视频播放按钮继续播放')); }} onPause={() => { setPlaying(false); if (showVideo && video.current && !video.current.paused) video.current.pause(); }} onError={() => { setAudioError('配乐无法播放，请重新加载'); setPlaying(false); }}/> : <small>{audioError || (details ? details.hasMusic ? '正在准备配乐…' : currentVideo ? '使用视频原声' : '此作品未提供可用的配乐' : '正在读取配乐信息…')}</small>}{audioError && audioSrc && <small role="alert">{audioError}<button onClick={() => setRetry(value => value + 1)}>重试配乐</button></small>}</div>}
    <div className="viewer-info"><h3>{work.title || '未命名作品'}</h3><p>{work.author} · {new Date(work.created * 1000).toLocaleDateString('zh-CN')} · {work.likes} 次点赞</p></div>
    <div className="viewer-actions"><button className="primary" onClick={onDownload}><ArrowDownToLine size={16}/>下载此作品</button><button className="secondary" disabled={!details?.hasAudio || saving} onClick={saveAudio}>{saving ? <LoaderCircle size={16} className="spin"/> : <Music2 size={16}/>}下载音频</button></div>
    {message && <p className="viewer-message" role="status">{message}</p>}
  </section></div>;
}
