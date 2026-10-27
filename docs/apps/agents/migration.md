# Agents 迁移

目标是将 BasG 完整迁为 GDS Agents，与 Files 平级，并采用 GDS 已有的设计语言。源快照为 `bas` 的 `24e39501e89dec4e986af8531043ae92013f9441`；迁移范围包括图形前端与前端配置；客户端协议直接依赖 BAS 仓库，不复制 Bas 服务端和 bast CLI。

## 运行

```sh
just dev agents
just dev agents --server unix:///run/user/1000/bas/service.sock
just dev agents --compact
just build agents
just release agents
```

等价 Cargo 命令为 `cargo run --features agents --bin gds-agents -- ...`。默认打开完整工作区；`--compact` 保留快捷输入窗口。Agents 连接独立运行的 Bas 服务，可使用原有 Unix socket、Windows pipe、TCP 和 TLS 连接配置。

Agents 的前端配置为 `$XDG_CONFIG_HOME/gds/agents/settings.json`，可用 `--settings-file` 或 `GDS_AGENTS_SETTINGS_FILE` 指定。缓存和会话导航状态分别位于用户 cache/state 目录中的 `gds/agents`。当前不会自动导入 BasG 用户配置，也不会写入 Bas 的配置文件。

## Nix 安装入口

`packages.<system>.gds-agents` 和 `apps.<system>.gds-agents` 提供独立构建及启动目标：

```sh
nix build .#gds-agents
nix run .#gds-agents
```

NixOS 可导入 `inputs.gds.nixosModules.gds-agents` 并设置 `programs.gds-agents.enable = true`。该模块仅安装前端，不启动或修改 Bas 服务。套件安装与默认模块的用法见 [NixOS 安装](../../nixos.md)。

## 模块边界

- `src/bin/agents` 拥有会话、提示输入、附件、Markdown、认证、权限、连接、历史缓存、Agent 注册和安装界面。
- `bas-client` 来自 [BAS 仓库](https://github.com/4t145/bas/tree/gds-client-library/crates/client)，由 Cargo 固定 Git revision。默认仅包含协议模型、认证数据和客户端传输，不生成服务端接口，不包含监听器、服务端 TLS 或 daemon 配置存储；`test-support` 仅供模拟 RPC 服务的测试使用。
- `src/bin/agents/configuration` 拥有前端外观与连接配置，复用 `gds::configuration::File` 的原子写入、冲突检测和托管文件保护。旧 JSON 中的 daemon 字段保持原样；管理界面通过带 revision 的 RPC 读取和修改服务配置，未连接时保留草稿，禁止直接写本地 daemon 文件。
- 公共部分复用 `gds::theme` 的语义配色和材质、`gds::typography` 的字体与字号规则、`gds::ui::Frame` / `NavItem` 的框架与导航呈现，以及 `gds::notifications::Overlay` 的通知呈现。窗口各自持有通知队列，状态与焦点不跨窗口传递。
- 为多窗口应用读取并比较完整桌面主题，公共主题模块提供 `Appearance::prepare` / `Prepared::install`；Files 的 `theme::apply` 仍通过同一个安装路径执行，现有配色和布局没有重新设计。
- `src/ui/settings.rs` 只承载设置行、滑块、量化与键盘步进。Files 的偏好模型、列配置、设置页和 TOML 存储已移回 `src/bin/files/settings`；卷服务已移回 `src/bin/files/storage`，保留原路径、序列化字段与操作实现。
- Files 的专用快捷键表已移回 `src/bin/files/actions`。公共动作仅保留通知与取消；公共 `Shortcut`、键帽与前缀提示直接接收各应用自己的动作和作用域。
- Fluent 解析、静态标签缓存和基础消息由 `gds::i18n` 提供；Files 的文件／设备等文案归 `src/bin/files/i18n`，Agents 文案归 `src/bin/agents/i18n`。各应用注册自己的资源，公共目录不打包任何应用领域文案。
- Agents 专用的会话模型、缓存、协议、安装器、认证及对话界面不提升为套件基础服务。

## 主窗口导航

会话管理、添加智能体、About 与设置在当前主窗口内覆盖工作区内容，使用左上角返回按钮逐级返回；会话管理、添加智能体和 About 的返回动作同时支持 Esc、Alt+Left 和 Ctrl+W（macOS 为 Cmd+W）。从设置打开添加智能体时先返回设置，再返回会话。切换页面不会重建底层工作区、清除未发送草稿或关闭后台会话；重新打开历史会话时回到它所属连接的工作区。

紧凑／展开属于窗口样式，切换按钮放在顶栏最右侧，不在底栏显示。从紧凑模式进入管理页会临时扩大窗口，返回工作区时恢复原模式。Niri 下隐藏原生窗口控制按钮时，这个应用模式切换仍保留。

本轮 Agents GUI 回归 403 项通过、2 项性能测试忽略，覆盖嵌套返回、紧凑模式恢复、草稿保留、同窗口会话恢复及已有设置／会话操作。

## 共享服务与设计语言

- `gds::theme::Observer` 是每进程一个事件源，监听 DMS、Qt 配置及解析后的色板路径，跟随文件原子替换和托管符号链接；窗口仅转发 GPUI 平台外观事件。应用保留自己的偏好模型，解析失败保留最后有效外观。
- `gds::ui::Shortcut`、`ShortcutBadge` 与 `keytips` 直接查询动作绑定；tooltip、快捷键页和按住修饰键的提示使用同一来源。受限尺寸、可滚动且保留字体回退的文本提示由 `gds::ui::tooltip` 提供。
- `gds::ui::button` 与 `gds::ui::window` 提供统一控件指针策略、原生窗口按钮和标题栏；窗口的业务关闭与最大化行为由应用注入。
- `gds::notifications` 按窗口登记独立队列，窗口关闭即释放；`surface` 可包装任意视图。后台结果可用 `Report::show_in` 指定原窗口，关闭后不会转送到另一窗口。Agents 的诊断日志仍由应用拥有。
- `gds::time` 使用一个进程时钟刷新所有窗口，Files 与 Agents 共用时间和语言规则，BAS 时间戳的转换留在 Agents。
- `gds::desktop::clipboard` 只共享带截止时间和大小限制的管道读取。Files 的复制／剪切协议和 Agents 的附件解析各自维护。
- GPUI 已负责桌面 Portal 的系统明暗通知，不另建通用 D-Bus 层。UDisks2 的卷、挂载与设备动作只属于 Files；会话、缓存和认证流程只属于 Agents。

## 许可证

本仓库第一方代码选用 MIT，保留迁入源码的 MIT 版权与许可文本。迁入来源记录在 `src/bin/agents/NOTICE`；品牌素材保留其来源说明。依赖仍按各自许可证分发，不将第三方代码改标为 MIT。当前迁移未引入 Zed 的 GPL 上层 Editor / Agent UI。

## 实现与验证

### 公共层整理（2026-10-27）

GDS 从 GitHub 固定引用 `bas-client` 的 `9ccfcdb960213ba4f506e93241d9bcd78de3a0b4`，Cargo 与 Nix 依赖下载、固定内容校验及全部目标编译检查通过。默认 Files 依赖图没有 BAS；普通 Agents 构建只启用客户端默认 feature，服务端测试接口只由 dev-dependency 启用。

使用远程依赖的完整 GUI 回归通过 **638 项**：公共库 54、Agents 397、Files 187；2 项原有性能测试忽略。覆盖真实文件监听的缺失路径与原子替换、托管符号链接、主题多窗口更新、快捷键与通知作用域、配置冲突及只读保护、离线草稿保留和带 revision 的 RPC 保存。BAS 仓库另外通过默认客户端 61 项测试、包含服务端的 342 项测试、BasG 编译及 3 项 Nix 源码检查。

Files 的独立 Nix 发布构建及包内测试通过：公共库 54、Files 186，1 项按原有规则过滤。发布包使用已锁定的远程依赖。

### 首次迁移记录

原有前端功能已迁入并适配 GPUI Kit 0.8，全部视图使用仓库现行 UIM，Cargo 图中只有一份 `gds-uim`。主窗口使用 GDS 的顶栏、侧栏、底栏和折叠导航；应用菜单包含设置与关于。会话行采用 38 px 密度、18 px Agent 图标和共同的固定角标，时间及目录在悬停时显示，保留双击重命名、草稿丢弃、归档、收藏、状态与上下文菜单。

设置中心在同一窗口内替换呈现，返回时恢复原会话与未发送草稿。分类使用公共 NavItem，Ctrl+Tab 仅切设置页。字体、字号和透明度等字段采用公共设置行、可搜索选择框与数值滑块，独立恢复默认。快捷键页、菜单和常规动作 tooltip 使用同一 ShortcutBadge，绑定由实际作用域解析。

初次迁移时全工作区 GUI 测试 **718 项通过、2 项原有性能测试忽略**：公共库 52 项、Agents 391 项、Files 187 项、Bas 客户端 88 项。workspace/all-targets 编译检查通过。工作流覆盖设置保留草稿、设置页和搜索输入的键盘作用域、实时权限批准、通知跨窗口隔离，以及迁入的会话、认证、历史和连接行为。

原生窗口已在 Niri 下启动，并在隔离 X11 环境中核对主窗口、紧凑模式、外观页和快捷键页；Files 的设置界面保持原有布局。独立临时 Bas 服务配合 ACP 测试 Agent，完成真实发送、流式回复、权限批准、重命名和固定；设置返回后未发送草稿保留；内置 Agent 的发送、归档和恢复通过原生操作验证。ACP 测试 Agent 的归档恢复被服务拒绝，原 `bast session restore` 也返回同样的 Invalid params，因而不将其记作前端通过项。

Nix 发布构建及包内测试通过（公共库 52 项、Agents 169 项）。已核对桌面入口、许可证／来源文件和动态库；直接运行安装产物完成原生启动、历史读取与发送／回复，无需开发环境。NixOS 模块开启后的实际配置求值通过。此次运行验证平台为 x86_64 Linux；Windows 和 macOS 尚未验收。
