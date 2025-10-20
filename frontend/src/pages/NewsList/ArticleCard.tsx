import { ArticleOut } from "../../types/api";
import { useEffect, useRef, useState } from "react";
import { formatDateTime, formatRelative } from "../../lib/time";
import { extractDomain } from "../../lib/domain";
import { recordArticleClick } from "../../lib/api";

export function ArticleCard({ article }: { article: ArticleOut }) {
  const [expanded, setExpanded] = useState(false);
  const [needsToggle, setNeedsToggle] = useState(false);
  const descRef = useRef<HTMLParagraphElement | null>(null);
  const domain = extractDomain(article.source_domain);
  const handleClick = () => {
    void recordArticleClick(article.id);
  };
  const description = article.description ?? "";

  // 根据实际渲染结果检测折叠状态是否溢出
  useEffect(() => {
    const calc = () => {
      if (!descRef.current) return;
      // 仅在折叠态检测：折叠时 p 元素带 line-clamp-2
      const el = descRef.current;
      const overflows = el.scrollHeight > el.clientHeight + 1; // 1px 容差
      setNeedsToggle(overflows);
    };

    // 下一帧再测量，确保样式生效
    const raf = requestAnimationFrame(calc);
    window.addEventListener("resize", calc);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", calc);
    };
  }, [description, expanded]);

  return (
    <article className="bg-white border border-slate-200 rounded-lg shadow-sm p-4 transition hover:shadow-md">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-lg font-semibold text-slate-900">
            <a
              href={article.url}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:underline"
              onClick={handleClick}
            >
              {article.title}
            </a>
          </h3>
          <p className="text-sm text-slate-500 mt-1">
            {domain} · {formatDateTime(article.published_at)} ({formatRelative(article.published_at)})
          </p>
        </div>
      </div>
      {article.description && (
        <div className={"mt-3 " + (expanded ? "max-h-48 overflow-y-auto pr-1" : "")}> 
          <p
            ref={descRef}
            className={"text-sm text-slate-600 " + (expanded ? "" : "line-clamp-2")}
          >
            {description}
          </p>
        </div>
      )}
      {article.description && needsToggle && (
        <div className="mt-2">
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="text-xs text-primary hover:underline"
          >
            {expanded ? "收起" : "展开"}
          </button>
        </div>
      )}
    </article>
  );
}
