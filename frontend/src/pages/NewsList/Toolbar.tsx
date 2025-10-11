import { useMemo } from "react";

export type ToolbarProps = {
  from?: string;
  to?: string;
  pageSize: number;
  onSubmit: (values: { from?: string; to?: string; pageSize: number }) => void;
  onRefresh: () => void;
};

export function Toolbar({ from, to, pageSize, onSubmit, onRefresh }: ToolbarProps) {
  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const formData = new FormData(event.currentTarget);
    const values = {
      from: (formData.get("from") as string) || undefined,
      to: (formData.get("to") as string) || undefined,
      pageSize: Number(formData.get("pageSize") ?? pageSize),
    };
    onSubmit(values);
  };

  const pageSizeOptions = useMemo(() => [10, 20, 50], []);

  return (
    <form
      onSubmit={handleSubmit}
      className="bg-white border border-slate-200 rounded-lg shadow-sm p-4 flex flex-wrap gap-4 items-end"
    >
      <div className="flex flex-col">
        <label htmlFor="from" className="text-sm font-medium text-slate-600">
          From
        </label>
        <input
          defaultValue={from}
          id="from"
          name="from"
          type="datetime-local"
          className="mt-1 w-56 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
      </div>

      <div className="flex flex-col">
        <label htmlFor="to" className="text-sm font-medium text-slate-600">
          To
        </label>
        <input
          defaultValue={to}
          id="to"
          name="to"
          type="datetime-local"
          className="mt-1 w-56 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
      </div>

      <div className="flex flex-col">
        <label htmlFor="pageSize" className="text-sm font-medium text-slate-600">
          Page size
        </label>
        <select
          defaultValue={pageSize}
          id="pageSize"
          name="pageSize"
          className="mt-1 w-32 rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        >
          {pageSizeOptions.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </div>

      <div className="flex gap-2">
        <button
          type="submit"
          className="inline-flex items-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-primary-dark focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2"
        >
          Apply
        </button>
        <button
          type="button"
          onClick={onRefresh}
          className="inline-flex items-center rounded-md border border-primary px-4 py-2 text-sm font-medium text-primary hover:bg-primary/10"
        >
          Refresh
        </button>
      </div>
    </form>
  );
}
