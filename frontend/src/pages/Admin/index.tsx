import { useCallback, useMemo, useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import {
  adminLogin,
  adminLogout,
  UnauthorizedError,
} from "../../lib/api";
import { FeedsPage } from "../Feeds";

type AdminSession = {
  token: string;
  expiresAt: number;
};

const SESSION_STORAGE_KEY = "news-admin-session";

const getStorage = (): Storage | null =>
  typeof window === "undefined" ? null : window.sessionStorage;

const loadSession = (): AdminSession | null => {
  const storage = getStorage();
  if (!storage) return null;
  const raw = storage.getItem(SESSION_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as AdminSession;
    if (typeof parsed?.token !== "string" || typeof parsed?.expiresAt !== "number") {
      storage.removeItem(SESSION_STORAGE_KEY);
      return null;
    }
    if (parsed.expiresAt <= Date.now()) {
      storage.removeItem(SESSION_STORAGE_KEY);
      return null;
    }
    return parsed;
  } catch {
    storage.removeItem(SESSION_STORAGE_KEY);
    return null;
  }
};

const persistSession = (session: AdminSession | null) => {
  const storage = getStorage();
  if (!storage) return;
  if (!session) {
    storage.removeItem(SESSION_STORAGE_KEY);
    return;
  }
  storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
};

export function AdminPage() {
  const [session, setSession] = useState<AdminSession | null>(() => loadSession());
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  const clearSession = useCallback((message?: string) => {
    setSession(null);
    persistSession(null);
    if (message) {
      setNotice(message);
    }
  }, []);

  useEffect(() => {
    if (!session) return;
    const remaining = session.expiresAt - Date.now();
    if (remaining <= 0) {
      clearSession("登录已过期，请重新登录");
      return;
    }
    setInfo(null);
    const timer = window.setTimeout(() => {
      clearSession("登录已过期，请重新登录");
    }, remaining);
    return () => window.clearTimeout(timer);
  }, [session, clearSession]);

  const loginMutation = useMutation({
    mutationFn: async (credentials: { username: string; password: string }) =>
      adminLogin(credentials.username, credentials.password),
    onSuccess: (data) => {
      const next: AdminSession = {
        token: data.token,
        expiresAt: Date.now() + data.expires_in * 1000,
      };
      setSession(next);
      persistSession(next);
      setNotice(null);
      setInfo(
        data.expires_in
          ? `登录成功，会话有效期 ${Math.round(data.expires_in / 60)} 分钟`
          : null
      );
    },
    onError: (error: Error) => {
      let message = error.message || "登录失败，请稍后重试";
      if (error instanceof UnauthorizedError) {
        message = "请重新登录后再试";
      } else if (message.startsWith("{")) {
        try {
          const parsed = JSON.parse(message) as { error?: { message?: string }; message?: string };
          message = parsed.error?.message ?? parsed.message ?? "登录失败，请稍后重试";
        } catch {
          message = "登录失败，请稍后重试";
        }
      }
      setNotice(message);
    },
  });

  const handleLogout = useCallback(
    async (message?: string) => {
      if (session) {
        try {
          await adminLogout(session.token);
        } catch (err) {
          if (!(err instanceof UnauthorizedError)) {
            console.warn("logout failed", err);
          }
        }
      }
      clearSession(message);
    },
    [session, clearSession]
  );

  const handleUnauthorized = useCallback(() => {
    clearSession("登录已过期，请重新登录");
  }, [clearSession]);

  const token = session?.token ?? "";
  const isLoggedIn = token.length > 0;

  const remainingText = useMemo(() => {
    if (!session) return null;
    const remaining = session.expiresAt - Date.now();
    if (remaining <= 0) return "会话已过期";
    if (remaining < 60_000) return "会话即将过期";
    return null;
  }, [session]);

  if (!isLoggedIn) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center px-6 py-12">
        <div className="w-full max-w-md rounded-2xl bg-white/95 shadow-2xl backdrop-blur-sm">
          <div className="flex flex-col items-center gap-3 border-b border-slate-200 px-8 py-8">
            <div className="flex h-14 w-14 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                className="h-8 w-8"
              >
                <path d="M12 12a5 5 0 1 0-5-5 5 5 0 0 0 5 5Z" />
                <path d="M17 21v-2a4 4 0 0 0-4-4h-2a4 4 0 0 0-4 4v2" />
              </svg>
            </div>
            <h1 className="text-2xl font-semibold text-slate-900">管理员登录</h1>
            <p className="text-center text-sm text-slate-500">
              请输入后台账号与密码，登录后即可维护订阅源与系统配置。
            </p>
          </div>

          <form
            className="space-y-5 px-8 py-8"
            onSubmit={(event) => {
              event.preventDefault();
              setNotice(null);
              setInfo(null);
              loginMutation.mutate({ username, password });
            }}
          >
            {notice && (
              <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600">
                {notice}
              </div>
            )}

            <label className="flex flex-col gap-1 text-sm font-medium text-slate-600">
              用户名
              <input
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                required
                placeholder="请输入用户名"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm font-medium text-slate-600">
              密码
              <input
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                type="password"
                required
                placeholder="请输入密码"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
              />
            </label>

            <div className="flex items-center justify-end text-xs text-slate-500">
              <a className="text-primary hover:underline" href="/">
                返回首页
              </a>
            </div>

            <button
              type="submit"
              disabled={loginMutation.isPending}
              className="w-full inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-white shadow-lg shadow-primary/30 transition hover:bg-primary-dark disabled:cursor-not-allowed disabled:opacity-60"
            >
              {loginMutation.isPending ? (
                <>
                  <span className="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
                  正在登录…
                </>
              ) : (
                "登录"
              )}
            </button>
          </form>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-slate-100 py-10">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6">
        <header className="rounded-2xl border border-slate-200 bg-white/90 px-8 py-6 shadow-lg shadow-slate-200">
          <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <div>
              <h1 className="text-2xl font-semibold text-slate-900">后台控制台</h1>
              <p className="mt-1 text-sm text-slate-500">
                管理订阅源、查看系统状态并规划后续的数据看板。
              </p>
            </div>
            <div className="flex flex-col items-start gap-2 text-sm text-slate-500 md:items-end">
              {info && <span>{info}</span>}
              {remainingText && <span>{remainingText}</span>}
              <button
                onClick={() => handleLogout("已退出登录")}
                className="inline-flex items-center rounded-md border border-primary px-4 py-2 text-sm font-medium text-primary hover:bg-primary/10"
              >
                退出登录
              </button>
            </div>
          </div>
        </header>

        {notice && (
          <div className="rounded-md border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-700">
            {notice}
          </div>
        )}

        <section className="rounded-2xl border border-slate-200 bg-white/95 px-6 py-6 shadow-lg shadow-slate-200">
          <FeedsPage token={token} onUnauthorized={handleUnauthorized} />
        </section>
      </div>
    </div>
  );
}
