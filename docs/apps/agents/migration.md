# Agents 迁移

目标是将 BasG 完整迁为 GDS Agents，与 Files 平级，并采用 GDS 已有的设计语言。源快照为 `bas` 的 `24e39501e89dec4e986af8531043ae92013f9441`；迁移范围包括图形前端、客户端协议与前端配置，不包括 Bas 服务端和 bast CLI。

## 运行

```sh
just dev-agents
just dev-agents --server unix:///run/user/1000/bas/service.sock
just dev-agents --compact
just build-agents
```

等价 Cargo 命令为 `cargo run --features agents --bin gds-agents -- ...`。默认打开完整工作区；`--compact` 保留快捷输入窗口。Agents 连接独立运行的 Bas 服务，可使用原有 Unix socket、Windows pipe、TCP 和 TLS 连接配置。

Agents 的前端配置为 `$XDG_CONFIG_HOME/gds/agents/settings.json`，可用 `--settings-file` 或 `GDS_AGENTS_SETTINGS_FILE` 指定。缓存和会话导航状态分别位于用户 cache/state 目录中的 `gds/agents`。当前不会自动导入 BasG 用户配置，也不会写入 Bas 的配置文件。

## Nix 安装入口

`packages.<system>.gds-agents` 和 `apps.<system>.gds-agents` 提供独立构建及启动目标：

```sh
nix build .#gds-agents
nix run .#gds-agents
```

NixOS 可导入 `inputs.gds.nixosModules.gds-agents` 并设置 `programs.gds-agents.enable = true`。该模块仅安装前端，不启动或修改 Bas 服务；Files 的模块和默认包入口继续保留。

## 模块边界

- `src/bin/agents` 拥有会话、提示输入、附件、Markdown、认证、权限、连接、历史缓存、Agent 注册和安装界面。
- `src/bin/agents/client` 是应用私有的 **Bas 客户端库**，包名 `gds-agents-client`；保留原协议生成和配置持久化能力。它不在 `gds` 公共 lib 中，也没有运行服务端的二进制。
- 公共部分复用 `gds::theme` 的语义配色和材质、`gds::typography` 的字体与字号规则、`gds::ui::Frame` / `NavItem` 的框架与导航呈现，以及 `gds::notifications::Overlay` 的通知呈现。窗口各自持有通知队列，状态与焦点不跨窗口传递。
- 为多窗口应用读取并比较完整桌面主题，公共主题模块提供 `Appearance::prepare` / `Prepared::install`；Files 的 `theme::apply` 仍通过同一个安装路径执行，现有配色和布局没有重新设计。
- `src/ui/settings.rs` 只承载设置行、滑块、量化与键盘步进。Files 的偏好模型、列配置、设置页和 TOML 存储已移回 `src/bin/files/settings`；卷服务已移回 `src/bin/files/storage`，保留原路径、序列化字段与操作实现。
- Files 的专用快捷键表已移回 `src/bin/files/actions`。公共动作仅保留通知与取消；公共 `Shortcut`、键帽与前缀提示直接接收各应用自己的动作和作用域。
- Fluent 解析、静态标签缓存和基础消息由 `gds::i18n` 提供；Files 的文件／设备等文案归 `src/bin/files/i18n`，Agents 文案归 `src/bin/agents/i18n`。各应用注册自己的资源，公共目录不打包任何应用领域文案。
- Agents 专用的会话模型、缓存、协议、安装器、认证及对话界面不提升为套件基础服务。

## 许可证

本仓库第一方代码选用 MIT，保留迁入源码的 MIT 版权与许可文本。迁入来源记录在 `src/bin/agents/NOTICE`；品牌素材保留其来源说明。依赖仍按各自许可证分发，不将第三方代码改标为 MIT。当前迁移未引入 Zed 的 GPL 上层 Editor / Agent UI。

## 实现与验证

原有前端功能已迁入并适配 GPUI Kit 0.8，全部视图使用仓库现行 UIM，Cargo 图中只有一份 `gds-uim`。主窗口使用 GDS 的顶栏、侧栏、底栏和折叠导航；应用菜单包含设置与关于。会话行采用 38 px 密度、18 px Agent 图标和共同的固定角标，时间及目录在悬停时显示，保留双击重命名、草稿丢弃、归档、收藏、状态与上下文菜单。

设置中心在同一窗口内替换呈现，返回时恢复原会话与未发送草稿。分类使用公共 NavItem，Ctrl+Tab 仅切设置页。字体、字号和透明度等字段采用公共设置行、可搜索选择框与数值滑块，独立恢复默认。快捷键页、菜单和常规动作 tooltip 使用同一 ShortcutBadge，绑定由实际作用域解析。

全工作区 GUI 测试 **718 项通过、2 项原有性能测试忽略**：公共库 52 项、Agents 391 项、Files 187 项、Bas 客户端 88 项。workspace/all-targets 编译检查通过。工作流覆盖设置保留草稿、设置页和搜索输入的键盘作用域、实时权限批准、通知跨窗口隔离，以及迁入的会话、认证、历史和连接行为。

原生窗口已在 Niri 下启动，并在隔离 X11 环境中核对主窗口、紧凑模式、外观页和快捷键页；Files 的设置界面保持原有布局。独立临时 Bas 服务配合 ACP 测试 Agent，完成真实发送、流式回复、权限批准、重命名和固定；设置返回后未发送草稿保留；内置 Agent 的发送、归档和恢复通过原生操作验证。ACP 测试 Agent 的归档恢复被服务拒绝，原 `bast session restore` 也返回同样的 Invalid params，因而不将其记作前端通过项。

Nix 发布构建及包内测试通过（公共库 52 项、Agents 169 项）。已核对桌面入口、许可证／来源文件和动态库；直接运行安装产物完成原生启动、历史读取与发送／回复，无需开发环境。NixOS 模块开启后的实际配置求值通过。此次运行验证平台为 x86_64 Linux；Windows 和 macOS 尚未验收。
