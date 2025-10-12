import { useQuery } from "@tanstack/react-query";
import { getArticles } from "../../lib/api";
import { ArticleCard } from "./ArticleCard";
import { ArticleOut, PageResp } from "../../types/api";

const DEFAULT_PAGE_SIZE = 20;

export function NewsListPage() {
  const query = useQuery<PageResp<ArticleOut>, Error>({
    queryKey: ["articles"],
    queryFn: () =>
      getArticles({
        page: 1,
        page_size: DEFAULT_PAGE_SIZE,
      }),
    staleTime: 30_000,
  });

  return (
    <div className="space-y-6">
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

      <div className="pt-4 text-sm text-slate-500">共 {query.data?.items.length ?? 0} 条展示内容</div>
    </div>
  );
}
