import { RefObject } from "react";

type SearchFormProps = {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (value: string) => void;
  inputRef: RefObject<HTMLInputElement>;
};

export function SearchForm({
  value,
  onChange,
  onSubmit,
  inputRef,
}: SearchFormProps) {
  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit(value);
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-wrap items-end gap-3 rounded-lg border border-slate-200 bg-white p-4 shadow-sm"
    >
      <label className="flex min-w-[18rem] flex-1 flex-col gap-1">
        <span className="text-sm font-medium text-slate-600">标题搜索</span>
        <input
          ref={inputRef}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          type="text"
          placeholder="输入关键词后按回车"
          className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
      </label>
    </form>
  );
}
