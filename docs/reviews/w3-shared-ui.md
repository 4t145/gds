# w3：共享库、derive、GUI 状态审查证据

## 基线与范围

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`；未改变既有生产代码、配置或测试。已阅读全局 ub-mode 技能；审查目标优先于其中提交/推送建议，本轮不推送、不修复。与 w4/w5 协商取消初始重叠连接任务。

重点阅读：[derive](<../../crates/gds-settings-derive/src/lib.rs>)、[表单模型](<../../src/ui/settings/form/model.rs>)、[编辑器](<../../src/ui/settings/form/editor.rs>)及[现有表单测试](<../../src/ui/settings/form/tests.rs>)、[split](<../../src/ui/split.rs>)、[文件观察器](<../../src/desktop/watch.rs>)及[测试](<../../src/desktop/watch/tests.rs>)、共享主题的 DMS/Qt 解析与热更新、时间显示、[工作区移除](<../../src/bin/agents/workspace/navigation/removal.rs>)。对图像与链接路径另作独立源码复核，交 w8 去重。不是整个 workspace GUI 逐行穷尽审查。

## 确认问题 W3-1：符号链接父目录切换后观察器仍跟随旧目录（P2，低影响）

- 位置：[watch:40–53](<../../src/desktop/watch.rs#L40-L53>)、[watch:77–79](<../../src/desktop/watch.rs#L77-L79>)。
- 触发：配置文件是 `current/palette.json`，`current` 为目录符号链接；程序启动观察后，原子替换 `current` 链接，使其指向新的现存目录。
- 原因：仅将原始文件和最终 canonical 文件作为 targets，并观察其父目录；Linux inotify 对目录链接的 watch 跟随旧目录，链接自身所在父目录没有被观察。链接替换不向旧目录发出相关事件，故 refresh 不会被唤醒。
- 用户影响：依赖此 Sources 的主题/设置热更新继续显示旧快照，新目录后续修改也不会被跟随，直至其他事件触发刷新或重启；不涉及写入、删除或外传。
- 复现：隔离临时 Cargo 项目原样复制生产观察器及已有测试，既有测试 1/1 通过；增加“原子切换父目录符号链接后必须有通知”的回归实验，3 秒未收到通知而失败，退出 101。完整实验源见 [watch-tests](<w3-evidence/watch-tests.rs>)；并非修改仓库现有测试。原观察器副本与生产基线相同。
- 最小建议：解析并观察路径中符号链接组件的所在父目录，路径过滤器纳入这些组件；保留普通文件原子替换支持。增加目录链接切换及后续文件修改的回归用例。
- w5 已独立复制实验并复核：新增回归退出101、原有对照退出0；协作 Finding `3e33d3c4-12d2-47f7-bbfc-ad85c361a1fe`。失败日志：[watch-regression](<w3-evidence/watch-regression.log>)。

## 验证与工具记录

完整全仓测试日志：[cargo-workspace](<w3-evidence/cargo-workspace.log>)。

| 命令/实验 | 退出码与真实结果 |
|---|---|
| `nix develop --command cargo test --locked --workspace --features agents --lib --bins` | **0**；lib 84、agents 183、files 195、derive 2，共 464 项通过。另有子进程测试独立输出，不重复计数；少量 dead_code 警告。 |
| `nix develop --command cargo test --locked --features gui-tests --lib ui::settings::form::tests` | 0；3 表单 GUI 测试通过 |
| `nix develop --command cargo test --locked --workspace --features gui-tests --lib --bins` | **0**；lib92、agents427 passed/2 ignored、files201、derive2，共 **722 passed/2 ignored**；[完整日志](<w3-evidence/cargo-gui-workspace.log>)。没有真实桌面交互或覆盖率含义。 |
| 临时观察器项目 `cargo test --offline`（增加回归实验前） | 0，原有 1 测试通过 |
| 临时项目 `cargo test --offline observes_retargeted_parent_directory_symlink -- --nocapture` | 101，预期观察事件缺失，3 秒超时，见上文 |
| `crapper --root /tmp/gds-w3-audit.JtbNoO --no-coverage --threshold 10 <derive/lib.rs> <ui/settings/form> <desktop/watch.rs> <theme>` | 0；复杂度诊断，**Cov/CRAP 全部 N/A**，不能据此声称 CRAP<10 达标。derive expand CC41、Options::parse CC38，DMS parse CC26、Qt parse_kde CC14、watch refresh CC12。 |
| `dryer --root /tmp/gds-w3-audit.JtbNoO --threshold 0.8 <同上范围>` | 0；仅三对主题测试候选（1.00/0.83/0.83），未发现应据此确认的行为缺陷；明确目标 duplication<0.8，不机械去重测试。 |
| `mutator --scan src/desktop/watch.rs` | 0，6 mutation sites |
| `mutator --root /tmp/gds-w3-audit.JtbNoO --no-coverage --lines 55 --max-workers 1 --test-command 'cargo test --offline watches_missing_sources_and_atomic_replacements_through_managed_links' --verbose /tmp/gds-w3-audit.JtbNoO/src/watch.rs` | 初次0/3 killed，短默认 timeout 可能把编译超时算 killed，**不作为测试有效性证据**。 |
| 上条加 `--mutate-all --timeout-factor 10000`，测试输出重定向 `mutant-test.log` 以检查原因 | **3**；仅运行 line55 的三个变异，**2 killed/1 survived**。survivor 是 targets 比较反转，在现有测试触发的目录集合变化下不影响可见行为；不能直接视为缺陷。全文件6 sites 不等于6已执行；`--no-coverage` 不提供真实覆盖率。 |

指标原始 EDN：[复杂度](<w3-evidence/crap.edn>)、[重复](<w3-evidence/dry.edn>)、[变异](<w3-evidence/watch-mutation.edn>)。工具只用于诊断，不更改生产代码迎合指标。隔离项目使用现有本机缓存离线编译，没有安装依赖或修改项目配置。

## 排除/未确认与盲区

- 编辑器先 clone 再 set/validate，通过才发布 Changed；未发现所读路径对未编辑字段的覆写；derive 对重复 field id、非命名字段、泛型等显式拒绝。数值 f32 精度边界及极端自定义注解只记录为可扩展测试空间，不算当前实际配置缺陷。
- 热更新 generation 拒绝陈旧异步结果；主题解析显式验证所需颜色角色与格式；未发现所读解析路径的任意代码执行。
- 图像缓存缺失 backend 本地性限制的源码链已独立复核，归 w8 的发现；不重复编号，不推断已实现文件外传。链接点击通过 `cx.open_url(file:)` 的平台行为与共享 `desktop::open_file` 安全检查不一致，已提示 w8，未实机确认执行影响，不作为确认漏洞。
- 要求的原始全仓命令未启用 `gui-tests`；后续补跑启用该 feature 的全仓测试通过（722 passed/2 ignored），包含[表单 GUI 测试](<../../src/ui/settings/form/tests.rs>)及 workspace 交互测试。两次结果不可简单相加当作不同测试总数。仍无覆盖率数据。
- 无 macOS/Windows 实机或图形交互、屏幕阅读器验证；主题切换、IME、焦点与复杂异步窗口序列仍有盲区。共同源码观察不是绝对无漏洞保证。
