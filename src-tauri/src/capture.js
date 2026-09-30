(() => {
  if (location.hostname !== 'www.douyin.com' || window.__shiguangInstalled) return;
  window.__shiguangInstalled = true;
  const queryId = new URLSearchParams(location.hash.slice(1)).get('sg') || new URLSearchParams(location.search).get('_sg') || '';
  const searchPath = location.pathname.match(/^\/search\/([^/]+)/)?.[1] || '';
  let input = searchPath;
  try { input = decodeURIComponent(searchPath); } catch {}
  input ||= new URLSearchParams(location.search).get('keyword') || new URLSearchParams(location.search).get('q') || '';
  const profileId = location.pathname.startsWith('/user/') ? location.pathname.slice(6).split('/')[0] : '';
  let resolved = false, lastPostUrl = '', cursor = '', hasMore = true, loading = false;
  const send = (kind, data, message = '') => {
    if (!queryId) return;
    const payload = JSON.stringify({ queryId, kind, data, message });
    if (payload.length > 8_000_000) return;
    return window.__TAURI_INTERNALS__?.invoke('capture_works', { payload }).catch(error => console.warn('拾光数据接收失败', String(error)));
  };
  const navigate = async (secUid, user) => {
    if (resolved || !/^[A-Za-z0-9_-]{10,200}$/.test(secUid)) return;
    resolved = true;
    // Navigating immediately can cancel the IPC request from this document.
    // A DOM-only match also supplies the verified ID while profile data loads.
    await send('author', user || { sec_uid: secUid, unique_id: input });
    location.assign(`https://www.douyin.com/user/${secUid}?_sg=${encodeURIComponent(queryId)}#sg=${encodeURIComponent(queryId)}`);
  };
  const matches = user => input && [user?.unique_id, user?.short_id, user?.uid].some(id => id != null && String(id) === input);
  const requestPath = url => {
    try { return new URL(typeof url === 'string' ? url : url?.url || String(url), location.origin).pathname; }
    catch { return ''; }
  };
  const relevant = url => /\/aweme\/v1\/web\/(?:aweme\/post|user\/profile|discover\/search|search\/user|general\/search\/single)(?:\/|$)/.test(requestPath(url));
  const deliver = (url, data) => {
    if (!data || typeof data !== 'object') return;
    const path = requestPath(url);
    if (/\/aweme\/v1\/web\/aweme\/post(?:\/|$)/.test(path)) {
      if (!Array.isArray(data.aweme_list)) return;
      const requestProfile = new URL(url, location.origin).searchParams.get('sec_user_id');
      if (!profileId || (requestProfile && requestProfile !== profileId)) return;
      if (data.aweme_list.some(work => work.author?.sec_uid ? work.author.sec_uid !== profileId : requestProfile !== profileId)) return;
      lastPostUrl = new URL(url, location.origin).href;
      cursor = String(data.max_cursor ?? '');
      hasMore = data.has_more !== 0 && data.has_more !== false;
      // The profile endpoint is not guaranteed on every navigation (page cache,
      // response order, or platform layout). Posts carry the same author data.
      const author = data.aweme_list.find(work => work.author)?.author;
      if (author) send('author', { ...author, sec_uid: profileId });
      send('works', data);
    } else if (/\/aweme\/v1\/web\/user\/profile\/other(?:\/|$)/.test(path)) {
      const requestProfile = new URL(url, location.origin).searchParams.get('sec_user_id');
      if (profileId && data.user && (data.user.sec_uid === profileId || (!data.user.sec_uid && requestProfile === profileId))) send('author', { ...data.user, sec_uid: profileId });
    } else if (input) {
      const source = [data.user_list, data.data?.user_list, data.data, data.users].find(Array.isArray) || [];
      const users = source.map(item => item.user_info || item.user || item);
      const exact = users.filter(matches);
      if (exact.length === 1 && exact[0].sec_uid) navigate(exact[0].sec_uid, exact[0]);
    }
  };
  const originalFetch = window.fetch;
  window.fetch = async function (...args) {
    const response = await originalFetch.apply(this, args);
    const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || String(args[0]);
    if (relevant(url)) response.clone().json().then(data => deliver(url, data)).catch(() => {});
    return response;
  };
  const originalOpen = XMLHttpRequest.prototype.open, originalSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (...args) { this.__captureUrl = String(args[1]); return originalOpen.apply(this, args); };
  XMLHttpRequest.prototype.send = function (...args) {
    this.addEventListener('load', () => {
      if (!relevant(this.__captureUrl)) return;
      try { deliver(this.__captureUrl, this.responseType === 'json' ? this.response : JSON.parse(this.responseText)); } catch {}
    }, { once: true });
    return originalSend.apply(this, args);
  };
  function scrollMore() {
    document.querySelectorAll('*').forEach(element => {
      if (element.scrollHeight > element.clientHeight + 100 && ['auto', 'scroll'].includes(getComputedStyle(element).overflowY)) element.scrollTo({ top: element.scrollHeight, behavior: 'instant' });
    });
    window.scrollTo({ top: document.body?.scrollHeight || 0, behavior: 'instant' });
  }
  window.__shiguangLoadMore = async () => {
    if (!profileId || !lastPostUrl || !hasMore || loading) return;
    loading = true;
    try {
      if (lastPostUrl && cursor) {
        const url = new URL(lastPostUrl);
        url.searchParams.set('max_cursor', cursor);
        url.searchParams.delete('a_bogus'); url.searchParams.delete('X-Bogus');
        // Use the current page fetch so Douyin's own request middleware can sign it.
        const response = await window.fetch(url.href, { credentials: 'include', signal: AbortSignal.timeout(15000) });
        const data = await response.clone().json().catch(() => null);
        if (!Array.isArray(data?.aweme_list)) scrollMore();
      } else scrollMore();
    } catch { scrollMore(); } finally { loading = false; }
  };
  if (queryId && input) {
    let attempts = 0;
    const timer = setInterval(() => {
      if (resolved || !location.pathname.startsWith('/search/')) { clearInterval(timer); return; }
      // DOM fallback keeps exact Douyin-ID matching working when the search API changes.
      const candidates = Array.from(document.querySelectorAll('a[href*="/user/"]')).filter(anchor => {
        // textContent joins the ID and the following statistics without spaces.
        // Read rendered lines, then individual ID labels, never a numeric prefix.
        const texts = [anchor.innerText || '', ...Array.from(anchor.querySelectorAll?.('p,span') || []).map(element => element.textContent || '')];
        return texts.some(text => {
          const match = text.match(/抖音号\s*[:：]\s*([A-Za-z0-9_.-]+)/);
          return match?.[1] === input;
        });
      });
      const ids = new Set(candidates.map(anchor => {
        try { const url = new URL(anchor.href); return url.hostname === 'www.douyin.com' ? url.pathname.split('/user/')[1]?.split('/')[0] : ''; } catch { return ''; }
      }).filter(Boolean));
      if (ids.size === 1) navigate([...ids][0]);
      if (++attempts >= 40) { clearInterval(timer); send('error', null, '作者查询暂未返回结果，请重试。若仍失败，请重新连接抖音账号后查询。'); }
    }, 1000);
  }
})();
