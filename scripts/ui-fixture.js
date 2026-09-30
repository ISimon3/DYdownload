// Development-only fixture: open /scripts/ui-fixture.html through Vite.
// No native IPC, network download, or filesystem operation is performed.
const callbacks = new Map(), listeners = new Map(), active = new Map();
let serial = 0, started = 0, cancelled = 0, maximized = false;
const thumbnail = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="640"><rect width="480" height="640" fill="#c2d5df"/><circle cx="355" cy="145" r="52" fill="#fff1c8"/><path d="M0 410L180 195L390 450L480 315V640H0Z" fill="#829c96"/><path d="M0 540L170 410L320 500L480 395V640H0Z" fill="#547970"/></svg>');
const status = () => { document.querySelector('#fixture-status').textContent = `模拟数据 · 启动 ${started} · 取消 ${cancelled} · 活动 ${active.size}`; };
const emit = (event, payload) => callbacks.get(listeners.get(event))?.({ event, payload });
const works = Array.from({ length: 5 }, (_, index) => ({ id: String(100 + index), title: `测试作品 ${index + 1}`, author: '山野记录', authorId: 'test-author', cover: thumbnail, kind: index === 0 ? '图集' : index === 1 ? 'Live' : '视频', created: 1700000000, likes: 10, duration: 30000 }));
window.isTauri = true;
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: 'main' } },
  convertFileSrc(path) { return path; },
  transformCallback(callback) { callbacks.set(++serial, callback); return serial; },
  async invoke(command, args = {}) {
    if (command === 'plugin:window|is_maximized') return maximized;
    if (command === 'plugin:window|toggle_maximize') { maximized = !maximized; return; }
    if (command === 'plugin:event|listen') { listeners.set(args.event, args.handler); return args.handler; }
    if (command === 'work_details') return { imageCount: args.id === '100' ? 3 : 0, mediaKinds: args.id === '100' ? ['image','image','image'] : args.id === '101' ? ['video','image','video'] : ['video'], hasAudio: true, hasMusic: true, musicTitle: '测试配乐' };
    if (command === 'prepare_work_media') { await new Promise(resolve => setTimeout(resolve, 150)); return args.kind === 'image' ? thumbnail.replace('%23c2d5df', args.index === 1 ? '%23dfc2d5' : args.index === 2 ? '%23d5dfc2' : '%23c2d5df') : args.kind === 'audio' ? '/scripts/fixtures/music.mp3' : '/scripts/fixtures/player.mp4'; }
    if (command === 'download_work_audio') return '音频已保存到作者文件夹（MP3）';
    if (command === 'session_status') return true;
    if (command === 'default_directory') return 'F:\\拾光测试';
    if (command === 'query_author') {
      emit('author', { queryId: args.queryId, author: { uid: '123', secUid: 'test-author', uniqueId: 'test', name: '山野记录', signature: '模拟作者，用于界面回归测试', followers: 10, following: 3, likes: 20, workCount: 5 } });
      emit('works', { queryId: args.queryId, works, hasMore: new URLSearchParams(location.search).has('loading') });
    }
    if (command === 'start_download') { started++; return new Promise((resolve, reject) => { active.set(args.id, { resolve, reject }); emit('download-progress', { id: args.id, status: 'downloading', done: 42, total: 100, message: '第 1 / 3 个文件' }); status(); if (new URLSearchParams(location.search).has('complete')) setTimeout(() => { if (!active.has(args.id)) return; active.delete(args.id); emit('download-progress', { id: args.id, status: 'done', done: 1, total: 1, message: '下载完成' }); resolve(); status(); }, 1500); }); }
    if (command === 'cancel_download') { const task = active.get(args.id); if (task) { cancelled++; active.delete(args.id); task.reject('已取消下载'); status(); } }
    if (command === 'saved_library') {
      await new Promise(resolve => setTimeout(resolve, 250));
      return [{ name: '山野记录__MS4wLjABAAAApfs0j9gNBMwZXeTlnyPANEDX', path: 'F:\\拾光测试\\山野记录__test-author', files: ['100_1.mp4', '101_1.webp', '102_1.jpg'].map(name => ({ name, path: `F:\\拾光测试\\山野记录__test-author\\${name}`, kind: name.endsWith('mp4') ? '视频' : '图片', size: 2345678 })) }];
    }
    if (command === 'media_thumbnail') { await new Promise(resolve => setTimeout(resolve, 350)); return thumbnail; }
    if (command === 'open_saved_path') document.querySelector('#fixture-status').textContent = `模拟打开：${args.path}`;
  }
};
await import('../src/main.tsx');
