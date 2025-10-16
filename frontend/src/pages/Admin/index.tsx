import { useCallback, useMemo, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  adminLogin,
  adminLogout,
  UnauthorizedError,
  getTranslationSettings,
  updateTranslationSettings,
} from "../../lib/api";
import { TranslationSettings, TranslationSettingsUpdate } from "../../types/api";
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

  const clearSession = useCallback(
    (message?: string) => {
      setSession(null);
      persistSession(null);
      setUsername("");
      setPassword("");
      setInfo(null);
      if (message) {
        setNotice(message);
      }
    },
    [setSession, setUsername, setPassword]
  );

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
      setSidebarOpen(false);
      setActiveSection("feeds");
    },
    [session, clearSession]
  );

  const handleUnauthorized = useCallback(() => {
    clearSession("登录已过期，请重新登录");
    setSidebarOpen(false);
    setActiveSection("feeds");
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

  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [activeSection, setActiveSection] = useState("feeds");

  const sections = useMemo(
    () => [
      {
        key: "feeds",
        label: "订阅源管理",
        description: "添加、监控并测试后台抓取源。",
        render: () => (
          <FeedsPage
            token={token}
            onUnauthorized={handleUnauthorized}
            showHeader={false}
          />
        ),
      },
      {
        key: "translation",
        label: "翻译服务",
        description: "配置翻译提供商并查看当前额度状态。",
        render: () => (
          <TranslationSettingsPanel
            token={token}
            onUnauthorized={handleUnauthorized}
          />
        ),
      },
    ],
    [handleUnauthorized, token]
  );

  useEffect(() => {
    if (sections.length === 0) {
      return;
    }
    if (!sections.some((section) => section.key === activeSection)) {
      setActiveSection(sections[0].key);
    }
  }, [sections, activeSection]);

  const activeSectionData = useMemo(
    () =>
      sections.find((section) => section.key === activeSection) ?? sections[0],
    [sections, activeSection]
  );

  const handleSectionChange = useCallback((key: string) => {
    setActiveSection(key);
    setSidebarOpen(false);
  }, []);

  if (!isLoggedIn) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-slate-200 via-white to-slate-300 flex items-center justify-center px-6 py-12">
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

  const desktopNavItemClass = (key: string) =>
    `w-full text-left rounded-lg px-3 py-2 text-sm font-medium transition ${
      activeSection === key
        ? "bg-primary/10 text-primary shadow"
        : "text-slate-600 hover:bg-slate-100"
    }`;

  const headerNavItemClass = (key: string) =>
    `rounded-full px-3 py-1 text-xs font-medium transition ${
      activeSection === key
        ? "bg-primary text-white shadow"
        : "bg-slate-200 text-slate-600 hover:bg-slate-300"
    }`;

  return (
    <div className="flex min-h-screen bg-slate-100 text-slate-900">
      {sidebarOpen && (
        <div className="lg:hidden">
          <div
            className="fixed inset-0 z-40 bg-slate-900/40 backdrop-blur-sm"
            onClick={() => setSidebarOpen(false)}
          />
          <div className="fixed inset-y-0 left-0 z-50 w-64 bg-white px-6 py-6 shadow-2xl">
            <div className="mb-6 flex items-center justify-between">
              <span className="text-lg font-semibold tracking-wide text-slate-900">
                新闻聚合面板
              </span>
              <button
                onClick={() => setSidebarOpen(false)}
                className="rounded-md p-2 text-slate-500 transition hover:bg-slate-100 hover:text-slate-700"
              >
                <span className="sr-only">关闭菜单</span>
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.6"
                  className="h-5 w-5"
                >
                  <path d="m6 6 12 12M18 6 6 18" strokeLinecap="round" />
                </svg>
              </button>
            </div>
            <nav className="space-y-1">
              {sections.map((section) => (
                <button
                  key={section.key}
                  onClick={() => handleSectionChange(section.key)}
                  className={desktopNavItemClass(section.key)}
                >
                  {section.label}
                </button>
              ))}
            </nav>
          </div>
        </div>
      )}

      <aside className="hidden lg:flex lg:w-64 lg:flex-col lg:border-r lg:border-slate-200 lg:bg-white">
        <div className="flex h-20 items-center justify-center border-b border-slate-200">
          <span className="text-lg font-semibold tracking-wide text-slate-900">
            新闻聚合面板
          </span>
        </div>
        <nav className="flex-1 space-y-1 px-6 py-6">
          {sections.map((section) => (
            <button
              key={section.key}
              onClick={() => handleSectionChange(section.key)}
              className={desktopNavItemClass(section.key)}
            >
              <span className="flex items-center gap-3">
                <span
                  className={`h-2 w-2 rounded-full ${
                    activeSection === section.key
                      ? "bg-primary"
                      : "bg-slate-300"
                  }`}
                />
                {section.label}
              </span>
            </button>
          ))}
        </nav>
        <div className="border-t border-slate-200 px-6 py-5 text-xs text-slate-500">
          {remainingText ? `状态：${remainingText}` : "状态：会话正常"}
        </div>
      </aside>

      <div className="flex flex-1 flex-col bg-slate-50">
        <header className="sticky top-0 z-30 border-b border-slate-200 bg-white/90 backdrop-blur">
          <div className="mx-auto flex w-full max-w-6xl items-center justify-between gap-4 px-4 py-3">
            <div className="flex items-center gap-3">
              <button
                className="inline-flex items-center justify-center rounded-md border border-slate-200 bg-white p-2 text-slate-600 shadow-sm transition hover:bg-slate-100 lg:hidden"
                onClick={() => setSidebarOpen(true)}
              >
                <span className="sr-only">打开菜单</span>
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.6"
                  className="h-5 w-5"
                >
                  <path d="M4 7h16M4 12h16M4 17h16" strokeLinecap="round" />
                </svg>
              </button>
              <div className="flex flex-col">
                <span className="text-sm font-semibold text-slate-800">
                  News 控制台
                </span>
                <span className="text-xs text-slate-500">
                  {activeSectionData?.label ?? ""}
                </span>
              </div>
            </div>
            <div className="flex items-center gap-4">
              {remainingText && (
                <span className="hidden text-xs text-slate-500 sm:inline">
                  {remainingText}
                </span>
              )}
              <button
                onClick={() => handleLogout("已退出登录")}
                className="inline-flex items-center rounded-md border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-600 shadow-sm transition hover:bg-slate-100"
              >
                退出登录
              </button>
            </div>
          </div>
          <div className="border-t border-slate-200 bg-white lg:hidden">
            <nav className="flex gap-2 overflow-x-auto px-4 py-2">
              {sections.map((section) => (
                <button
                  key={section.key}
                  onClick={() => handleSectionChange(section.key)}
                  className={headerNavItemClass(section.key)}
                >
                  {section.label}
                </button>
              ))}
            </nav>
          </div>
        </header>

        <main className="flex-1 overflow-y-auto bg-slate-100">
          <div className="mx-auto w-full max-w-6xl space-y-6 px-4 py-6">
            {info && (
              <div className="flex items-start justify-between gap-4 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 shadow-sm">
                <span>{info}</span>
                <button
                  onClick={() => setInfo(null)}
                  className="rounded-full px-2 text-emerald-600 transition hover:bg-emerald-100 hover:text-emerald-800"
                >
                  ×
                </button>
              </div>
            )}
            {notice && (
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-700 shadow-sm">
                {notice}
              </div>
            )}

            <section className="rounded-2xl border border-white/60 bg-white/95 p-6 shadow-xl shadow-slate-200">
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 pb-4">
                <div>
                  <h1 className="text-2xl font-semibold text-slate-900">
                    {activeSectionData?.label ?? ""}
                  </h1>
                  {activeSectionData?.description && (
                    <p className="mt-1 text-sm text-slate-500">
                      {activeSectionData.description}
                    </p>
                  )}
                </div>
                <div className="flex items-center gap-3 text-xs text-slate-500">
                  {remainingText && <span>{remainingText}</span>}
                </div>
              </div>
              <div className="pt-6">
                {activeSectionData?.render()}
              </div>
            </section>
          </div>
        </main>
      </div>
    </div>
  );
}

function TranslationSettingsPanel({
  token,
  onUnauthorized,
}: {
  token: string;
  onUnauthorized: () => void;
}) {
  const queryClient = useQueryClient();
  const [baiduAppId, setBaiduAppId] = useState("");
  const [baiduSecret, setBaiduSecret] = useState("");
  const [deepseekKey, setDeepseekKey] = useState("");
  const [dirtyAppId, setDirtyAppId] = useState(false);
  const [dirtySecret, setDirtySecret] = useState(false);
  const [dirtyDeepseek, setDirtyDeepseek] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [localTranslate, setLocalTranslate] = useState<boolean | null>(null);

  const settingsQuery = useQuery<TranslationSettings, Error>({
    queryKey: ["translation-settings", token],
    queryFn: () => getTranslationSettings(token),
    enabled: Boolean(token),
    retry: false,
  });

  useEffect(() => {
    if (settingsQuery.error instanceof UnauthorizedError) {
      onUnauthorized();
    }
  }, [settingsQuery.error, onUnauthorized]);

  useEffect(() => {
    setFeedback(null);
    setBaiduAppId("");
    setBaiduSecret("");
    setDeepseekKey("");
    setDirtyAppId(false);
    setDirtySecret(false);
    setDirtyDeepseek(false);
    setLocalTranslate(null);
  }, [settingsQuery.data]);

  const mutation = useMutation<TranslationSettings, Error, TranslationSettingsUpdate>({
    mutationFn: async (payload: TranslationSettingsUpdate) =>
      updateTranslationSettings(token, payload),
    onSuccess: (data) => {
      queryClient.setQueryData(["translation-settings", token], data);
      queryClient.invalidateQueries({ queryKey: ["translation-settings", token] });
      const baiduPending =
        !data.baidu_configured &&
        Boolean(data.baidu_app_id_masked) &&
        Boolean(data.baidu_secret_key_masked);
      const deepseekPending =
        !data.deepseek_configured && Boolean(data.deepseek_api_key_masked);
      if (baiduPending || deepseekPending) {
        setFeedback("配置已保存，正在验证凭据…");
      } else {
        setFeedback("翻译配置已更新");
      }
      setDirtyAppId(false);
      setDirtySecret(false);
      setDirtyDeepseek(false);
      setBaiduAppId("");
      setBaiduSecret("");
      setDeepseekKey("");
      setLocalTranslate(null);
    },
    onError: (err: Error) => {
      if (err instanceof UnauthorizedError) {
        onUnauthorized();
      } else {
        setFeedback(err.message || "翻译配置更新失败");
      }
      setLocalTranslate(null);
    },
  });

  const settings = settingsQuery.data;
  const provider = settings?.provider ?? "";
  const options = settings?.available_providers ?? ["deepseek", "baidu", "ollama"];
  const busy = mutation.isPending;
  const translateDescriptions =
    localTranslate ?? settings?.translate_descriptions ?? false;
  const hasBaiduCredentials =
    Boolean(settings?.baidu_app_id_masked) && Boolean(settings?.baidu_secret_key_masked);
  const hasDeepseekCredentials = Boolean(settings?.deepseek_api_key_masked);
  const hasOllamaConfig = Boolean(settings?.ollama_base_url);
  const pendingBaiduVerification =
    Boolean(settings) && hasBaiduCredentials && !settings?.baidu_configured && !settings?.baidu_error;
  const pendingDeepseekVerification =
    Boolean(settings) && hasDeepseekCredentials && !settings?.deepseek_configured && !settings?.deepseek_error;
  const pendingOllamaVerification =
    Boolean(settings) && hasOllamaConfig && !settings?.ollama_configured && !settings?.ollama_error;

  useEffect(() => {
    if (!token) return;
    if (!pendingBaiduVerification && !pendingDeepseekVerification && !pendingOllamaVerification)
      return;
    const timer = window.setInterval(() => {
      queryClient.invalidateQueries({ queryKey: ["translation-settings", token] });
    }, 4000);
    return () => window.clearInterval(timer);
  }, [
    pendingBaiduVerification,
    pendingDeepseekVerification,
    pendingOllamaVerification,
    queryClient,
    token,
  ]);

  const formatLabel = (value: string) => {
    if (value === "baidu") return "百度翻译";
    if (value === "ollama") return "Ollama 本地";
    return "Deepseek";
  };
  const available = (value: string) => {
    if (!settings) return false;
    if (value === "baidu") return settings.baidu_configured;
    if (value === "deepseek") return settings.deepseek_configured;
    if (value === "ollama") return settings.ollama_configured;
    return false;
  };
  const providerError = (value: string) => {
    if (!settings) return null;
    if (value === "baidu") return settings.baidu_error ?? null;
    if (value === "deepseek") return settings.deepseek_error ?? null;
    if (value === "ollama") return settings.ollama_error ?? null;
    return null;
  };
  const providerStatusSuffix = (value: string) => {
    if (!settings) return "（未配置）";
    if (available(value)) return "";
    const errorMessage = providerError(value);
    if (errorMessage) return "（验证失败）";
    const hasCredential =
      value === "baidu" ? hasBaiduCredentials : hasDeepseekCredentials;
    if (value === "ollama") {
      return hasOllamaConfig ? "（待验证）" : "（未配置）";
    }
    return hasCredential ? "（待验证）" : "（未配置）";
  };
  const statusHints: string[] = [];
  if (pendingDeepseekVerification) {
    statusHints.push("Deepseek 凭据验证中…");
  }
  if (pendingBaiduVerification) {
    statusHints.push("百度翻译凭据验证中…");
  }
  if (pendingOllamaVerification) {
    statusHints.push("Ollama 连通性验证中…");
  }
  if (settings || localTranslate !== null) {
    statusHints.push(
      translateDescriptions ? "当前翻译标题和摘要。" : "当前仅翻译标题。"
    );
  }
  const statusMessage = busy
    ? "正在更新翻译配置…"
    : feedback ?? (statusHints.length > 0 ? statusHints.join(" ") : null);

  const autoUpdate = (payload: TranslationSettingsUpdate) => {
    if (busy) return;
    if (Object.keys(payload).length === 0) return;
    mutation.mutate(payload);
  };

  const handleToggleDescriptions = () => {
    if (busy) return;
    const next = !translateDescriptions;
    setLocalTranslate(next);
    mutation.mutate({ translate_descriptions: next });
  };

  return (
    <div className="space-y-5">
      {settingsQuery.isLoading ? (
        <div className="text-sm text-slate-500">正在加载翻译配置…</div>
      ) : settingsQuery.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600">
          {settingsQuery.error.message || "翻译配置加载失败"}
        </div>
      ) : (
        <>
          <div className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-slate-700">默认翻译服务</p>
                <p className="text-xs text-slate-500">
                  选择后台操作默认使用的翻译服务提供商。
                </p>
              </div>
              <span className="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary">
                当前：{provider ? formatLabel(provider) : "未选择"}
              </span>
            </div>
            <select
              value={provider}
              onChange={(event) => {
                const value = event.target.value;
                if (!settings) return;
                if (value === settings.provider) return;
                if (!available(value)) return;
                mutation.mutate({ provider: value });
              }}
              disabled={busy}
              className="mt-3 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {options.map((option) => {
                const disabled = !available(option);
                const errorMessage = providerError(option);
                const suffix = disabled ? providerStatusSuffix(option) : "";
                return (
                  <option
                    key={option}
                    value={option}
                    disabled={disabled}
                    title={errorMessage || undefined}
                  >
                    {`${formatLabel(option)}${suffix}`}
                  </option>
                );
              })}
            </select>
          </div>

          <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-slate-700">翻译内容范围</p>
                <p className="text-xs text-slate-500">
                  默认仅翻译标题，开启后会同步翻译摘要。
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={translateDescriptions}
                onClick={handleToggleDescriptions}
                disabled={busy}
                className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition ${
                  translateDescriptions ? "bg-primary" : "bg-slate-300"
                } ${busy ? "opacity-60" : ""}`}
              >
                <span
                  className={`inline-block h-5 w-5 transform rounded-full bg-white transition ${
                    translateDescriptions ? "translate-x-5" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
            <p className="mt-3 text-xs text-slate-500">
              {translateDescriptions
                ? "标题与摘要都会翻译成中文，适合人工审核或前台展示。"
                : "仅翻译标题，更省额度，摘要保留原文。"}
            </p>
          </section>

          <div className="grid gap-4 lg:grid-cols-2">
            <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-slate-700">Deepseek</p>
                  <p className="text-xs text-slate-500">
                    使用语言模型快速完成中英文互译。
                  </p>
                </div>
                <span
                  className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${
                    settings?.deepseek_configured
                      ? "bg-emerald-100 text-emerald-700"
                      : "bg-slate-200 text-slate-600"
                  }`}
                >
                  {settings?.deepseek_configured ? "可用" : "未配置"}
                </span>
              </div>
              <div className="mt-4 space-y-2">
                <label className="text-xs font-medium text-slate-500" htmlFor="translation-deepseek-key">
                  Deepseek API Key
                </label>
                <input
                  id="translation-deepseek-key"
                  value={dirtyDeepseek ? deepseekKey : ""}
                  onChange={(event) => {
                    setDeepseekKey(event.target.value);
                    setDirtyDeepseek(true);
                  }}
                  onBlur={() => {
                    if (!dirtyDeepseek) return;
                    autoUpdate({ deepseek_api_key: deepseekKey });
                  }}
                  placeholder={
                    settings?.deepseek_api_key_masked ?? "请输入 Deepseek API Key"
                  }
                  className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                />
                {settings?.deepseek_error ? (
                  <p className="text-xs text-red-500">{settings.deepseek_error}</p>
                ) : pendingDeepseekVerification ? (
                  <p className="text-xs text-slate-500">正在验证 API 凭据…</p>
                ) : null}
              </div>
            </section>

            <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-slate-700">百度翻译</p>
                  <p className="text-xs text-slate-500">
                    绑定凭据后可作为冗余的翻译备选方案。
                  </p>
                </div>
                <span
                  className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${
                    settings?.baidu_configured
                      ? "bg-emerald-100 text-emerald-700"
                      : "bg-slate-200 text-slate-600"
                  }`}
                >
                  {settings?.baidu_configured ? "可用" : "未配置"}
                </span>
              </div>
              <div className="mt-4 grid gap-3 sm:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-xs font-medium text-slate-500" htmlFor="translation-baidu-app-id">
                    App ID
                  </label>
                  <input
                    id="translation-baidu-app-id"
                    value={dirtyAppId ? baiduAppId : ""}
                    onChange={(event) => {
                      setBaiduAppId(event.target.value);
                      setDirtyAppId(true);
                    }}
                    onBlur={() => {
                      if (!dirtyAppId) return;
                      autoUpdate({ baidu_app_id: baiduAppId });
                    }}
                    placeholder={settings?.baidu_app_id_masked ?? "请输入 App ID"}
                    className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium text-slate-500" htmlFor="translation-baidu-secret">
                    Secret Key
                  </label>
                  <input
                    id="translation-baidu-secret"
                    value={dirtySecret ? baiduSecret : ""}
                    onChange={(event) => {
                      setBaiduSecret(event.target.value);
                      setDirtySecret(true);
                    }}
                    onBlur={() => {
                      if (!dirtySecret) return;
                      autoUpdate({ baidu_secret_key: baiduSecret });
                    }}
                    placeholder={
                      settings?.baidu_secret_key_masked ?? "请输入 Secret Key"
                    }
                    className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                  />
                </div>
              </div>
              {settings?.baidu_error ? (
                <p className="mt-3 text-xs text-red-500">{settings.baidu_error}</p>
              ) : pendingBaiduVerification ? (
                <p className="mt-3 text-xs text-slate-500">正在验证 API 凭据…</p>
              ) : !hasBaiduCredentials && (settings?.baidu_app_id_masked || settings?.baidu_secret_key_masked) ? (
                <p className="mt-3 text-xs text-orange-500">请同时填写 App ID 与 Secret Key。</p>
              ) : null}
            </section>
            <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm lg:col-span-2">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-slate-700">Ollama 本地翻译</p>
                  <p className="text-xs text-slate-500">
                    使用本地部署模型完成翻译任务，需先部署并启动 Ollama 服务。
                  </p>
                </div>
                <span
                  className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${
                    settings?.ollama_configured
                      ? "bg-emerald-100 text-emerald-700"
                      : "bg-slate-200 text-slate-600"
                  }`}
                >
                  {settings?.ollama_configured ? "可用" : "未配置"}
                </span>
              </div>
              <div className="mt-4 grid gap-3 sm:grid-cols-2">
                <div className="space-y-1 text-xs text-slate-500">
                  <p className="font-medium text-slate-600">服务地址</p>
                  <p className="rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-slate-600">
                    {settings?.ollama_base_url ?? "http://127.0.0.1:11434"}
                  </p>
                </div>
                <div className="space-y-1 text-xs text-slate-500">
                  <p className="font-medium text-slate-600">模型名称</p>
                  <p className="rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-slate-600">
                    {settings?.ollama_model ?? "未指定"}
                  </p>
                </div>
              </div>
              {settings?.ollama_error ? (
                <p className="mt-3 text-xs text-red-500">{settings.ollama_error}</p>
              ) : pendingOllamaVerification ? (
                <p className="mt-3 text-xs text-slate-500">正在尝试连接 Ollama 服务…</p>
              ) : (
                <p className="mt-3 text-xs text-slate-500">
                  如需自定义，请在服务器上通过环境变量或配置文件设置 `OLLAMA_BASE_URL`、`OLLAMA_MODEL`。
                </p>
              )}
            </section>
          </div>

          {statusMessage ? (
            <div className="flex flex-wrap items-center gap-3 text-xs text-slate-500">
              {statusMessage}
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
