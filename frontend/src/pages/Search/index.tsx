import {
  useInfiniteQuery,
  type InfiniteData,
} from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { ArticleCard } from "../NewsList/ArticleCard";
import { getArticles } from "../../lib/api";
import { ArticleOut, PageResp } from "../../types/api";
import { SearchForm } from "./SearchForm";

const DEFAULT_PAGE_SIZE = 20;

export function SearchPage() {
  const [inputValue, setInputValue] = useState("");
  const [keyword, setKeyword] = useState<string | undefined>(undefined);
  const inputRef = useRef<HTMLInputElement>(null);

  const trimmedKeyword = useMemo(() => keyword?.trim(), [keyword]);
  const queryEnabled = Boolean(trimmedKeyword);

  const query = useInfiniteQuery<
    PageResp<ArticleOut>,
    Error,
    InfiniteData<PageResp<ArticleOut>, number>,
    ["articles", { keyword?: string }],
    number
  >({
    queryKey: ["articles", { keyword: trimmedKeyword }],
    enabled: queryEnabled,
    initialPageParam: 1,
    queryFn: ({ pageParam }) =>
      getArticles({
        page: pageParam,
        page_size: DEFAULT_PAGE_SIZE,
        keyword: trimmedKeyword ?? undefined,
      }),
    getNextPageParam: (lastPage) =>
      lastPage.items.length < lastPage.page_size ? undefined : lastPage.page + 1,
    staleTime: 30_000,
  });

  const articles: ArticleOut[] =
    query.data?.pages.flatMap((page) => page.items) ?? [];

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = (rawValue: string) => {
    const next = rawValue.trim();
    setKeyword(next ? next : undefined);
    setInputValue(next ? rawValue : "");
  };

  return (
    <div className="space-y-6">
      <section className="space-y-3 rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
        <div>
          <h2 className="text-lg font-semibold text-slate-900">搜索文章</h2>
          <p className="mt-1 text-sm text-slate-500">
            仅支持标题关键字匹配，输入后按 Enter 开始检索。
          </p>
        </div>
        <SearchForm
          value={inputValue}
          onChange={setInputValue}
          onSubmit={handleSubmit}
          inputRef={inputRef}
        />
      </section>

      <section className="space-y-4 rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
        <header className="flex items-center justify-between border-b border-slate-200 pb-3">
          <h3 className="text-lg font-semibold text-slate-900">搜索结果</h3>
          {trimmedKeyword && (
            <span className="text-sm text-slate-500">关键字：{trimmedKeyword}</span>
          )}
        </header>

        {!trimmedKeyword ? (
          <div className="rounded-md border border-dashed border-slate-300 bg-slate-50 p-6 text-center text-sm text-slate-500">
            输入关键字并按 Enter 开始搜索。
          </div>
        ) : query.isLoading && !articles.length ? (
          <div className="text-sm text-slate-500">正在搜索…</div>
        ) : query.isError ? (
          <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700">
            {(query.error as Error).message || "搜索失败，请稍后再试。"}
          </div>
        ) : articles.length > 0 ? (
          <div className="space-y-4">
            {articles.map((article) => (
              <ArticleCard key={article.id} article={article} />
            ))}
          </div>
        ) : (
          <div className="rounded-md border border-slate-200 bg-slate-50 p-6 text-center text-sm text-slate-500">
            未找到匹配的文章。
          </div>
        )}

        {queryEnabled && query.hasNextPage && (
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
