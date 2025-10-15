# 管理员权限设计方案

## 目标
- 管理操作（订阅源维护、规则配置、后续统计等）仅限认证管理员执行。
- 匿名访问仍可浏览文章、精选列表与搜索。
- 提供独立的 `/admin` 管理界面，便于扩展更多后台功能。

## 角色划分
- **访客**：匿名用户，只能查看文章与搜索，不可修改数据。
- **管理员**：登录后可管理订阅源、调整规则、查看统计等。

## 认证策略
### 凭证与存储
- 单个管理员账号，通过配置提供（例如 `ADMIN_USERNAME`、`ADMIN_PASSWORD_HASH`）。
- 密码推荐使用 Argon2（或 bcrypt）加盐哈希存储。
- 应用启动时加载到 `AppState`。

### 登录流程
1. 管理员访问 `/admin`。
2. 提交用户名与密码。
3. 后端校验成功后签发会话：
   - **优先方案**：返回含签名 token 的 HttpOnly Secure Cookie（`admin_session`），有效期短且可滑动刷新。
   - **备选方案**：返回 JSON token，由前端暂存在 `sessionStorage`，后续请求放入 `Authorization` 头。
4. 提供 `POST /admin/logout`，清除 Cookie 或注销 token。

### 会话校验
- 中间件/提取器在每次 `/admin/**` 或后台 API 调用时验证 token。
- 通过后，将 `AdminClaims` 注入请求上下文供业务使用。
- 失败返回 `401 Unauthorized`，必要时附带 `WWW-Authenticate`。
- 会话默认有效期 5 分钟，可实现滑动过期策略（在有效期内有操作则刷新过期时间），兼顾安全与易用。

## 后端调整
1. **配置扩展**
   - 在配置模块新增 `AdminConfig { username, password_hash }`。
- MVP 阶段可使用固定凭证（用户名 `admin`、密码 `123456`），后续再引入环境变量与哈希存储。
2. **认证模块**
   - 实现登录、注销 handler。
   - 编写 `AdminAuthLayer`（tower 中间件），负责校验会话。
3. **路由划分**
   - 公共 API 保持不变（`/articles`, `/articles/featured` 等）。
   - 管理 API 统一挂载在 `/admin/api`：
     - `GET /admin/api/feeds`
     - `POST /admin/api/feeds`
     - `DELETE /admin/api/feeds/:id`
     - `POST /admin/api/feeds/test`
     - 后续规则、统计接口也在该前缀下。
4. **错误处理**
   - 返回统一格式 `{ "error": { "message": "..." } }`。
   - 登录接口可考虑限流/退避，防止暴力破解。

## 前端调整
1. **路由结构**
   - 访客侧保留现有页面（`/`、`/featured`、`/search`）。
   - 新增 `/admin` 子应用：未登录时呈现登录表单，登录后显示订阅源管理。
   - 后续若新增 dashboard、settings 等页面，可在 `/admin` 内继续扩展子路由。
2. **状态管理**
   - 创建 `useAdminAuth` Hook 或 Context，管理登录状态与凭证。
   - 登录成功后在 `/admin` 内切换到订阅源管理视图，未登录访问时保持在登录表单。
   - 遇到后台 API 返回 `401` 时清空会话并提示重新登录。
3. **界面**
   - 后台独立导航栏、登录表单、错误提示。
   - 提供退出登录按钮。
4. **API 封装**
   - 新增 `adminRequest` 方法，自动附加 `Authorization` 或依赖 Cookie。
   - 公共 `getArticles` 等接口不受影响。

## 安全注意事项
- 强制 HTTPS 以保护凭证与会话。
- Cookie 建议设置 HttpOnly、Secure、SameSite。
- 要求强密码，支持后续轮换。
- 可记录管理员操作日志，便于审计。
- 登录接口建议加 rate limit 防止暴力破解。

## 迁移步骤
1. 实现后端认证模块并保护现有 feed 类接口。
2. 构建 `/admin` 前端登录与导航，将订阅源管理页面迁移过来。
3. 更新文档、配置说明、部署脚本（例如 Nginx、环境变量）。
4. 后续逐步接入统计看板、规则配置等功能。

## 后续扩展
- 多管理员账号与角色权限。
- 双因素认证、安全提醒。
- 更细粒度的操作审计。
- 统计报表、点击热度图表等高级功能。
