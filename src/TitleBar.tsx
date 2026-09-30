import { useEffect, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Copy, Minus, Square, X } from 'lucide-react';
import appIcon from '../src-tauri/icons/app.svg';

export function TitleBar({ dark, onError }: { dark: boolean; onError: (error: unknown) => void }) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!isTauri()) return;
    const current = getCurrentWindow();
    let disposed = false;
    const update = () => { void current.isMaximized().then(value => { if (!disposed) setMaximized(value); }).catch(() => {}); };
    update();
    const subscription = current.onResized(update);
    return () => { disposed = true; void subscription.then(unlisten => unlisten()).catch(() => {}); };
  }, []);
  async function action(kind: 'minimize' | 'maximize' | 'close') {
    if (!isTauri()) return;
    try {
      const current = getCurrentWindow();
      if (kind === 'close') await current.close();
      else if (kind === 'minimize') await current.minimize();
      else { await current.toggleMaximize(); setMaximized(await current.isMaximized()); }
    } catch (error) { onError(error); }
  }
  return <header className={`app-titlebar ${dark ? 'dark' : ''}`} aria-label="窗口标题栏">
    <div className="titlebar-drag" data-tauri-drag-region>
      <img src={appIcon} alt="" draggable={false}/><span>拾光</span><small>SHIGUANG</small>
    </div>
    <div className="window-controls">
      <button aria-label="最小化窗口" title="最小化" onClick={() => void action('minimize')}><Minus size={15}/></button>
      <button aria-label={maximized ? '还原窗口' : '最大化窗口'} title={maximized ? '还原' : '最大化'} onClick={() => void action('maximize')}>{maximized ? <Copy size={12}/> : <Square size={12}/>}</button>
      <button className="window-close" aria-label="关闭程序" title="关闭" onClick={() => void action('close')}><X size={17}/></button>
    </div>
  </header>;
}
