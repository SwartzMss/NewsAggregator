# Ollama 翻译集成指南

## 适用场景

当需要在本地或私有环境中完成英文资讯的机器翻译，且希望避免付费 API 的额度限制时，可以使用 [Ollama](https://ollama.com/) 搭配 NewsAggregator 的内置适配器。Ollama 提供了统一的 HTTP 接口，可以拉起本地大模型并响应翻译请求。

## 前置准备

1. 安装并启动 Ollama（以 Linux 为例）：
   ```bash
   curl -fsSL https://ollama.com/install.sh | sh
   ollama serve
   ```
2. 拉取用于翻译的模型，例如：
   ```bash
   ollama pull qwen2.5:3b
   ```
   你也可以选择其它支持中文输出的模型，注意模型与显存占用的平衡。

## 配置项

系统仅从数据库读取 Ollama 配置（需在管理后台「翻译服务」界面中编辑并保存）。不再支持从 `config/config.yaml` 或环境变量回退读取：

| 配置项 | 说明 | 默认值 | 环境变量 |
| ------ | ---- | ------ | -------- |
| `translation.ollama_base_url` | Ollama 服务地址，例如 `http://127.0.0.1:11434` | 需在后台填写 | （不支持） |
| `translation.ollama_model` | 翻译使用的模型名称 | 需在后台填写 | （不支持） |
| `ai.ollama.timeout_secs` | HTTP 请求超时时间（秒） | `30` | `OLLAMA_TIMEOUT_SECS` |

若希望启动时默认选用 Ollama 作为翻译服务，可额外设置：

```bash
export TRANSLATOR_PROVIDER=ollama
```

## 管理后台中的状态

部署完成后进入「翻译服务」页面，可见 “Ollama 本地翻译” 卡片，并可以直接编辑服务地址与模型名称。保存后系统会自动写入数据库并触发连通性验证：

- **可用**：表明连通性验证通过，可以在下拉菜单中选择。
- **验证失败**：检查卡片中的报错信息，通常与服务地址/模型名不匹配有关。
- **未配置**：表示未提供有效地址或模型名称，或服务未启动。

切换默认翻译服务为 Ollama 时，会自动校验一次 `Title → 中文标题` 的示例翻译，以确认模型能按约定返回 JSON。

## 使用说明

- Prompt 复用了 Deepseek 翻译的模板，要求模型输出形如 `{"title": "...", "description": "..."}` 的 JSON（无额外文本）。请确认所选模型在该指令下能稳定输出 JSON。
- 后台「翻译内容范围」默认仅翻译标题，如需连摘要一起翻译，可开启“标题 + 摘要”开关。
- 若你同时配置了 Deepseek/Baidu，本地 Ollama 将与它们一起出现在备选列表中，系统会根据默认顺序尝试翻译并在失败时回退。

## 故障排查

1. **“验证失败：connection refused”**  
   - 确认 Ollama 服务已运行，后台中填写的服务地址或环境变量 `OLLAMA_BASE_URL` 指向正确地址。
2. **“无法解析翻译结果”**  
   - 模型可能没有按照提示输出 JSON。尝试在 Ollama CLI 中直接运行同样的提示语，调整模型/模板后再试。
3. **前台仍显示英文摘要**  
   - 确认后台“翻译内容范围”未开启摘要翻译，或模型返回的 `description` 字段为空。

完成以上配置后，即可使用本地大模型替代云端 API，实现近乎无限额度的翻译能力。
