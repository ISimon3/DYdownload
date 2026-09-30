import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { ArrowLeft, ArrowUpRight, Film, Folder, Image, ImageOff, Play, RefreshCw, Search } from 'lucide-react';

type SavedFile = { name: string; path: string; kind: string; size: number };
type SavedAuthor = { name: string; path: string; files: SavedFile[] };
const sizeLabel = (bytes: number) => bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(2)} GB` : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
const authorLabel = (name: string) => name.replace(/__[A-Za-z0-9_-]+$/, '');

function Thumbnail({ file, directory }: { file?: SavedFile; directory: string }) {
  const host = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [source, setSource] = useState('');
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(entries => { if (entries.some(entry => entry.isIntersecting)) { setVisible(true); observer.disconnect(); } }, { rootMargin: '160px' });
    if (host.current) observer.observe(host.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!file || !visible) return;
    let disposed = false;
    setSource(''); setFailed(false);
    invoke<string>('media_thumbnail', { directory, path: file.path })
      .then(value => { if (!disposed) setSource(value); }).catch(() => { if (!disposed) setFailed(true); });
    return () => { disposed = true; };
  }, [file?.path, file?.size, directory, visible]);
  return <div ref={host} className={`media-thumbnail ${file && !source && !failed ? 'shimmer' : ''}`}>
    {source ? <img src={source} alt="" loading="lazy" onError={() => { setSource(''); setFailed(true); }}/> : failed ? <span className="thumbnail-fallback"><ImageOff size={25}/><small>预览不可用</small></span> : !file ? <Folder size={36} strokeWidth={1.3}/> : null}
    {file?.kind === '视频' && source && <span className="thumbnail-play"><Play size={19} fill="currentColor"/></span>}
  </div>;
}

export function SavedLibrary({ directory, revision }: { directory: string; revision: number }) {
  const [authors, setAuthors] = useState<SavedAuthor[]>([]);
  const [selected, setSelected] = useState('');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('全部');
  const [refresh, setRefresh] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [limit, setLimit] = useState(60);
  useEffect(() => { setSelected(''); setQuery(''); setAuthors([]); setFilter('全部'); }, [directory]);
  useEffect(() => {
    let disposed = false;
    setError('');
    if (!directory || !isTauri()) { setLoading(false); return; }
    setLoading(true);
    invoke<SavedAuthor[]>('saved_library', { directory }).then(result => { if (!disposed) setAuthors(result); })
      .catch(e => { if (!disposed) setError(String(e)); }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [directory, revision, refresh]);
  useEffect(() => { setLimit(60); }, [selected, query, filter]);
  const author = authors.find(item => item.path === selected);
  const files = author?.files.filter(file => file.name.toLowerCase().includes(query.toLowerCase()) && (filter === '全部' || file.kind === filter)) ?? [];
  const folders = authors.filter(item => authorLabel(item.name).toLowerCase().includes(query.toLowerCase()));
  const openFolder = (path: string) => { setSelected(path); setQuery(''); setFilter('全部'); };
  async function openPath(path: string) {
    try { await invoke('open_saved_path', { directory, path }); } catch (e) { setError(String(e)); }
  }
  return <section className="saved-page">
    <div className="saved-heading"><div><h1>拾光作品</h1><p className="subtitle">{authors.length} 位作者 · {authors.reduce((sum, item) => sum + item.files.length, 0)} 个媒体文件</p></div><button className="secondary" disabled={loading || !directory || !isTauri()} onClick={() => setRefresh(value => value + 1)}><RefreshCw size={15} className={loading ? 'spin' : ''}/>{loading ? '更新中' : '刷新'}</button></div>
    {error && <div className="notice" role="alert">{error}</div>}
    {!isTauri() ? <div className="queue-empty"><Folder size={42}/><h3>在桌面应用中查看本地作品</h3><p>已保存的作者和作品会显示在这里。</p></div> : <>
      <div className="saved-toolbar"><div className="saved-context">{author ? <><button className="icon-button" aria-label="返回作者列表" onClick={() => openFolder('')}><ArrowLeft size={18}/></button><h2>{authorLabel(author.name)}</h2></> : <h2>我的作者</h2>}</div><label className="saved-search"><Search size={15}/><input aria-label={author ? '搜索文件名' : '搜索作者'} placeholder={author ? '搜索文件名' : '搜索作者'} value={query} onChange={e => setQuery(e.target.value)}/></label>{author && <button className="secondary" onClick={() => openPath(author.path)}><Folder size={15}/>打开文件夹</button>}</div>
      {author && <div className="saved-filter-row"><div className="segmented">{['全部', '图片', '视频'].map(kind => <button key={kind} className={filter === kind ? 'chosen' : ''} aria-pressed={filter === kind} onClick={() => setFilter(kind)}>{kind}</button>)}</div><span>{files.length} 个作品</span></div>}
      {loading && !authors.length ? <div className="media-grid" aria-label="正在读取本地作品" aria-busy="true">{Array.from({ length: 6 }, (_, index) => <div className="media-skeleton" key={index}><div className="shimmer"/><span className="shimmer"/><small className="shimmer"/></div>)}</div> : author ? <>
        {files.length ? <div className="media-grid">{files.slice(0, limit).map(file => <article className="media-card" key={file.path}>
          <button className="media-cover-button" onClick={() => openPath(file.path)} aria-label={`打开 ${file.name}`}><Thumbnail file={file} directory={directory}/><span className="media-type">{file.kind === '视频' ? <Film size={12}/> : <Image size={12}/>} {file.kind}</span></button>
          <div className="media-card-heading"><h3 title={file.name}>{file.name}</h3><button className="icon-button" aria-label={`打开 ${file.name}`} onClick={() => openPath(file.path)}><ArrowUpRight size={15}/></button></div>
          <div className="media-card-meta"><span>{sizeLabel(file.size)}</span></div>
        </article>)}</div> : <div className="queue-empty"><Film size={36}/><h3>{query || filter !== '全部' ? '没有匹配的作品' : '作品保存完成后会显示在这里'}</h3></div>}
        {files.length > limit && <button className="secondary load-more" onClick={() => setLimit(value => value + 60)}>显示更多作品</button>}
      </> : folders.length ? <div className="media-grid author-grid">{folders.map(item => <button className="author-tile" key={item.path} onClick={() => openFolder(item.path)}><Thumbnail file={item.files[0]} directory={directory}/><div className="author-tile-caption"><strong>{authorLabel(item.name)}</strong><span>{item.files.length} 个作品</span></div></button>)}</div> : <div className="queue-empty"><Folder size={42} strokeWidth={1.3}/><h3>{query ? '没有匹配的作者' : '还没有本地作品'}</h3><p>{query ? '试试其他名称。' : '下载作品后会自动出现在这里。'}</p></div>}
    </>}
  </section>;
}
