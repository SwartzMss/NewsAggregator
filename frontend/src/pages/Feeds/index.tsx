import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteFeed,
  listFeeds,
  upsertFeed,
  testFeed,
  UnauthorizedError,
} from "../../lib/api";
import { FeedOut, FeedUpsertPayload } from "../../types/api";
import { FeedTable } from "./FeedTable";
import { FeedFormModal } from "./FeedFormModal";

type FeedsPageProps = {
  token: string;
  onUnauthorized: () => void;
  showHeader?: boolean;
};

type StatusFilter = "all" | "enabled" | "disabled" | "failing";

type FeedbackState =
  | {
      type: "success" | "error" | "info";
      message: string;
    }
  | null;

export function FeedsPage({
  token,
  onUnauthorized,
  showHeader = true,
}: FeedsPageProps) {
  const queryClient = useQueryClient();
  const feedsQuery = useQuery<FeedOut[], Error>({
    queryKey: ["feeds", token],
    queryFn: () => listFeeds(token),
    enabled: Boolean(token),
    retry: false,
  });

  useEffect(() => {
    if (feedsQuery.error instanceof UnauthorizedError) {
      onUnauthorized();
    }
  }, [feedsQuery.error, onUnauthorized]);

  const [modalOpen, setModalOpen] = useState(false);
  const [editingFeed, setEditingFeed] = useState<FeedOut | null>(null);
  const [busyIds, setBusyIds] = useState<Set<number>>(new Set());
  const [testingIds, setTestingIds] = useState<Set<number>>(new Set());
  const [feedback, setFeedback] = useState<FeedbackState>(null);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");

  useEffect(() => {
    if (!feedback) return;
    const timer = window.setTimeout(() => setFeedback(null), 5000);
    return () => window.clearTimeout(timer);
  }, [feedback]);

  const resetModal = () => {
    setEditingFeed(null);
    setModalOpen(false);
  };

  const invalidateFeeds = () =>
    queryClient.invalidateQueries({ queryKey: ["feeds", token] });

  const upsertMutation = useMutation<FeedOut, Error, FeedUpsertPayload>({
    mutationFn: async (payload: FeedUpsertPayload) => {
      const response = await upsertFeed(token, payload);
      return response;
    },
    onSuccess: () => {
      setFeedback({ type: "success", message: "订阅源已保存" });
      invalidateFeeds();
    },
    onError: (err: Error) => {
      if (err instanceof UnauthorizedError) {
        onUnauthorized();
        return;
      }
      setFeedback({
        type: "error",
        message: err.message || "保存订阅源失败",
      });
    },
  });

  const deleteMutation = useMutation<void, Error, FeedOut>({
    mutationFn: async (feed: FeedOut) => {
      setBusyIds((prev) => new Set(prev).add(feed.id));
      await deleteFeed(token, feed.id);
    },
    onSuccess: () => {
      setFeedback({ type: "success", message: "订阅源已删除" });
      invalidateFeeds();
    },
    onError: (err: Error) => {
      if (err instanceof UnauthorizedError) {
        onUnauthorized();
        return;
      }
      setFeedback({
        type: "error",
        message: err.message || "删除订阅源失败",
      });
    },
    onSettled: (_data, _error, feed) => {
      if (feed) {
        setBusyIds((prev) => {
          const next = new Set(prev);
          next.delete(feed.id);
          return next;
        });
      }
    },
  });

  const handleToggle = async (feed: FeedOut, enabled: boolean) => {
    setBusyIds((prev) => new Set(prev).add(feed.id));
    try {
      await upsertMutation.mutateAsync({
        id: feed.id,
        url: feed.url,
        source_domain: feed.source_domain,
        enabled,
      });
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(feed.id);
        return next;
      });
    }
  };

  const handleDelete = async (feed: FeedOut) => {
    if (!confirm(`确认删除订阅源 ${feed.title ?? feed.source_domain} 吗？`)) {
      return;
    }
    await deleteMutation.mutateAsync(feed);
  };

  const handleOpenModal = (feed?: FeedOut) => {
    setEditingFeed(feed ?? null);
    setModalOpen(true);
  };

  const handleTestFeed = async (feed: FeedOut) => {
    setTestingIds((prev) => new Set(prev).add(feed.id));
    setFeedback(null);
    try {
      const result = await testFeed(token, feed.url);
      const summary =
        result.entry_count > 0
          ? `抓取成功：${result.status}，解析到 ${result.entry_count} 条内容`
          : `抓取成功：${result.status}，未返回可用条目`;
      setFeedback({
        type: "info",
        message: `${feed.title ?? feed.source_domain} ${summary}`,
      });
      invalidateFeeds();
    } catch (err) {
      setFeedback({
        type: "error",
        message:
          (err as Error).message ||
          `${feed.title ?? feed.source_domain} 测试抓取失败`,
      });
    } finally {
      setTestingIds((prev) => {
        const next = new Set(prev);
        next.delete(feed.id);
        return next;
      });
    }
  };

  const feeds = feedsQuery.data ?? [];
  const busySet = useMemo(() => busyIds, [busyIds]);
  const testingSet = useMemo(() => testingIds, [testingIds]);

  const stats = useMemo(() => {
    const total = feeds.length;
    const enabled = feeds.filter((feed) => feed.enabled).length;
    const failing = feeds.filter((feed) => {
      const status = feed.last_fetch_status ?? undefined;
      if (typeof status === "number" && status >= 400) return true;
      return feed.fail_count > 0;
    }).length;
    return { total, enabled, failing };
  }, [feeds]);

  const filteredFeeds = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    return feeds.filter((feed) => {
      if (statusFilter === "enabled" && !feed.enabled) {
        return false;
      }
      if (statusFilter === "disabled" && feed.enabled) {
        return false;
      }
      if (statusFilter === "failing") {
        const status = feed.last_fetch_status ?? undefined;
        const failing =
          (typeof status === "number" && status >= 400) || feed.fail_count > 0;
        if (!failing) {
          return false;
        }
      }

      if (!keyword) return true;
      const haystack = [
        feed.title ?? "",
        feed.source_domain ?? "",
        feed.url ?? "",
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(keyword);
    });
  }, [feeds, search, statusFilter]);

  const emptyMessage =
    filteredFeeds.length === 0
      ? feeds.length === 0
        ? "暂无订阅源，请先新增一个 RSS 链接。"
        : "没有符合当前筛选条件的订阅源。"
      : undefined;

  const statusFilters: Array<{ key: StatusFilter; label: string; count?: number }> =
    [
      { key: "all", label: "全部", count: stats.total },
      { key: "enabled", label: "启用中", count: stats.enabled },
      {
        key: "disabled",
        label: "已停用",
        count: Math.max(stats.total - stats.enabled, 0),
      },
      { key: "failing", label: "抓取异常", count: stats.failing },
    ];

  return (
    <div className="space-y-6">
      <div
        className={
          showHeader
            ? "flex flex-wrap items-center justify-between gap-3"
            : "flex flex-wrap items-center justify-between gap-3 rounded-lg border border-slate-200 bg-white px-4 py-3 shadow-sm"
        }
      >
        <div className="flex flex-1 items-center gap-2 rounded-md border border-slate-200 bg-white px-3 py-2 shadow-sm sm:max-w-sm">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            className="h-4 w-4 text-slate-400"
          >
            <path d="m21 21-4.35-4.35" />
            <circle cx="11" cy="11" r="6" />
          </svg>
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="搜索订阅源（标题 / 域名 / URL）"
            className="w-full border-0 bg-transparent text-sm text-slate-600 placeholder:text-slate-400 focus:outline-none focus:ring-0"
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              className="text-xs text-slate-400 hover:text-slate-600"
            >
              清除
            </button>
          )}
        </div>
        <button
          onClick={() => handleOpenModal()}
          className="inline-flex items-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-primary-dark"
        >
          + 新增订阅源
        </button>
      </div>

      <div className="grid gap-3 sm:grid-cols-3">
        <SummaryTile label="订阅源总数" value={stats.total} />
        <SummaryTile label="启用中" value={stats.enabled} tone="emerald" />
        <SummaryTile label="抓取异常" value={stats.failing} tone="rose" />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {statusFilters.map(({ key, label, count }) => {
          const active = statusFilter === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => setStatusFilter(key)}
              className={`inline-flex items-center gap-1 rounded-full border px-3 py-1 text-xs font-medium transition ${
                active
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-slate-200 bg-white text-slate-600 hover:border-primary/40 hover:text-primary"
              }`}
            >
              <span>{label}</span>
              {typeof count === "number" && (
                <span
                  className={`rounded-full px-1.5 ${
                    active ? "bg-primary text-white" : "bg-slate-100 text-slate-500"
                  }`}
                >
                  {count}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {feedback && (
        <div
          className={`rounded-md border p-3 text-sm ${
            feedback.type === "success"
              ? "border-emerald-200 bg-emerald-50 text-emerald-700"
              : feedback.type === "error"
              ? "border-red-200 bg-red-50 text-red-700"
              : "border-sky-200 bg-sky-50 text-sky-700"
          }`}
        >
          {feedback.message}
        </div>
      )}

      {feedsQuery.isLoading ? (
        <div className="text-sm text-slate-500">正在加载订阅源…</div>
      ) : feedsQuery.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700">
          {(feedsQuery.error as Error).message || "加载订阅源失败"}
        </div>
      ) : (
        <FeedTable
          feeds={filteredFeeds}
          onToggle={handleToggle}
          onEdit={(feed) => handleOpenModal(feed)}
          onDelete={handleDelete}
          onTest={handleTestFeed}
          busyIds={busySet}
          testingIds={testingSet}
          emptyMessage={emptyMessage}
        />
      )}

      <FeedFormModal
        open={modalOpen}
        initial={editingFeed}
        onClose={resetModal}
        onSubmit={async (payload) => {
          await upsertMutation.mutateAsync(payload);
        }}
        submitting={upsertMutation.isPending}
        onTest={(url) => testFeed(token, url)}
      />
    </div>
  );
}

type SummaryTileProps = {
  label: string;
  value: number;
  tone?: "emerald" | "rose" | "slate";
};

function SummaryTile({ label, value, tone = "slate" }: SummaryTileProps) {
  const styles =
    tone === "emerald"
      ? "border-emerald-100 bg-emerald-50 text-emerald-700"
      : tone === "rose"
      ? "border-rose-100 bg-rose-50 text-rose-700"
      : "border-slate-100 bg-slate-50 text-slate-700";

  return (
    <div className={`rounded-lg border px-4 py-3 ${styles}`}>
      <p className="text-xs font-medium uppercase tracking-wide text-slate-500/80">
        {label}
      </p>
      <p className="mt-1 text-2xl font-semibold">{value}</p>
    </div>
  );
}
