import { useInfiniteQuery, type InfiniteData } from "@tanstack/react-query";
import { getArticles } from "../../lib/api";
import { ArticleCard } from "./ArticleCard";
import { ArticleOut, PageResp } from "../../types/api";

const DEFAULT_PAGE_SIZE = 20;

export function NewsListPage() {
  const query = useInfiniteQuery<
    PageResp<ArticleOut>,
    Error,
    InfiniteData<PageResp<ArticleOut>, number>,
    [string],
    number
  >({
    queryKey: ["articles"],
    initialPageParam: 1,
    queryFn: ({ pageParam }) =>
      getArticles({
        page: pageParam,
        page_size: DEFAULT_PAGE_SIZE,
      }),
    getNextPageParam: (lastPage) =>
      lastPage.items.length < lastPage.page_size ? undefined : lastPage.page + 1,
    staleTime: 30_000,
  });

  const articles: ArticleOut[] =
    query.data?.pages.flatMap((page) => page.items) ?? [];

  return (
    <div className="space-y-6">
      {query.isLoading && !articles.length ? (
        <div className="text-sm text-slate-500">正在加载最新文章…</div>
      ) : query.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700">
          {(query.error as Error).message || "文章列表加载失败"}
        </div>
      ) : articles.length > 0 ? (
        <div className="space-y-4">
          {articles.map((article) => (
            <ArticleCard key={article.id} article={article} />
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-slate-200 bg-white p-6 text-center text-sm text-slate-500">
          暂无文章。请先添加订阅源等待抓取。
        </div>
      )}

      <div className="flex items-center justify-between border-t border-slate-200 pt-4">
        <span className="text-sm text-slate-500">共 {articles.length} 条展示内容</span>
        {query.hasNextPage && (
          <button
            onClick={() => query.fetchNextPage()}
            disabled={query.isFetchingNextPage}
            className="rounded-md border border-primary px-4 py-2 text-sm font-medium text-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {query.isFetchingNextPage ? "加载中…" : "查看更多"}
          </button>
        )}
      </div>
    </div>
  );
}
