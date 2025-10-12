import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { getArticles } from "../../lib/api";
import { ArticleCard } from "./ArticleCard";
import { Toolbar } from "./Toolbar";

const toLocalInputValue = (value?: string | null) => {
  if (!value) return "";
  const date = new Date(value);
  const tzOffset = date.getTimezoneOffset();
  const adjusted = new Date(date.getTime() - tzOffset * 60_000);
  return adjusted.toISOString().slice(0, 16); // yyyy-MM-ddTHH:mm
};

const toISO = (value?: string) => {
  if (!value) return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
  return date.toISOString();
};

export function NewsListPage() {
  const [params, setParams] = useSearchParams();

  const page = Number(params.get("page") ?? "1");
  const pageSize = Number(params.get("page_size") ?? "20");
  const from = params.get("from") ?? undefined;
  const to = params.get("to") ?? undefined;

  const query = useQuery({
    queryKey: ["articles", { from, to, page, pageSize }],
    queryFn: () =>
      getArticles({
        from,
        to,
        page,
        page_size: pageSize,
      }),
    keepPreviousData: true,
  });

  const totalPages = useMemo(() => {
    if (!query.data) return 1;
    const total = query.data.total_hint;
    return total > 0 ? Math.max(1, Math.ceil(total / query.data.page_size)) : query.data.items.length > 0 ? page : 1;
  }, [query.data, page]);

  const handleSubmitFilters = (values: { from?: string; to?: string; pageSize: number }) => {
    setParams((prev) => {
      const next = new URLSearchParams(prev);
      const isoFrom = toISO(values.from);
      const isoTo = toISO(values.to);
      if (isoFrom) next.set("from", isoFrom);
      else next.delete("from");
      if (isoTo) next.set("to", isoTo);
      else next.delete("to");
      next.set("page_size", String(values.pageSize));
      next.set("page", "1");
      return next;
    });
  };

  const handleRefresh = () => {
    query.refetch();
  };

  const navigatePage = (nextPage: number) => {
    setParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set("page", String(nextPage));
      return next;
    });
  };

  const localFrom = toLocalInputValue(from);
  const localTo = toLocalInputValue(to);

  return (
    <div className="space-y-6">
      <Toolbar from={localFrom} to={localTo} pageSize={pageSize} onSubmit={handleSubmitFilters} onRefresh={handleRefresh} />

      {query.isLoading ? (
        <div className="text-sm text-slate-500">正在加载最新文章…</div>
      ) : query.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700">
          {(query.error as Error).message || "文章列表加载失败"}
        </div>
      ) : query.data && query.data.items.length > 0 ? (
        <div className="space-y-4">
          {query.data.items.map((article) => (
            <ArticleCard key={article.id} article={article} />
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-slate-200 bg-white p-6 text-center text-sm text-slate-500">
          暂无文章。请先添加订阅源等待抓取。
        </div>
      )}

      <div className="flex items-center justify-between border-t border-slate-200 pt-4">
        <div className="text-sm text-slate-500">
          第 {page} / {totalPages} 页
        </div>
        <div className="flex gap-2">
          <button
            className="rounded-md border border-slate-300 px-3 py-1 text-sm text-slate-600 hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-50"
            onClick={() => navigatePage(Math.max(1, page - 1))}
            disabled={page <= 1 || query.isFetching}
          >
            上一页
          </button>
          <button
            className="rounded-md border border-slate-300 px-3 py-1 text-sm text-slate-600 hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-50"
            onClick={() => navigatePage(page + 1)}
            disabled={query.isFetching || page >= totalPages}
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
