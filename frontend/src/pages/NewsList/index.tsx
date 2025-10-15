import {
  useInfiniteQuery,
  type InfiniteData,
  useQuery,
} from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { getArticles, getFeaturedArticles } from "../../lib/api";
import { ArticleCard } from "./ArticleCard";
import { ArticleOut, PageResp } from "../../types/api";

const DEFAULT_PAGE_SIZE = 20;

export function NewsListPage() {
  const query = useInfiniteQuery<
    PageResp<ArticleOut>,
    Error,
    InfiniteData<PageResp<ArticleOut>, number>,
    ["articles"],
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

  const featuredQuery = useQuery({
    queryKey: ["articles", "featured", { limit: 6 }],
    queryFn: () => getFeaturedArticles(6),
    staleTime: 30_000,
  });
  const featured = featuredQuery.data ?? [];

  return (
    <div className="space-y-6">
      {featured.length > 0 && (
        <section className="space-y-3 rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold text-slate-900">精选新闻</h2>
            <Link to="/featured" className="text-sm text-primary hover:underline">
              查看全部
            </Link>
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            {featured.map((article) => (
              <ArticleCard key={`featured-${article.id}`} article={article} />
            ))}
          </div>
        </section>
      )}

      <section className="space-y-4 rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
        <header className="border-b border-slate-200 pb-3">
          <h2 className="text-lg font-semibold text-slate-900">最新文章</h2>
        </header>

        {query.isLoading && !articles.length ? (
          <div className="text-sm text-slate-500">正在加载文章…</div>
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
          <div className="rounded-md border border-slate-200 bg-slate-50 p-6 text-center text-sm text-slate-500">
            暂无文章。请先添加订阅源等待抓取。
          </div>
        )}

        {query.hasNextPage && (
          <div className="flex justify-center border-t border-slate-200 pt-4">
            <button
              onClick={() => query.fetchNextPage()}
              disabled={query.isFetchingNextPage}
              className="rounded-md border border-primary px-4 py-2 text-sm font-medium text-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {query.isFetchingNextPage ? "加载中…" : "查看更多"}
            </button>
          </div>
        )}
      </section>
    </div>
  );
}
