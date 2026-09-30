import { useEffect, useState } from 'react';
import { Clock3, X } from 'lucide-react';

const key = 'shiguang.searchHistory';
function readHistory(): string[] {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(key) || '[]');
    return Array.isArray(saved) ? [...new Set(saved.filter((value): value is string => typeof value === 'string' && value.length > 0 && value.length <= 512))].slice(0, 12) : [];
  } catch { return []; }
}
export function useSearchHistory() {
  const [history, setHistory] = useState(readHistory);
  useEffect(() => {
    try { localStorage.setItem(key, JSON.stringify(history)); } catch { /* History stays usable for this session. */ }
  }, [history]);
  function update(change: (previous: string[]) => string[]) {
    setHistory(change);
  }
  return {
    history,
    remember: (value: string) => update(previous => [value, ...previous.filter(item => item !== value)].slice(0, 12)),
    remove: (value: string) => update(previous => previous.filter(item => item !== value)),
  };
}
export function SearchHistory({ items, disabled, onSearch, onRemove }: { items: string[]; disabled: boolean; onSearch: (value: string) => void; onRemove: (value: string) => void }) {
  if (!items.length) return null;
  return <div className="search-history" aria-label="历史搜索">
    <span className="search-history-label"><Clock3 size={14}/>最近搜索</span>
    <div className="search-history-items">{items.map(value => <span className="search-history-chip" key={value}>
      <button type="button" disabled={disabled} title={value} aria-label={`搜索 ${value}`} onClick={() => onSearch(value)}>{value}</button>
      <button type="button" className="search-history-remove" aria-label={`删除历史 ${value}`} title="删除这条历史" onClick={() => onRemove(value)}><X size={12}/></button>
    </span>)}</div>
  </div>;
}
