import { useEffect, useState } from "react";
import { testFeed } from "../../lib/api";
import { FeedOut, FeedTestResult, FeedUpsertPayload } from "../../types/api";

export type FeedFormModalProps = {
  open: boolean;
  initial?: FeedOut | null;
  onClose: () => void;
  onSubmit: (payload: FeedUpsertPayload) => Promise<void> | void;
  submitting?: boolean;
};

const emptyForm: FeedUpsertPayload = {
  url: "",
  source_domain: "",
  enabled: true,
  fetch_interval_seconds: 600,
  title: "",
  site_url: "",
  filter_condition: "",
};

const guessSourceDomain = (raw: string): string | null => {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  try {
    const url = new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
    const host = url.hostname.toLowerCase();
    return host.startsWith("www.") ? host.slice(4) : host;
  } catch {
    return null;
  }
};

export function FeedFormModal({ open, initial, onClose, onSubmit, submitting }: FeedFormModalProps) {
  const [form, setForm] = useState<FeedUpsertPayload>(emptyForm);
  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<FeedTestResult | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [sourceDomainEdited, setSourceDomainEdited] = useState(false);

  useEffect(() => {
    if (open) {
      setError(null);
      setTestResult(null);
      setTestError(null);
      if (initial) {
        setForm({
          id: initial.id,
          url: initial.url,
          source_domain: initial.source_domain,
          enabled: initial.enabled,
          fetch_interval_seconds: initial.fetch_interval_seconds,
          title: initial.title ?? "",
          site_url: initial.site_url ?? "",
          filter_condition: initial.filter_condition ?? "",
        });
        setSourceDomainEdited(Boolean(initial.source_domain?.trim()));
      } else {
        setForm(emptyForm);
        setSourceDomainEdited(false);
      }
    }
  }, [open, initial]);

  useEffect(() => {
    if (sourceDomainEdited) return;
    const guess = guessSourceDomain(form.url);
    if (guess && form.source_domain !== guess) {
      setForm((prev) => ({
        ...prev,
        source_domain: guess,
      }));
    }
  }, [form.url, form.source_domain, sourceDomainEdited]);

  if (!open) return null;

  const handleChange = (
    event: React.ChangeEvent<
      HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement
    >
  ) => {
    const target = event.target;

    const { name } = target;
    let nextValue: unknown;

    if (target instanceof HTMLInputElement) {
      if (target.type === "checkbox") {
        nextValue = target.checked;
      } else if (target.type === "number" || name === "fetch_interval_seconds") {
        if (target.value === "") {
          nextValue = undefined;
        } else {
          const parsed = Number(target.value);
          nextValue = Number.isNaN(parsed) ? undefined : parsed;
        }
      } else {
        nextValue = target.value;
      }
    } else {
      nextValue = target.value;
    }

    setForm((prev) => ({
      ...prev,
      [name]: nextValue,
    }));

    if (name === "url") {
      setTestResult(null);
      setTestError(null);
    }

    if (name === "source_domain" && typeof nextValue === "string") {
      const trimmed = nextValue.trim();
      setSourceDomainEdited(trimmed.length > 0);
    }
  };

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    if (!form.url.trim() || !form.source_domain.trim()) {
      setError("请填写订阅源 URL 和来源域名");
      return;
    }
    try {
      await onSubmit({
        ...form,
        title: form.title?.trim() || undefined,
        site_url: form.site_url?.trim() || undefined,
        filter_condition: form.filter_condition?.trim() || undefined,
      });
      onClose();
    } catch (err) {
      setError((err as Error).message ?? "保存订阅源失败");
    }
  };

  const handleTest = async () => {
    const url = form.url.trim();
    if (!url) {
      setTestResult(null);
      setTestError("请先填写 RSS 地址");
      return;
    }

    setTesting(true);
    setTestResult(null);
    setTestError(null);

    try {
      const result = await testFeed(url);
      setTestResult(result);

      setForm((prev) => ({
        ...prev,
        title:
          prev.title && prev.title.trim().length > 0
            ? prev.title
            : result.title ?? "",
        site_url:
          prev.site_url && prev.site_url.trim().length > 0
            ? prev.site_url
            : result.site_url ?? "",
      }));
    } catch (err) {
      setTestError((err as Error).message || "测试失败");
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4">
      <div className="w-full max-w-xl rounded-lg bg-white shadow-lg">
        <div className="border-b border-slate-200 px-5 py-4">
          <h2 className="text-lg font-semibold text-slate-900">
            {initial ? "编辑订阅源" : "新增订阅源"}
          </h2>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4 px-5 py-4">
          <div className="grid gap-4 md:grid-cols-2">
            <label className="flex flex-col text-sm font-medium text-slate-600">
              <div className="flex items-center justify-between gap-3">
                <span>RSS 地址</span>
                <button
                  type="button"
                  onClick={handleTest}
                  disabled={testing || form.url.trim().length === 0}
                  className="inline-flex items-center rounded-md border border-primary px-2.5 py-1 text-xs font-medium text-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {testing ? "测试中…" : "测试链接"}
                </button>
              </div>
              <input
                name="url"
                value={form.url}
                onChange={handleChange}
                required
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              来源域名
              <input
                name="source_domain"
                value={form.source_domain}
                onChange={handleChange}
                required
                placeholder="example.com"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
              <span className="mt-1 text-xs font-normal text-slate-500">
                系统会根据 RSS 地址自动补全，可手动调整。
              </span>
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              标题覆盖
              <input
                name="title"
                value={form.title ?? ""}
                onChange={handleChange}
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              站点主页
              <input
                name="site_url"
                value={form.site_url ?? ""}
                onChange={handleChange}
                placeholder="https://example.com"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              抓取间隔（秒）
              <input
                name="fetch_interval_seconds"
                type="number"
                min={60}
                step={60}
                value={form.fetch_interval_seconds ?? 600}
                onChange={handleChange}
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="mt-6 flex items-center gap-2 text-sm font-medium text-slate-600">
              <input
                type="checkbox"
                name="enabled"
                checked={form.enabled ?? true}
                onChange={handleChange}
                className="h-4 w-4 rounded border-slate-300 text-primary focus:ring-primary"
              />
              启用订阅
            </label>
          </div>

          <label className="flex flex-col text-sm font-medium text-slate-600">
            <span>过滤条件（SQL 布尔表达式，可选）</span>
            <textarea
              name="filter_condition"
              value={form.filter_condition ?? ""}
              onChange={handleChange}
              className="mt-1 h-24 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              placeholder="例如：url LIKE '%newsflashes%'"
            />
            <span className="mt-1 text-xs font-normal text-slate-400">
              条件为真时文章保留，反之将被自动清理。请勿填写分号、注释或其他数据修改语句。
            </span>
          </label>

          {testResult && (
            <div className="rounded-md border border-green-200 bg-green-50 px-3 py-2 text-sm text-green-700">
              测试成功：HTTP {testResult.status}，解析到 {testResult.entry_count} 条内容
              {testResult.title && (
                <span className="block md:inline">，标题：{testResult.title}</span>
              )}
              {testResult.site_url && (
                <span className="block md:inline">，主页：{testResult.site_url}</span>
              )}
            </div>
          )}

          {testError && (
            <div className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
              {testError}
            </div>
          )}

          {error && <p className="text-sm text-red-600">{error}</p>}

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-slate-300 px-4 py-2 text-sm text-slate-600 hover:bg-slate-100"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="inline-flex items-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-primary-dark disabled:cursor-not-allowed disabled:opacity-60"
            >
              {submitting ? "保存中…" : "保存"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
