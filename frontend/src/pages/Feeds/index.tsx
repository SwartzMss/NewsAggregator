import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { deleteFeed, listFeeds, upsertFeed } from "../../lib/api";
import { FeedOut, FeedUpsertPayload } from "../../types/api";
import { FeedTable } from "./FeedTable";
import { FeedFormModal } from "./FeedFormModal";

export function FeedsPage() {
  const queryClient = useQueryClient();
  const feedsQuery = useQuery({ queryKey: ["feeds"], queryFn: listFeeds });
  const [modalOpen, setModalOpen] = useState(false);
  const [editingFeed, setEditingFeed] = useState<FeedOut | null>(null);
  const [busyIds, setBusyIds] = useState<Set<number>>(new Set());
  const [feedback, setFeedback] = useState<string | null>(null);

  const resetModal = () => {
    setEditingFeed(null);
    setModalOpen(false);
  };

  const invalidateFeeds = () => queryClient.invalidateQueries({ queryKey: ["feeds"], exact: true });

  const upsertMutation = useMutation({
    mutationFn: async (payload: FeedUpsertPayload) => {
      const response = await upsertFeed(payload);
      return response;
    },
    onSuccess: () => {
      setFeedback("订阅源已保存");
      invalidateFeeds();
    },
    onError: (err: Error) => {
      setFeedback(err.message || "保存订阅源失败");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (feed: FeedOut) => {
      setBusyIds((prev) => new Set(prev).add(feed.id));
      await deleteFeed(feed.id);
    },
    onSuccess: () => {
      setFeedback("订阅源已删除");
      invalidateFeeds();
    },
    onError: (err: Error) => {
      setFeedback(err.message || "删除订阅源失败");
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

  const feeds = feedsQuery.data ?? [];
  const busySet = useMemo(() => busyIds, [busyIds]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-slate-900">订阅源列表</h2>
          <p className="text-sm text-slate-500">管理 RSS 链接及抓取状态。</p>
        </div>
        <button
          onClick={() => handleOpenModal()}
          className="inline-flex items-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-primary-dark"
        >
          + 新增订阅源
        </button>
      </div>

      {feedback && (
        <div className="rounded-md border border-slate-200 bg-white p-3 text-sm text-slate-600">
          {feedback}
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
          feeds={feeds}
          onToggle={handleToggle}
          onEdit={(feed) => handleOpenModal(feed)}
          onDelete={handleDelete}
          busyIds={busySet}
        />
      )}

      <FeedFormModal
        open={modalOpen}
        initial={editingFeed}
        onClose={resetModal}
        onSubmit={(payload) => upsertMutation.mutateAsync(payload)}
        submitting={upsertMutation.isPending}
      />
    </div>
  );
}
