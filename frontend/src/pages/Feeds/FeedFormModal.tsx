import { useEffect, useState } from "react";
import { FeedOut, FeedUpsertPayload } from "../../types/api";

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
  source_display_name: "",
  language: "",
  country: "",
  enabled: true,
  fetch_interval_seconds: 600,
  title: "",
  site_url: "",
};

export function FeedFormModal({ open, initial, onClose, onSubmit, submitting }: FeedFormModalProps) {
  const [form, setForm] = useState<FeedUpsertPayload>(emptyForm);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setError(null);
      if (initial) {
        setForm({
          id: initial.id,
          url: initial.url,
          source_domain: initial.source_domain,
          source_display_name: initial.source_display_name ?? "",
          language: initial.language ?? "",
          country: initial.country ?? "",
          enabled: initial.enabled,
          fetch_interval_seconds: initial.fetch_interval_seconds,
          title: initial.title ?? "",
          site_url: initial.site_url ?? "",
        });
      } else {
        setForm(emptyForm);
      }
    }
  }, [open, initial]);

  if (!open) return null;

  const handleChange = (
    event: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>
  ) => {
    const { name, value, type, checked } = event.target;
    setForm((prev) => ({
      ...prev,
      [name]: type === "checkbox" ? checked : value,
    }));
  };

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    if (!form.url.trim() || !form.source_domain.trim()) {
      setError("Feed URL and source domain are required.");
      return;
    }
    try {
      await onSubmit({
        ...form,
        source_display_name: form.source_display_name?.trim() || undefined,
        language: form.language?.trim() || undefined,
        country: form.country?.trim() || undefined,
        title: form.title?.trim() || undefined,
        site_url: form.site_url?.trim() || undefined,
      });
      onClose();
    } catch (err) {
      setError((err as Error).message ?? "Failed to save feed.");
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4">
      <div className="w-full max-w-xl rounded-lg bg-white shadow-lg">
        <div className="border-b border-slate-200 px-5 py-4">
          <h2 className="text-lg font-semibold text-slate-900">
            {initial ? "Edit Feed" : "New Feed"}
          </h2>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4 px-5 py-4">
          <div className="grid gap-4 md:grid-cols-2">
            <label className="flex flex-col text-sm font-medium text-slate-600">
              URL
              <input
                name="url"
                value={form.url}
                onChange={handleChange}
                required
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Source domain
              <input
                name="source_domain"
                value={form.source_domain}
                onChange={handleChange}
                required
                placeholder="example.com"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Display name
              <input
                name="source_display_name"
                value={form.source_display_name ?? ""}
                onChange={handleChange}
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Title override
              <input
                name="title"
                value={form.title ?? ""}
                onChange={handleChange}
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Site URL
              <input
                name="site_url"
                value={form.site_url ?? ""}
                onChange={handleChange}
                placeholder="https://example.com"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Language
              <input
                name="language"
                value={form.language ?? ""}
                onChange={handleChange}
                placeholder="en"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Country
              <input
                name="country"
                value={form.country ?? ""}
                onChange={handleChange}
                placeholder="US"
                className="mt-1 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </label>
            <label className="flex flex-col text-sm font-medium text-slate-600">
              Fetch interval (seconds)
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
              Enabled
            </label>
          </div>

          {error && <p className="text-sm text-red-600">{error}</p>}

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-slate-300 px-4 py-2 text-sm text-slate-600 hover:bg-slate-100"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="inline-flex items-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-primary-dark disabled:cursor-not-allowed disabled:opacity-60"
            >
              {submitting ? "Saving…" : "Save"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
