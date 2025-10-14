import { FeedOut } from "../../types/api";
import { formatDateTime } from "../../lib/time";

export type FeedTableProps = {
  feeds: FeedOut[];
  onToggle: (feed: FeedOut, enabled: boolean) => Promise<void> | void;
  onEdit: (feed: FeedOut) => void;
  onDelete: (feed: FeedOut) => Promise<void> | void;
  busyIds: Set<number>;
};

export function FeedTable({ feeds, onToggle, onEdit, onDelete, busyIds }: FeedTableProps) {
  if (feeds.length === 0) {
    return (
      <div className="rounded-md border border-slate-200 bg-white p-6 text-center text-sm text-slate-500">
        暂无订阅源，请先新增一个 RSS 链接。
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-slate-200 bg-white shadow-sm">
      <div className="overflow-x-auto">
        <table className="min-w-full divide-y divide-slate-200 text-sm">
          <thead className="bg-slate-50 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">
          <tr>
            <th className="px-4 py-3">标题</th>
            <th className="hidden md:table-cell px-4 py-3">RSS URL</th>
            <th className="hidden lg:table-cell px-4 py-3">域名</th>
            <th className="px-4 py-3">启用</th>
            <th className="hidden md:table-cell px-4 py-3">最近抓取</th>
            <th className="hidden lg:table-cell px-4 py-3">状态码</th>
            <th className="hidden lg:table-cell px-4 py-3">失败次数</th>
            <th className="px-4 py-3">操作</th>
          </tr>
          </thead>
          <tbody className="divide-y divide-slate-200 bg-white">
          {feeds.map((feed) => {
            const busy = busyIds.has(feed.id);
            const lastFetch = formatDateTime(feed.last_fetch_at ?? undefined);
            return (
              <tr key={feed.id} className="hover:bg-slate-50">
                <td className="px-4 py-3 font-medium text-slate-900">
                  <div className="flex flex-col gap-1">
                    <span>{feed.title || feed.source_domain}</span>
                    <span className="break-all text-xs font-normal text-slate-500 md:hidden">
                      {feed.url}
                    </span>
                    <span className="text-xs font-normal text-slate-500 md:hidden">{feed.source_domain}</span>
                    <span className="text-xs font-normal text-slate-500 md:hidden">
                      最近抓取：{lastFetch || "—"}
                    </span>
                  </div>
                </td>
                <td className="hidden md:table-cell px-4 py-3 text-slate-600">
                  <a href={feed.url} target="_blank" rel="noopener noreferrer" className="hover:underline">
                    {feed.url}
                  </a>
                </td>
                <td className="hidden lg:table-cell px-4 py-3 text-slate-600">{feed.source_domain}</td>
                <td className="px-4 py-3 text-slate-600">
                  <label className="inline-flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={feed.enabled}
                      onChange={(event) => onToggle(feed, event.target.checked)}
                      disabled={busy}
                      className="h-4 w-4 rounded border-slate-300 text-primary focus:ring-primary"
                    />
                    {feed.enabled ? "开启" : "关闭"}
                  </label>
                </td>
                <td className="hidden md:table-cell px-4 py-3 text-slate-600">{lastFetch}</td>
                <td className="hidden lg:table-cell px-4 py-3 text-slate-600">{feed.last_fetch_status ?? "-"}</td>
                <td className="hidden lg:table-cell px-4 py-3 text-slate-600">{feed.fail_count}</td>
                <td className="px-4 py-3">
                  <div className="flex flex-wrap gap-2">
                    <button
                      onClick={() => onEdit(feed)}
                      className="rounded-md border border-slate-300 px-3 py-1 text-xs font-medium text-slate-600 hover:bg-slate-100"
                    >
                      编辑
                    </button>
                    <button
                      onClick={() => onDelete(feed)}
                      disabled={busy}
                      className="rounded-md border border-red-300 px-3 py-1 text-xs font-medium text-red-600 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      删除
                    </button>
                  </div>
                </td>
              </tr>
            );
          })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
