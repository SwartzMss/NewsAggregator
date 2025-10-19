import React, { useCallback, useMemo, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  adminLogin,
  adminLogout,
  UnauthorizedError,
  getTranslationSettings,
  updateTranslationSettings,
  getModelSettings,
  updateModelSettings,
  testModelConnectivity,
  getAiDedupSettings,
  updateAiDedupSettings,
} from "../../lib/api";
import { TranslationSettings, TranslationSettingsUpdate, AiDedupSettings, AiDedupSettingsUpdate, AdminLoginResponse } from "../../types/api";
import { FeedsPage } from "../Feeds";

type AdminSession = {
  token: string;
  expiresAt: number;
};

type Section = {
  key: string;
  label: string;
  description?: string;
  render: () => React.ReactNode;
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
    onSuccess: (data: AdminLoginResponse) => {
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

  const sections = useMemo<Section[]>(
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
        key: "model-config",
        label: "大模型配置",
        description: "配置 Deepseek / Ollama 参数，并手动测试可用性。",
        render: () => (
          <ModelSettingsPanel token={token} onUnauthorized={handleUnauthorized} />
        ),
      },
      {
        key: "translation",
        label: "翻译配置",
        description: "启用/关闭翻译、默认服务与内容范围。",
        render: () => (
          <TranslationSettingsPanel
            token={token}
            onUnauthorized={handleUnauthorized}
          />
        ),
      },
      {
        key: "ai-dedup",
        label: "AI 去重",
        description: "配置是否启用模型二次相似判定以及使用的提供商。",
        render: () => (
          <AiDedupSettingsPanel
            token={token}
            onUnauthorized={handleUnauthorized}
            onGotoModelSettings={() => setActiveSection("model-config")}
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
    if (!sections.some((section: Section) => section.key === activeSection)) {
      setActiveSection(sections[0].key);
    }
  }, [sections, activeSection]);

  const activeSectionData = useMemo(
    () => sections.find((section: Section) => section.key === activeSection) ?? sections[0],
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

function ModelSettingsPanel({ token, onUnauthorized }: { token: string; onUnauthorized: () => void }) {
  const qc = useQueryClient();
  const [deepseekKey, setDeepseekKey] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState<string | null>(null);
  const [ollamaModel, setOllamaModel] = useState<string | null>(null);
  const [dsBusy, setDsBusy] = useState(false);
  const [olBusy, setOlBusy] = useState(false);
  const [dsMsg, setDsMsg] = useState<string | null>(null);
  const [olMsg, setOlMsg] = useState<string | null>(null);

  const q = useQuery({
    queryKey: ["model-settings", token],
    queryFn: () => getModelSettings(token),
    enabled: !!token,
    retry: false,
  });
  useEffect(() => {
    if (q.error instanceof UnauthorizedError) onUnauthorized();
  }, [q.error, onUnauthorized]);

  useEffect(() => {
    setDsMsg(null);
    setOlMsg(null);
    setDeepseekKey("");
    setOllamaUrl(null);
    setOllamaModel(null);
  }, [q.data]);

  const currentOllamaUrl = q.data?.ollama_base_url ?? "";
  const currentOllamaModel = q.data?.ollama_model ?? "";
  const urlValue = ollamaUrl ?? currentOllamaUrl;
  const modelValue = ollamaModel ?? currentOllamaModel;

  const saveDeepseek = async (apiKey: string) => {
    setDsBusy(true);
    try {
      const data = await updateModelSettings(token, { deepseek_api_key: apiKey });
      qc.setQueryData(["model-settings", token], data);
      qc.invalidateQueries({ queryKey: ["model-settings", token] });
      setDsMsg("Deepseek 配置已保存");
    } catch (e) {
      setDsMsg((e as Error).message || "保存失败");
    } finally {
      setDsBusy(false);
    }
  };
  const saveOllamaBase = async (baseUrl: string) => {
    setOlBusy(true);
    try {
      const data = await updateModelSettings(token, { ollama_base_url: baseUrl });
      qc.setQueryData(["model-settings", token], data);
      qc.invalidateQueries({ queryKey: ["model-settings", token] });
      setOlMsg("Ollama 服务地址已保存");
    } catch (e) {
      setOlMsg((e as Error).message || "保存失败");
    } finally {
      setOlBusy(false);
    }
  };
  const saveOllamaModel = async (model: string) => {
    setOlBusy(true);
    try {
      const data = await updateModelSettings(token, { ollama_model: model });
      qc.setQueryData(["model-settings", token], data);
      qc.invalidateQueries({ queryKey: ["model-settings", token] });
      setOlMsg("Ollama 模型名称已保存");
    } catch (e) {
      setOlMsg((e as Error).message || "保存失败");
    } finally {
      setOlBusy(false);
    }
  };

  const test = async (provider: "deepseek" | "ollama") => {
    if (provider === "deepseek") setDsBusy(true); else setOlBusy(true);
    try {
      await testModelConnectivity(token, provider);
      if (provider === "deepseek") setDsMsg("Deepseek 测试通过"); else setOlMsg("Ollama 测试通过");
    } catch (e) {
      const msg = (e as Error).message || `${provider} 测试失败`;
      if (provider === "deepseek") setDsMsg(msg); else setOlMsg(msg);
    } finally {
      if (provider === "deepseek") setDsBusy(false); else setOlBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      {q.isLoading ? (
        <div className="text-sm text-slate-500">正在加载大模型配置…</div>
      ) : q.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600">
          {(q.error as Error).message || "大模型配置加载失败"}
        </div>
      ) : (
        <>
          <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <div className="flex items-center justify-between gap-3 mb-3">
              <div>
                <p className="text-sm font-medium text-slate-700">Deepseek</p>
                <p className="text-xs text-slate-500">填写 API Key 后点击测试。</p>
              </div>
              <button
                className="rounded-md bg-primary px-3 py-1.5 text-xs text-white disabled:opacity-60"
                disabled={dsBusy}
                onClick={() => test("deepseek")}
              >测试连接</button>
            </div>
            {dsMsg && <p className="mb-2 text-xs text-slate-600">{dsMsg}</p>}
            <input
              className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
              placeholder={q.data?.deepseek_api_key_masked ?? "请输入 Deepseek API Key"}
              value={deepseekKey}
              onChange={(e) => setDeepseekKey(e.target.value)}
              onBlur={() => {
                if (!deepseekKey) return;
                saveDeepseek(deepseekKey);
                setDeepseekKey("");
              }}
            />
          </section>

          <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <div className="flex items-center justify-between gap-3 mb-3">
              <div>
                <p className="text-sm font-medium text-slate-700">Ollama 本地</p>
                <p className="text-xs text-slate-500">设置本地服务地址与模型名称后点击测试。</p>
              </div>
              <button
                className="rounded-md bg-primary px-3 py-1.5 text-xs text-white disabled:opacity-60"
                disabled={olBusy}
                onClick={() => test("ollama")}
              >测试连接</button>
            </div>
            {olMsg && <p className="mb-2 text-xs text-slate-600">{olMsg}</p>}
            <div className="grid gap-3 sm:grid-cols-2">
              <input
                className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                placeholder="http://127.0.0.1:11434"
                value={urlValue}
                onChange={(e) => setOllamaUrl(e.target.value)}
                onBlur={() => {
                  const raw = ollamaUrl ?? currentOllamaUrl;
                  const trimmed = raw.trim();
                  if (trimmed === currentOllamaUrl.trim()) { setOllamaUrl(null); return; }
                  setOllamaUrl(trimmed);
                  saveOllamaBase(trimmed);
                }}
              />
              <input
                className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                placeholder="qwen2.5:3b"
                value={modelValue}
                onChange={(e) => setOllamaModel(e.target.value)}
                onBlur={() => {
                  const raw = ollamaModel ?? currentOllamaModel;
                  const trimmed = raw.trim();
                  if (trimmed === currentOllamaModel.trim()) { setOllamaModel(null); return; }
                  setOllamaModel(trimmed);
                  saveOllamaModel(trimmed);
                }}
              />
            </div>
          </section>
        </>
      )}
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
  const [deepseekKey, setDeepseekKey] = useState("");
  const [ollamaBaseUrlDraft, setOllamaBaseUrlDraft] = useState<string | null>(null);
  const [ollamaModelDraft, setOllamaModelDraft] = useState<string | null>(null);
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
    setDeepseekKey("");
    setOllamaBaseUrlDraft(null);
    setOllamaModelDraft(null);
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
      setFeedback("翻译配置已更新");
      setDirtyAppId(false);
      setDirtySecret(false);
      setDirtyDeepseek(false);
      setDeepseekKey("");
      setOllamaBaseUrlDraft(null);
      setOllamaModelDraft(null);
      setLocalTranslate(null);
    },
    onError: (err: Error) => {
      if (err instanceof UnauthorizedError) {
        onUnauthorized();
      } else {
        setFeedback(err.message || "大模型配置更新失败");
      }
      setLocalTranslate(null);
    },
  });

  const settings = settingsQuery.data;
  const provider = settings?.provider ?? "";
  const translationEnabled = settings?.translation_enabled ?? false;
  // 根据后端返回的 *_configured 字段动态生成可选 provider 列表；不再使用 fallback 默认值
  const options = useMemo(() => {
    if (!settings) return [] as string[];
    const list: string[] = [];
    if (settings.ollama_configured) list.push("ollama");
    return list;
  }, [settings]);
  const busy = mutation.isPending;
  const translateDescriptions = true;
  const currentOllamaBaseUrl = settings?.ollama_base_url ?? "";
  const currentOllamaModel = settings?.ollama_model ?? "";
  const hasOllamaConfig = Boolean(currentOllamaBaseUrl.trim());
  const pendingOllamaVerification =
    Boolean(settings) && hasOllamaConfig && !settings?.ollama_configured && !settings?.ollama_error;
  const ollamaBaseUrlValue = ollamaBaseUrlDraft ?? currentOllamaBaseUrl;
  const ollamaModelValue = ollamaModelDraft ?? currentOllamaModel;

  useEffect(() => {
    if (!token) return;
    if (!pendingOllamaVerification)
      return;
  const timer = window.setInterval(() => {
      queryClient.invalidateQueries({ queryKey: ["translation-settings", token] });
    }, 4000);
    return () => window.clearInterval(timer);
  }, [
    pendingOllamaVerification,
    queryClient,
    token,
  ]);

  const formatLabel = (value: string) => (value === "ollama" ? "Ollama 本地" : value);
  const available = (value: string) => {
    if (!settings) return false;
    if (value === "ollama") return settings.ollama_configured;
    return false;
  };
  const providerError = (value: string) => {
    if (!settings) return null;
    if (value === "ollama") return settings.ollama_error ?? null;
    return null;
  };
  const providerStatusSuffix = (value: string) => {
    if (!settings) return "（未配置）";
    if (available(value)) return "";
    const errorMessage = providerError(value);
    if (errorMessage) return "（验证失败）";
    if (value === "ollama") return hasOllamaConfig ? "（待验证）" : "（未配置）";
    return "（未配置）";
  };
  const statusHints: string[] = [];
  // Deepseek 验证提示已移除（大模型配置面板内手动测试）
  if (pendingOllamaVerification) {
    statusHints.push("Ollama 连通性验证中…");
  }
  if (settings || localTranslate !== null) {
    statusHints.push(
      translateDescriptions ? "当前翻译标题和摘要。" : "当前仅翻译标题。"
    );
  }
  const statusMessage = busy
    ? "正在更新大模型配置…"
    : feedback ?? (statusHints.length > 0 ? statusHints.join(" ") : null);

  const autoUpdate = (payload: TranslationSettingsUpdate) => {
    if (busy) return;
    if (Object.keys(payload).length === 0) return;
    mutation.mutate(payload);
  };

  const handleToggleDescriptions = () => {};

  return (
    <div className="space-y-5">
      {settingsQuery.isLoading ? (
        <div className="text-sm text-slate-500">正在加载大模型配置…</div>
      ) : settingsQuery.isError ? (
        <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600">
          {settingsQuery.error.message || "大模型配置加载失败"}
        </div>
      ) : (
        <>
          <div className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
              <div>
                <p className="text-sm font-medium text-slate-700">启用大模型翻译</p>
                <p className="text-xs text-slate-500">关闭后不使用大模型翻译，但仍可配置各服务的参数。</p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={translationEnabled}
                onClick={() => {
                  if (busy) return;
                  const next = !translationEnabled;
                  // 如果关闭翻译，不修改 provider；如果开启且当前 provider 不可用，保持原值，前端提示用户选择
                  mutation.mutate({ translation_enabled: next });
                }}
                disabled={busy}
                className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition ${translationEnabled ? 'bg-primary' : 'bg-slate-300'} disabled:opacity-60`}
              >
                <span
                  className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition ${translationEnabled ? 'translate-x-5' : 'translate-x-1'}`}
                />
              </button>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-slate-700">默认大模型服务</p>
                <p className="text-xs text-slate-500">默认仅使用 Ollama。</p>
              </div>
              <span className="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary">当前：Ollama 本地</span>
            </div>
          </div>

          <section className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
            <p className="text-sm text-slate-600">当前默认翻译标题与摘要（不可更改）。</p>
          </section>

          <div className="grid gap-4 lg:grid-cols-2">
            {/* Deepseek 配置移至“⼤模型配置”面板 */}

            {/* 模型配置（Ollama）已移至“⼤模型配置”面板 */}
            {/*
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
                <div className="space-y-2">
                  <label className="text-xs font-medium text-slate-500" htmlFor="translation-ollama-base-url">
                    服务地址
                  </label>
                  <input
                    id="translation-ollama-base-url"
                    value={ollamaBaseUrlValue}
                    onChange={(event) => setOllamaBaseUrlDraft(event.target.value)}
                    onBlur={() => {
                      if (busy) return;
                      const raw = ollamaBaseUrlDraft ?? currentOllamaBaseUrl;
                      const trimmed = raw.trim();
                      const currentTrimmed = currentOllamaBaseUrl.trim();
                      if (trimmed === currentTrimmed) {
                        setOllamaBaseUrlDraft(null);
                        return;
                      }
                      setOllamaBaseUrlDraft(trimmed);
                      autoUpdate({ ollama_base_url: trimmed });
                    }}
                    placeholder="http://127.0.0.1:11434"
                    className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium text-slate-500" htmlFor="translation-ollama-model">
                    模型名称
                  </label>
                  <input
                    id="translation-ollama-model"
                    value={ollamaModelValue}
                    onChange={(event) => setOllamaModelDraft(event.target.value)}
                    onBlur={() => {
                      if (busy) return;
                      const raw = ollamaModelDraft ?? currentOllamaModel;
                      const trimmed = raw.trim();
                      const currentTrimmed = currentOllamaModel.trim();
                      if (trimmed === currentTrimmed) {
                        setOllamaModelDraft(null);
                        return;
                      }
                      setOllamaModelDraft(trimmed);
                      autoUpdate({ ollama_model: trimmed });
                    }}
                    placeholder="qwen2.5:3b"
                    className="w-full rounded-md border border-slate-300 px-3 py-2 text-xs shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
                  />
                </div>
              </div>
              {settings?.ollama_error ? (
                <p className="mt-3 text-xs text-red-500">{settings.ollama_error}</p>
              ) : pendingOllamaVerification ? (
                <p className="mt-3 text-xs text-slate-500">正在尝试连接 Ollama 服务…</p>
              ) : (
                <p className="mt-3 text-xs text-slate-500">
                  填写本地服务地址与模型名称后会自动验证连通性，留空则禁用 Ollama 翻译。
                </p>
              )}
            </section>
            */}
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

function AiDedupSettingsPanel({
  token,
  onUnauthorized,
  onGotoModelSettings,
}: {
  token: string;
  onUnauthorized: () => void;
  onGotoModelSettings: () => void;
}) {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery<AiDedupSettings, Error>({
    queryKey: ["ai-dedup-settings", token],
    queryFn: () => getAiDedupSettings(token),
    enabled: Boolean(token),
    retry: false,
  });

  useEffect(() => {
    if (settingsQuery.error instanceof UnauthorizedError) {
      onUnauthorized();
    }
  }, [settingsQuery.error, onUnauthorized]);

  const mutation = useMutation<AiDedupSettings, Error, AiDedupSettingsUpdate>({
    mutationFn: (payload: AiDedupSettingsUpdate) => updateAiDedupSettings(token, payload),
    onSuccess: (data: AiDedupSettings) => {
      queryClient.setQueryData(["ai-dedup-settings", token], data);
      queryClient.invalidateQueries({ queryKey: ["ai-dedup-settings", token] });
    },
    onError: (err: Error) => {
      if (err instanceof UnauthorizedError) {
        onUnauthorized();
      }
    },
  });

  const settings = settingsQuery.data;
  const busy = mutation.isPending;

  const toggleEnabled = () => {
    if (!settings) return;
    const next = !settings.enabled;
    let provider = settings.provider || undefined;
    if (next && !provider) {
      // default provider selection when enabling and none chosen
      provider = settings.deepseek_configured
        ? "deepseek"
        : settings.ollama_configured
        ? "ollama"
        : undefined;
      // if still none available, guide user to model settings instead of failing the request
      if (!provider) {
        onGotoModelSettings();
        return;
      }
    }
    mutation.mutate({ enabled: next, provider });
  };

  const changeProvider = (value: string) => {
    if (!settings) return;
    const provider = value || undefined;
    mutation.mutate({ provider });
  };

  if (settingsQuery.isLoading) {
    return <div className="text-sm text-slate-500">正在加载 AI 去重配置…</div>;
  }
  if (settingsQuery.isError) {
    return (
      <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600">
        {settingsQuery.error.message || "AI 去重配置加载失败"}
      </div>
    );
  }
  if (!settings) return null;

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
          <div>
            <p className="text-sm font-medium text-slate-700">启用 AI 去重</p>
            <p className="text-xs text-slate-500">
              开启后对高相似文章进行模型二次判定，减少重复内容入库。
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={settings.enabled}
            onClick={toggleEnabled}
            disabled={busy}
            className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition ${settings.enabled ? 'bg-primary' : 'bg-slate-300'} disabled:opacity-60`}
          >
            <span
              className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition ${settings.enabled ? 'translate-x-5' : 'translate-x-1'}`}
            />
          </button>
        </div>
        <div className="mt-2">
          <p className="text-xs text-slate-500">
            当前判定参数：threshold = {settings.threshold}, max_checks = {settings.max_checks}。
            修改需代码调整，前端只读展示。
          </p>
        </div>
      </div>
      <div className="rounded-lg border border-slate-200 bg-white px-5 py-4 shadow-sm">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
          <div>
            <p className="text-sm font-medium text-slate-700">模型提供商</p>
            <p className="text-xs text-slate-500">
              启用时必须选择一个已配置的提供商；不做自动校验，后台按选择调用。
            </p>
          </div>
          <span className="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary">
            {settings.provider ? settings.provider : '未选择'}
          </span>
        </div>
        <select
          value={settings.provider || ''}
          onChange={(e) => changeProvider(e.target.value)}
          disabled={busy}
          className="w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm shadow-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30 disabled:cursor-not-allowed disabled:opacity-70"
        >
          <option value="">(未选择)</option>
          <option value="deepseek" disabled={!settings.deepseek_configured}>Deepseek</option>
          <option value="ollama" disabled={!settings.ollama_configured}>Ollama</option>
        </select>
        <p className="mt-3 text-xs text-slate-500">
          {settings.enabled
            ? settings.provider
              ? '已启用，后台会在相似度触发区间调用该模型进行判定。'
              : '已启用但未选择 provider，后台将跳过模型判定。'
            : '未启用，后台不会进行模型二次判定。'}
        </p>
        {(!settings.deepseek_configured && !settings.ollama_configured) ||
          (settings.provider === 'deepseek' && !settings.deepseek_configured) ||
          (settings.provider === 'ollama' && !settings.ollama_configured) ||
          !settings.provider ? (
            <button
              type="button"
              onClick={onGotoModelSettings}
              className="mt-2 rounded-md border border-slate-300 px-2 py-1 text-xs text-slate-600 hover:bg-slate-50"
            >
              前往大模型配置
            </button>
          ) : null}
      </div>
    </div>
  );
}
