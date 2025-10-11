import { ArticleOut } from "../../types/api";
import { formatDateTime, formatRelative } from "../../lib/time";
import { extractDomain } from "../../lib/domain";

export function ArticleCard({ article }: { article: ArticleOut }) {
  const domain = article.source_display_name ?? extractDomain(article.source_domain);
  return (
    <article className="bg-white border border-slate-200 rounded-lg shadow-sm p-4 transition hover:shadow-md">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-lg font-semibold text-slate-900">
            <a href={article.url} target="_blank" rel="noopener noreferrer" className="hover:underline">
              {article.title}
            </a>
          </h3>
          <p className="text-sm text-slate-500 mt-1">
            {domain} · {formatDateTime(article.published_at)} ({formatRelative(article.published_at)})
          </p>
        </div>
      </div>
      {article.description && (
        <p className="text-sm text-slate-600 mt-3 line-clamp-2">
          {article.description}
        </p>
      )}
    </article>
  );
}
