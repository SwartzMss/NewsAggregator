import { NavLink, Outlet, useNavigate } from "react-router-dom";

const navLinkClass = ({ isActive }: { isActive: boolean }) =>
  `px-3 py-2 rounded-md text-sm font-medium transition-colors duration-150 ${
    isActive
      ? "bg-primary text-white"
      : "text-slate-600 hover:text-primary hover:bg-primary/10"
  }`;

export function AppLayout() {
  const navigate = useNavigate();

  return (
    <div className="min-h-screen flex flex-col">
      <header className="bg-white border-b border-slate-200">
        <div className="mx-auto max-w-6xl px-4 py-3 flex items-center justify-between">
          <button
            type="button"
            onClick={() => navigate("/")}
            className="text-xl font-semibold text-primary transition-colors hover:text-primary-dark focus:outline-none focus:ring-2 focus:ring-primary/30"
          >
            新闻聚合面板
          </button>
          <nav className="flex gap-2">
            <NavLink to="/" className={navLinkClass} end>
              新闻列表
            </NavLink>
            <NavLink to="/featured" className={navLinkClass}>
              精选
            </NavLink>
            <NavLink to="/search" className={navLinkClass}>
              搜索
            </NavLink>
          </nav>
        </div>
      </header>

      <main className="flex-1">
        <div className="mx-auto max-w-6xl px-4 py-6">
          <Outlet />
        </div>
      </main>

      <footer className="bg-white border-t border-slate-200">
        <div className="mx-auto max-w-6xl px-4 py-3 text-sm text-slate-500">
          © {new Date().getFullYear()} 新闻聚合服务 · Rust + React
        </div>
      </footer>
    </div>
  );
}
