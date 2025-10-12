import { useQuery } from "@tanstack/react-query";
import { getFeaturedArticles } from "../../lib/api";
import { ArticleCard } from "../NewsList/ArticleCard";

export function FeaturedPage() {
  const query = useQuery({
    queryKey: ["articles", "featured", { limit: 20 }],
    queryFn: () => getFeaturedArticles(20),
    staleTime: 30_000,
  });

  const articles = query.data ?? [];

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-slate-900">精选新闻</h2>
        <p className="text-sm text-slate-500">根据用户点击热度自动整理</p>
      </div>

      {query.isLoading ? (
        <div className="text-sm text-slate-500">正在加载精选新闻…</div>
      ) : query.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700">
          {(query.error as Error).message || "精选新闻加载失败"}
        </div>
      ) : articles.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-2">
          {articles.map((article) => (
            <ArticleCard key={article.id} article={article} />
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-slate-200 bg-white p-6 text-center text-sm text-slate-500">
          暂无精选新闻。请稍后再试。
        </div>
      )}
    </div>
  );
}
