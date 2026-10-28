# GDS 共享主题与材质 · 概念设计 0.2

本文记录当前共享主题的配色分工、来源适配与能力边界。视觉语义与代码供 GDS 应用复用，配置分别保存于各应用；跨应用统一配置另行设计。视觉语义见 [视觉语言](README.md)。本文对应 **设置 → 外观**；完整设置中心的导航、页面组织与作用域见 [设置中心](../settings/README.md)。

## 1. 产品契约

**桌面提供色彩，GDS 提供稳定的视觉语言。**

- 外观设置当前仅在本应用生效：主题来源、明暗偏好、字体和各表面不透明度等均保存到应用专属配置；不会自动同步到其它应用。
- 应用 UI 只消费解析后的语义主题，来源读取与配置持久化由各自模块负责。
- 本应用读取自己的配置；选择系统主题时可以只读桌面导出的配色，但这不意味着共享应用偏好。跨应用配置与同步不属于当前实现，也不引入主题后台服务。
- 读取外部主题并不接管它：不修改 Qt 全局环境、不执行 QML/QSS、不触发 Matugen、不覆盖用户桌面设置。
- “设置 → 外观”显示实际来源、明暗模式、来源路径、导入状态，以及“透明 / 模糊不可用”等能力信息。失败不能仍显示“已跟随桌面”。

## 2. 共享数据流

```text
Qt 调色板适配器 ─┐
DMS 导出适配器 ──┼─> 来源颜色 / 字体元数据
内置主题 ────────┘          │
                           ▼
                统一语义映射 + 缺失角色补全
                           │
              本应用用户偏好 / 高对比与材质偏好
                           │
                           ▼
                  不可变的有效主题快照
                           │
            GPUI Kit 主题桥接 + GDS 公共组件
                           │
                当前应用（公共组件可复用）
```

不用动态插件系统。首期采用明确的来源类型和解析函数即可。语义主题的发布是一次性替换完整快照，不让一个窗口半新半旧；当前不提供跨应用配置同步。

## 3. 初期实际支持什么

### 3.1 Qt：读取调色板，不执行 QWidget 样式

Qt 的平台主题、`QPalette`、`QStyle`、QSS 与 Kvantum 不是同一个接口。没有一个对所有 Linux 桌面都适用的“Qt 主题文件”。

以下 Qt 适配能力保留在代码层；当前原生设置界面暂不显示 Qt 来源选项，不删除既有配置或解析器：

1. **qt5ct / qt6ct 自定义调色板**：读取对应配置的 `[Appearance] custom_palette` 与 `color_scheme_path`，沿实际路径导入调色板。
2. **qtct 格式**：读取 `[ColorScheme]` 的 `active_colors`、`inactive_colors`、`disabled_colors`。这是按 `QPalette::ColorRole` 排列的数组，必须按 Qt 版本与角色数量解释，不能随意猜下标。
3. **已知 KDE `.colors` 基本颜色组导入**：用于 DMS 生成文件或用户明确选择的文件，解析 Window / View / Button / Selection 等组。不宣称复现 KDE 全部派生色、色彩效果和当前 Plasma 主题发现。
4. 没有启用自定义调色板、来源为未支持的后端或只有不透明的样式插件时，显示“该 Qt 来源尚不可导入”，允许手选调色板或使用内置主题。不能伪称与该 Qt 应用完全一致。

默认候选位置是 XDG 配置目录内的 `qt6ct/qt6ct.conf`、`qt5ct/qt5ct.conf`，但**生效配置优于固定文件名**。Qt5 与 Qt6 配置不同且无法判断目标时，让用户选择，不静默混合两者。字体和图标主题若可从已识别配置读出，可作为独立输入；无法识别时使用系统 / 用户设置。

### 3.2 DMS：读取生成的语义颜色

这里暂按 **DankMaterialShell** 理解 DMS。调研基线为 DMS **v1.6.3**；旧仓库链接目前重定向到 `duxlabs-dev/dux-shell`，因此下文使用固定版本，不把新主分支当成用户的安装版本。

推荐首版直接读取它已经生成的颜色数据：

```text
$XDG_CACHE_HOME/DankMaterialShell/dms-colors.json
默认：~/.cache/DankMaterialShell/dms-colors.json
```

该版本的结构包含 `mode`、`colors.dark`、`colors.light` 与 `dank16`。`Appearance.mode` 保存 `ModePreference`（跟随系统 / 浅色 / 深色），实际应用结果 `Mode` 只包含浅色 / 深色。默认跟随系统时，DMS 来源根据导出的 `mode` 选择配色组；显式浅色 / 深色忽略该活动字段，只读取用户请求的组。缺少请求的颜色组时明确报错，不悄悄换到另一组。再将 `primary`、`surface`、`on_surface` 等角色映射到 GDS。终端可在格式被识别时使用 `dank16`；缺失时使用与明暗模式匹配的完整内置 ANSI 调色板，不用 UI 的几个颜色冒充 16 色终端调色板。

**前提：DMS 已启用并生成相应 Matugen 输出。** 该 JSON 是内部导出数据，不是稳定公共 API；缓存可能缺失或陈旧，存在文件不证明当前桌面正在使用它。首版要限定已验证的结构，并允许手动选择来源路径。没有可用导出时，不通过执行用户 QML 或复制 DMS 主题逻辑来猜出“当前主题”。

DMS 还可能生成：

```text
$XDG_CONFIG_HOME/qt5ct/colors/matugen.conf
$XDG_CONFIG_HOME/qt6ct/colors/matugen.conf
$XDG_DATA_HOME/color-schemes/DankMatugen.colors
$XDG_DATA_HOME/color-schemes/DankMatugenLight.colors
$XDG_DATA_HOME/color-schemes/DankMatugenDark.colors
```

输出受模板开关和依赖检测影响，不保证每台机器都有这些文件。直接 DMS 导入不可用时，可以明确选择这些已生成的调色板；如果无法确定正在使用哪份明暗文件，不自动猜测。

**必须区分：DMS 导出的配色 ≠ 当前 Qt 真正生效的配色。** v1.6.3 的应用脚本会将 qtct 配置指向 KDE `.colors`，而标准上游 qt6ct 的解析器只读取三组颜色数组。发行版补丁、qt6ct-kde 与 qtengine 的行为不能混为一谈。GDS 可以识别已知数据格式，但这不证明其他 Qt 应用一定采用同一配色。

### 3.3 不包含在初期承诺中的能力

完整 Plasma 活跃主题发现、qtengine 自动发现、关闭 Matugen 时的 DMS 主题逻辑、任意 QStyle/QSS/Kvantum 渲染、GTK 样式照搬均不作为首版依赖。后续每增加一种来源只扩展公共适配层，不改每个应用。

这不影响首期交付“qtct 调色板导入”和“DMS 已生成配色跟随”，但不能将它们宣传成“兼容所有 Qt / DMS 主题”。

## 4. 固定配色分工

Files、Agents 与公共组件共用 `src/theme/palette.rs` 的角色快照和 Kit 桥接。DMS / Matugen、Qt 和内置来源只提供角色的值；应用不另设配色表。常规组件读取 Kit 的语义字段，自绘面板及状态层通过 `gds::theme::Colors::current(cx)` 读取同一快照。主题热更新一次替换整份快照，透明度设置只改变材质 alpha。

| GDS 语义 | Qt 输入 | DMS / Material 输入 | 说明 |
| --- | --- | --- | --- |
| `surface.window` | Window | surface | 窗口底色 |
| `surface.content` | Base | surface | 文件内容 / 输入内容底色 |
| `surface.sidebar` | Window / 派生层级 | surface_container_low | 缺失时回到 surface，并用分隔线保证层级 |
| `surface.control` | Button | surface_container | 普通控件背景 |
| `surface.panel` | Button / ButtonText | surface_container_high / on_surface | Agents 遥控面板等抬升表面；对应 `Colors.panel` / `panel_foreground` |
| `surface.overlay` | ToolTipBase / ToolTipText | surface_container_high / on_surface | 菜单、提示、拖动预览与弹出层；对应 Kit `popover` / `popover_foreground`，保持不透明 |
| `text.primary` | WindowText / Text | on_surface | 按所在表面选择成对的前景色 |
| `text.secondary` | 由有效前景派生 | on_surface_variant | 检查可读性，不机械降低全控件 alpha |
| `accent` | Accent，缺失则 Highlight | primary | 强调，不等同所有选中背景 |
| `selection.bg` / `fg` | Highlight / HighlightedText | primary_container / on_primary_container | 成对导入，不能强制白字 |
| `border.subtle` | Mid / 派生 | outline_variant | 优先保留表面区分度 |
| `border.control` | Mid / 派生 | outline | 输入框等需要辨认的边界；对应 Kit `input`，不替代细分隔线 |
| `focus` / `progress` | Highlight | primary | 焦点环、插入光标、进度和活动标记；不作为普通面板的染色 |
| `shadow` | 公共阴影色 | shadow | 仅用于投影；玻璃面板的边缘高光取自自身前景色 |
| `status.error` | 公共状态色补全 | error / on_error | Qt 标准角色没有完整业务状态语义 |
| `status.success` / `status.warning` | 公共状态色补全 | 公共状态色补全 | 成功、警告使用套件的明暗成对默认值；不把任意 secondary / tertiary 色解释成绿或黄 |
| disabled / inactive | 对应 ColorGroup | 基于语义角色生成 | 非活动窗口不等于禁用窗口 |

容器色与文字色成对使用。Files 的页签、文件行、卷行及容量文字，Agents 的会话、命令候选、附件、队列编辑项和书签标记，在选中时使用 `primary_container` / `on_primary_container`，不强制白字，也不保留选中前的次级文字色。正文用 `on_surface`，侧栏正文用所在来源的侧栏前景，分区标题、路径和辅助动作使用不透明的 `on_surface_variant`。

现有文本框和选择框采用描边式设计，统一通过 `ui::field::input` / `select` 构造：内部使用 `surface.content` 和正文前景，外框使用 `outline`，不从描边色推导底色。显式无外观的编辑器和嵌入式搜索框继续继承宿主表面。

中性悬停层统一取所在表面前景的 8%，按下层取 12%；有底色的按钮在主题层与各自的成对前景混合。拖放和框选填充统一取 `primary` 的 12%，轮廓使用 `primary`。持久选中使用完整的容器 / 前景配对，不能用按下色代替。公共 `ui::button::Button` 负责幽灵按钮的中性状态，避免 Kit 再自行调亮、调暗或在悬停时换成选中文字色；Files 与 Agents 不各自定义一组透明度。

玻璃遥控面板只取 `surface.panel`，采用统一半透明材质、圆角、边缘和投影；不额外混入白色、主色或大面积渐变。主内容和侧栏分别保持用户设置的不透明度，文字和图标不跟随表面透明度变淡。语法高亮、终端 ANSI 与内容图片沿用各自的内容配色，不用 UI 角色覆盖。

缺失角色由公共映射补齐：DMS 的 control 缺失时取导入的 content，panel 缺失时取 control，outline 缺失时取 subtle border；Qt 的 panel 使用 Button / ButtonText，overlay 继续使用 ToolTipBase / ToolTipText。外部来源缺失层级时不掺入内置主题的表面色。外部主题的布局、圆角和交互不会透传。

## 5. 来源选择、覆盖与动态更新

本节定义“设置 → 外观”所连接的主题行为；页面导航、表单组织和作用域统一遵循 [设置中心](../settings/README.md)，不在主题模块另建设置入口或外观抽屉。

当前界面的主题下拉只提供平级的“内置”和“系统主题”；Niri 环境下系统主题使用 DMS 的导出配色。Qt 解析器仍保留，原有 Qt 配置不被静默改写，但相关选项不显示。不存在仅凭缓存文件就宣称已经自动发现活跃桌面的行为。

主题和明暗的默认值都为跟随系统。明暗下拉还有独立的浅色 / 深色手动覆盖；切换或恢复主题来源不连带重置明暗与透明度。内置或保留的 Qt 来源在系统模式下读取平台明暗偏好。已有显式配置不被重写。来源缺少对应配色时应明确失败并保持上次有效外观；首次启动则明确使用内置安全回退，不能把新的系统默认再当作回退重试。操作图标保持 GDS 词汇，只有内容图标跟随已识别的系统图标主题。

取值优先级：**临时预览 → 本应用用户偏好 → 所选来源的有效角色 → 内置缺省角色**。主题模式与字体等设置分别解析，不能把几个互相冲突的来源任意拼接成一套“自动主题”。

Files 的外观配置保存在 `$XDG_CONFIG_HOME/gds/files/settings.toml` 的 appearance 分组，字体使用同一文件的 font 分组，默认值由 `Preferences::default()` 定义。旧的 `$XDG_CONFIG_HOME/gds/settings.toml` 只作为首次兼容导入源，不再写入；已有应用配置始终优先。各设置真实变化即应用并自动保存，恢复按钮只复位对应字段。透明度滑块只更新已应用材质的 alpha，不重新读取色板。应用布局 / 标签恢复放在各自的状态存储，不把色值存进标签数据。

`gds::theme::Observer` 为每个进程统一监听 DMS 导出、Qt 配置与解析后的色板文件。监听父目录以跟随原子替换，缺失路径创建后重新定位目录，托管符号链接同时监听其实际目标。内置 / Qt 来源的“跟随系统明暗”复用 GPUI 的平台外观通知，多个窗口只转发事件，不各自创建监听线程；关闭最早创建的窗口不终止进程监听。

变化后在后台读取并解析，成功才在 UI 线程应用；失败保留最后有效主题，继续等待后续变化。切换来源后旧结果不覆盖新选择，字体和透明度保持用户偏好，也不改写桌面文件或应用配置。事件源合并尚未消费的变化，不使用定时轮询，也不额外延迟刷新。

## 6. 透明、模糊与实际能力

| 能力 | 设计承诺 | 边界 |
| --- | --- | --- |
| 实色 | 所有支持环境的可用基线 | 关闭效果后不损失信息层级 |
| 半透明表面 | 可配置，作用于背景而非文字 / 图标 | 需要窗口本身允许 alpha；某些窗口系统 / 合成器需降级 |
| 桌面背板模糊 | 自动请求的可选增强 | GPUI `Blurred` 不保证支持；不能把浏览器 CSS 模糊当成 Linux 实现证据 |
| 菜单与对话框 | 默认不透明 | 尤其保障冲突、权限和危险操作的可读性 |

原生默认值由 [`src/design.rs`](../../src/design.rs) 统一定义：主界面背景默认 85% 不透明，侧边栏背景默认 50%，顶栏和底栏分别沿用所在列的材质。所有应用均在外观设置中提供两个独立的百分比滑块，修改立即应用并保存，恢复默认只重置所选表面。窗口默认支持 alpha，调整不透明度即可即时生效，不强制启用模糊；不透明度也不是读取 DMS 透明度的结果。窗口不得通过整体 opacity 把文字也变淡。Agents 的“自动”背景跟随公共窗口默认值，已有显式背景偏好保持优先。

调研时 GPUI 上游 Wayland 实现使用可选 KDE blur manager，X11 路径主要处理透明。即使某个扩展协议已存在，也不代表实际选用的 GPUI 版本或合成器已支持。实现时在目标 NixOS / Wayland 环境验证，不承诺 GNOME、KWin、Hyprland、Sway 与 X11 的效果相同。

## 7. NixOS / Home Manager 边界

- 尊重 XDG 目录与符号链接，不硬编码 `/home/用户名`、`/bin/bash` 或发行版资源目录。
- GDS 只消费配置，不写 `/nix/store`，不替用户安装 DMS、Qt 插件或修改会话环境。
- Home Manager 管理的 Qt 指针配置与 DMS 动态生成调色板应分开归属；不要让两个写入者争用同一份配置。
- 套件外观配置若是声明式只读文件，“设置 → 外观”提供临时预览和可复制的配置建议，而不是覆盖链接或写入一个悄悄抢优先级的配置。
- 当前未检查用户的桌面安装与 Home Manager 设置；兼容验收要使用实际版本、后端、XDG 路径和符号链接布局。

## 8. 实现验收场景

1. Qt 明确启用的自定义调色板被读取，活动 / 非活动 / 禁用状态不会混用。
2. DMS 更新配色和明暗后，两个 GDS 测试窗口都更新；原子替换不会导致永久停止跟随。
3. 来源缺失、格式不支持、禁用输出与陈旧缓存均不伪报成功，保留可用外观。
4. 浅色 / 深色、实色 / 半透明、明暗壁纸、系统缩放和字体放大下内容仍可读。
5. 不支持模糊时依旧可用；“设置 → 外观”能说明实际能力，材质变化不破坏公共网格的列宽与贯通边线。
6. Nix 声明式配置和只读来源不会被应用改写。
7. 外观是完整设置中心中的共享页面；返回工作区不改变文件标签、终端会话集合或 dock 状态。

## 9. 一手依据

以下是设计依据，不是本仓库已通过的运行测试。DMS 引用固定 v1.6.3，其他主分支需在实现时锁定具体版本。

- [Qt QPalette 角色与状态组](https://doc.qt.io/qt-6/qpalette.html)、[QStyle 与控件绘制](https://doc.qt.io/qt-6/qstyle.html)。
- [标准 qt6ct 调色板解析](https://github.com/trialuser02/qt6ct/blob/master/src/qt6ct-common/qt6ct.cpp)。
- [DMS 主题来源与缓存读取](https://github.com/duxlabs-dev/dux-shell/blob/v1.6.3/quickshell/Common/Theme.qml)、[导出 JSON 模板](https://github.com/duxlabs-dev/dux-shell/blob/v1.6.3/quickshell/matugen/templates/dank.json)。
- [DMS Matugen 生成与刷新](https://github.com/duxlabs-dev/dux-shell/blob/v1.6.3/core/internal/matugen/matugen.go)、[Qt 应用脚本](https://github.com/duxlabs-dev/dux-shell/blob/v1.6.3/quickshell/scripts/qt.sh)、[qtct 模板](https://github.com/duxlabs-dev/dux-shell/blob/v1.6.3/quickshell/matugen/templates/qtct-colors.conf)。
- [GPUI 窗口背景能力说明](https://docs.rs/gpui/0.2.2/gpui/enum.WindowBackgroundAppearance.html)、[Wayland 实现](https://github.com/zed-industries/zed/blob/main/crates/gpui_linux/src/linux/wayland/window.rs)、[X11 实现](https://github.com/zed-industries/zed/blob/main/crates/gpui_linux/src/linux/x11/window.rs)。
- [Home Manager Qt 模块](https://github.com/nix-community/home-manager/blob/master/modules/misc/qt/default.nix)。
