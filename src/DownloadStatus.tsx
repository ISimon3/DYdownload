import { useEffect, useState } from 'react';
import { ArrowDownToLine, Check, ChevronRight, LoaderCircle, TriangleAlert } from 'lucide-react';

type Job = { work: { id: string; title: string }; status: string; done: number; total: number; message: string };
export function DownloadStatus({ jobs, loading, onOpen }: { jobs: Job[]; loading: boolean; onOpen: () => void }) {
  const [hidden, setHidden] = useState(false);
  const active = jobs.filter(job => job.status === 'queued' || job.status === 'downloading');
  const running = active.filter(job => job.status === 'downloading');
  const done = jobs.filter(job => job.status === 'done').length;
  const failed = jobs.filter(job => job.status === 'error').length;
  const busy = active.length > 0 || loading;
  const outcome = jobs.map(job => `${job.work.id}:${job.status}`).join('|');
  useEffect(() => {
    setHidden(false);
    if (busy || !outcome) return;
    const timer = setTimeout(() => setHidden(true), failed ? 6500 : 3500);
    return () => clearTimeout(timer);
  }, [busy, outcome, failed]);
  if ((!jobs.length && !loading) || (hidden && !busy)) return null;
  return <aside className="download-status" aria-label="下载进度">
    <button className="download-status-summary" onClick={onOpen} aria-label="查看下载队列">
      {busy ? <LoaderCircle size={17} className="spin"/> : failed ? <TriangleAlert size={17}/> : <Check size={17}/>}
      <span><strong>{busy ? `下载中 · ${active.length} 个待完成` : failed ? `${failed} 个下载失败` : '下载队列已结束'}</strong><small>{done} / {jobs.length} 个已完成{loading ? ' · 正在读取更多作品' : failed ? ` · ${failed} 个失败` : ''}</small></span><ChevronRight size={15}/>
    </button>
    {running.map(job => <div className="download-status-item" key={job.work.id}>
      <div><ArrowDownToLine size={12}/><span title={job.work.title}>{job.work.title || job.work.id}</span><b>{job.total > 0 ? `${Math.min(100, Math.floor(job.done / job.total * 100))}%` : '处理中'}</b></div>
      <div className={`progress ${job.total ? '' : 'indeterminate'}`} role="progressbar" aria-label={job.work.title || job.work.id} aria-valuemin={0} aria-valuemax={100} aria-valuenow={job.total ? Math.min(100, Math.floor(job.done / job.total * 100)) : undefined}><i style={{ width: job.total ? `${Math.min(100, job.done / job.total * 100)}%` : '30%' }}/></div>
      <small>{job.message || '正在连接媒体服务器…'}</small>
    </div>)}
  </aside>;
}
