export function WorkLoading({ loading, count, hasMore }: { loading: boolean; count: number; hasMore: boolean }) {
  return <div className={`work-loading ${loading ? 'is-loading' : ''}`} role="status" aria-live="polite">
    <div className="work-loading-copy"><span>{loading ? <><i className="loading-dots"><b/><b/><b/></i>正在整理作品</> : hasMore ? '读取已暂停' : '作品已就绪'}</span><small>已发现 <strong>{count}</strong> 个作品</small></div>
    {loading && <div className="work-loading-track"><i/></div>}
  </div>;
}
