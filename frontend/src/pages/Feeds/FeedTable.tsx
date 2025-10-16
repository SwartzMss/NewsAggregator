import { FeedOut } from "../../types/api";
import { formatDateTime } from "../../lib/time";

export type FeedTableProps = {
  feeds: FeedOut[];
  onToggle: (feed: FeedOut, enabled: boolean) => Promise<void> | void;
  onEdit: (feed: FeedOut) => void;
  onDelete: (feed: FeedOut) => Promise<void> | void;
  onTest?: (feed: FeedOut) => Promise<void> | void;
  busyIds: Set<number>;
  testingIds?: Set<number>;
  emptyMessage?: string;
};

export function FeedTable({
  feeds,
  onToggle,
  onEdit,
  onDelete,
  onTest,
  busyIds,
  testingIds,
  emptyMessage,
}: FeedTableProps) {
  if (feeds.length === 0) {
    return (
      <div className="rounded-md border border-slate-200 bg-white p-6 text-center text-sm text-slate-500">
        {emptyMessage ?? "暂无订阅源，请先新增一个 RSS 链接。"}
      </div>
    );
  }

  const testingSet = testingIds ?? new Set<number>();

  return (
    <div className="rounded-lg border border-slate-200 bg-white shadow-sm">
      <div className="overflow-x-auto">
        <table className="min-w-full divide-y divide-slate-200 text-sm">
          <thead className="bg-slate-50 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">
            <tr>
              <th className="px-4 py-3">订阅源</th>
              <th className="hidden md:table-cell px-4 py-3">最新抓取</th>
              <th className="hidden lg:table-cell px-4 py-3">状态</th>
              <th className="hidden lg:table-cell px-4 py-3">失败次数</th>
              <th className="px-4 py-3">启用</th>
              <th className="px-4 py-3 text-right">操作</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-200 bg-white">
            {feeds.map((feed) => {
              const busy = busyIds.has(feed.id);
              const testing = testingSet.has(feed.id);
              const lastFetch = formatDateTime(feed.last_fetch_at ?? undefined);
              const status = getStatusBadge(feed);

              return (
                <tr key={feed.id} className="align-top hover:bg-slate-50">
                  <td className="px-4 py-3">
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-slate-900">
                          {feed.title || feed.source_domain}
                        </span>
                        <span
                          className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium ${
                            feed.enabled
                              ? "bg-emerald-50 text-emerald-700"
                              : "bg-slate-100 text-slate-500"
                          }`}
                        >
                          {feed.enabled ? "启用" : "停用"}
                        </span>
                      </div>
                      <a
                        href={feed.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="break-all text-xs text-primary hover:underline"
                      >
                        {feed.url}
                      </a>
                      <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-slate-500">
                        <span>来源：{feed.source_domain}</span>
                        {feed.language && <span>语言：{feed.language}</span>}
                        <span>间隔：{formatInterval(feed.fetch_interval_seconds)}</span>
                      </div>
                    </div>
                  </td>
                  <td className="hidden md:table-cell px-4 py-3 text-xs text-slate-600">
                    {lastFetch ? lastFetch : "尚未抓取"}
                  </td>
                  <td className="hidden lg:table-cell px-4 py-3">
                    <span
                      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${status.className}`}
                      title={status.tooltip}
                    >
                      {status.label}
                    </span>
                  </td>
                  <td className="hidden lg:table-cell px-4 py-3 text-xs text-rose-600">
                    {feed.fail_count > 0 ? (
                      <span className="inline-flex items-center rounded-full bg-rose-50 px-2 py-0.5 font-medium">
                        {feed.fail_count}
                      </span>
                    ) : (
                      <span className="text-slate-400">0</span>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <label className="inline-flex items-center gap-2 text-sm text-slate-600">
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
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap justify-end gap-2">
                      {onTest && (
                        <button
                          onClick={() => onTest(feed)}
                          disabled={testing}
                          className="rounded-md border border-primary px-3 py-1 text-xs font-medium text-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          {testing ? "测试中…" : "测试抓取"}
                        </button>
                      )}
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

function getStatusBadge(feed: FeedOut) {
  const status = feed.last_fetch_status ?? undefined;
  if (typeof status === "number") {
    if (status >= 200 && status < 300) {
      return {
        label: `成功 ${status}`,
        className: "bg-emerald-50 text-emerald-700",
        tooltip: "最近一次抓取成功",
      };
    }
    if (status >= 300 && status < 400) {
      return {
        label: `重定向 ${status}`,
        className: "bg-amber-50 text-amber-700",
        tooltip: "最近一次抓取发生重定向",
      };
    }
    if (status >= 400) {
      return {
        label: `失败 ${status}`,
        className: "bg-rose-50 text-rose-700",
        tooltip: "最近一次抓取返回错误状态码",
      };
    }
  }

  if (feed.fail_count > 0) {
    return {
      label: "近期失败",
      className: "bg-rose-50 text-rose-700",
      tooltip: "最近抓取多次失败，请检查 RSS 状态",
    };
  }

  return {
    label: "等待抓取",
    className: "bg-slate-100 text-slate-600",
    tooltip: "尚未抓取或没有可用的状态信息",
  };
}

function formatInterval(seconds: number) {
  if (seconds <= 0) {
    return "—";
  }
  const minutes = Math.max(Math.round(seconds / 60), 1);
  if (minutes < 60) {
    return `${minutes} 分钟`;
  }
  const hours = minutes / 60;
  if (hours < 24) {
    return `${hours % 1 === 0 ? hours : hours.toFixed(1)} 小时`;
  }
  const days = hours / 24;
  return `${days % 1 === 0 ? days : days.toFixed(1)} 天`;
}
