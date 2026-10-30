# GDS 全仓 ub-mode Swarm 审查报告

## 结论摘要

**本报告第1–7节保留原审查轮次记录；用户随后授权的修复轮次见第8节及[修复摘要](<repair-summary.md>)。原审查结论和旧测试数量不得当作修复后验证。**

原轮次为**审查而非修复**。八个 worker 自主协商范围并交叉复核，确认 **5 项去重问题：2 项 P1、3 项 P2**。优先处理跨设备移动的数据丢失及远端 Markdown 图片加载的本地特殊文件无界读取，其次处理 PTY 自动提交、配置观察漏通知和开发安装恢复失败。未修改生产代码、配置或既有测试，未推送、部署或合并用户分支；Swarm accepted head 的集成仅包含审查报告及其证据。

要求的 Rust 全工作区命令通过（464 passed）；额外 GUI feature 全仓测试通过（722 passed、2 ignored）；要求的 portal Nix check 退出0。**测试通过不等于无漏洞、覆盖率充分或完成真实桌面/所有平台验证。**本轮没有真实行覆盖率，因此不宣称 CRAP<10 达标。

## 1. 基线、代码范围与审查约束

- 生产基线：`2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`（`fix(agents): avoid stacking terminal surface backgrounds`）。报告所涉源码行号均对应此基线，之后集成只增加审查文档。
- `git ls-tree -r --name-only <基线>` 得到 **559 个跟踪文件**，完整清单见 [基线文件清单](<baseline-tracked-files.txt>)。其中475 Rust、3 Python、9 Nix；agents子树307文件、files子树126文件、共享src83文件，其余为derive、打包、脚本、配置与文档/资源。
- 输入范围为整个跟踪树；排除 `.agents/worktrees`、构建输出及第三方依赖内部实现。必要时只检查依赖调用边界和锁定版本，不把它们说成已完成第三方审计。
- 全局 `/home/atlas/.agents/skills/ub-mode/SKILL.md` 已加载。采用先复现、边界验证、最小实验、保留日志的原则；owner 的“只审查、不得生产修复/推送”优先于技能中的提交推送建议。
- 目标阈值 **CRAP<10，duplication<0.8**。工具用于诊断，不机械重构；无真实coverage时只报告CC，CRAP/Cov均N/A。Nix不受工具支持，明确采用人工审查/求值。
- 这是全树盘点与分层风险审查，**不是559文件逐行穷尽证明**。源码有通读与关键入口抽查之分，测试/资源更多通过执行、关联检查和清单核对覆盖；未深审路径在下表及盲区列出。

## 2. 覆盖矩阵与八人协作

| 子系统/范围 | 责任与方式 | 覆盖维度/证据 | 剩余边界 |
|---|---|---|---|
| 文件复制/移动、删除、回收站、重命名 | w1发现，w7复核，w6接续落盘；原模块隔离实验+源代码 | correctness、数据丢失、symlink/no-replace、提交顺序；[files分项](<w1-files-operations.md>) | 未穷尽多程序竞争/断电/特殊FS；重命名GUI不是逐行穷尽 |
| files导航、排序、目录、选择/tabs、设置 | w4补审关键实现与测试 | pending id取消、稳定排序、legacy只读导入、配置列校验；Finding `3c8a492a-e3c3-4595-b11f-6e6cc569bd55` | selection/tabs/settings关键入口抽查；ACL/NSS/网络FS未实测 |
| Python portal、picker、文件剪贴板 | w7逐行Python与协议；D-Bus测试及有界变异 | trust boundary、非UTF8、取消、子进程、过滤器、64MiB限制；[portal分项](<w7-portal-boundaries.md>) | 无真实compositor、GTK/native modal焦点/文档授权全链路 |
| 设备/storage service/linux/usage/model | w7静态审查与既有测试 | 身份复查、互斥、UDisks超时、容量模型；补审Finding `1cdfc865-b3d7-403f-9c37-ef93a815997d` | 无物理USB/光驱、polkit、拔插压力 |
| 共享terminal/PTY、认证终端及local terminal | w2发现；w1/w4/w5复核，w5接续 | 终端注入、排队/取消、进程生命周期；真实Session实验；[终端分项](<w2-terminal.md>) | shell协作协议、进程树极端竞态、IME实机未穷尽 |
| agents连接、协议、权限、会话、提交 | w4通读关键路径，读关联测试 | session/cursor/turn校验、重连、deadline/drop、mTLS/transport；[协议分项](<ub-mode-agents-protocol.md>) | 真实daemon/证书轮换、RPC洪泛、网络断连压力未实测 |
| agents配置、缓存/SQL、历史、ACP、主题 | w5通读及路径隔离测试/变异 | section合并、权限/hardlink/symlink、串行写入、generation、512MiB LRU；[持久化分项](<w5-persistence.md>) | 掉电、多进程首次建库、完整daemon quit未验证 |
| shared UI/settings/derive/theme/watch/time/desktop | w3重点通读，w5复核watcher | 类型/边界、热更新、表单发布、路径观察；[共享分项](<w3-shared-ui.md>) | 不把全仓GUI测试当真实桌面、辅助功能或覆盖率 |
| agents workspace composer/queue/outbox/navigation/rename | w3补审全文/关键回调+GUI suite | Entity身份、目录epoch、draft generation、receipt相关释放；Finding `bceca995-05e4-40a6-a34c-acf128e94f17` | 全状态图、native picker跨后端异步真实序列未穷尽 |
| 附件、图像、URL、HTTP、音频与剪贴板 | w8审查，w3/w4复核；w4接续媒体证据 | 不可信内容到本地资源、大小/超时、加载依赖边界；[媒体分项](<ub-mode-agents-media-w8.md>) | 无完整GUI OOM实测、外传证明或全媒体实际mutation |
| agents catalog/registry/icon/diagnostics | w5补审完整关键文件 | RPC安装来源、icon限额/缓存权限、日志限额与结构字段redact；Finding `c1d623cb-519e-4360-b53a-d8517455aa8d` | daemon安装内部不属本仓；自由文本泄密/解码像素压力未实测 |
| Nix/flake、just、install-dev、Cargo锁与manifest | w6通读全部Nix/脚本，求值、Python复现 | 打包/模块、恢复、平台、固定rev/hash/checksum；[打包分项](<w6-nix-packaging.md>) | 无完整release构建/VM、advisory DB扫描或aarch64实机 |
| 测试、文档、i18n、静态资源、license/notices | 全树清单+关联阅读，w3全工作区/GUI、w7 portal执行 | 回归充分性、资源路径、许可元数据与非UTF8测试 | 非所有文案/视觉资源逐项核验；未作完整许可证法律审计 |

自主协调过程：w2/w4最初终端范围重叠后公开去重；w3从重叠连接范围转shared UI/derive；w5后续补catalog，w4补files GUI，w3补workspace，w7补picker后段/fallback/cut。w1/w2/w8后因会话不可用，Task CAS分别由w6/w5/w4接续证据落盘，保留原作者及独立复核来源，不把接续重复算成新增发现。

## 3. 已确认发现（按优先级排序，合并同源路径）

### F-01 · P1 · 跨设备移动删除未复制的并发新增子文件

- **位置**：[transfer:368–379](<../../src/bin/files/operations/transfer.rs#L368-L379>)，调用顺序[415–423](<../../src/bin/files/operations/transfer.rs#L415-L423>)。
- **触发/原因**：跨设备rename返回XDEV，目录copy/commit后另一程序新增子文件；删除阶段只比较根dev/ino，继而`remove_dir_all`，不确认所有当前子项已复制。
- **影响**：操作成功但新文件在源/目标均不存在，永久丢失。普通同设备rename不走此路径。
- **证据/复核**：w1原模块隔离harness在真实方法之间插入确定性并发写；14既有+1缺陷断言通过，输出文件两端均不存在；w7独立运行及w6第三次重跑确认。[复现日志](<w1-evidence/reproduction.log>)、[实验源](<w1-evidence/audit-test.rs>)、[重现步骤](<w1-files-operations.md>)。这是fallback组成步骤实验，不是实际双挂载GUI端到端移动。
- **建议**：只删除已验证已复制且未改变的条目，目录非递归清理；变化时保留源并报告部分成功。根inode/mtime再检查不能单独解决TOCTOU，需明确并发写入/快照语义。

### F-02 · P1 · 远端Markdown图片可触达本地特殊文件无界读取

- **位置**：[images:244–248](<../../src/bin/agents/workspace/images.rs#L244-L248>)未区分后端本地性；[372–375](<../../src/bin/agents/workspace/images.rs#L372-L375>)把file URI解析为本地Resource。远端chunk至Markdown链见[协议复核](<ub-mode-agents-protocol.md#独立交叉复核-a远端-markdown-图像可触达本地资源>)。
- **触发/原因**：远端agent/daemon输出含`![](file:///dev/zero)`等图片引用，显示时自动加载；调用锁定的gpui-pre 0.3.9 ImageAssetLoader本地分支，无普通文件/字节总量限制地`fs::read`。HTTP超时不能限制这个本地分支。
- **影响**：不可信远端内容可触发客户端本地特殊文件读取和内存耗尽/可用性风险；**本轮没有证明图片或本地秘密已外传，也没有实测完整GUI崩溃/abort**。
- **证据/复核**：w8追踪依赖实现，用相同读取操作在RLIMIT_AS=128MiB的隔离Rust子进程读取`/dev/zero`，得到`read error: out of memory`；w3/w4独立确认本地性边界缺失及远端消息可达链。完整操作/依赖位置和证据见[媒体分项](<ub-mode-agents-media-w8.md>)。风险等级基于可达的无界分配，不夸大实验范围。
- **建议**：远端内容默认拒绝本地资源；所有图片来源限制总字节、像素/解码资源，拒绝设备/FIFO等非普通文件，限制并发并支持取消。仅给HTTP加超时不足。

### F-03 · P2 · 切换终端目录会提交尚未确认的shell输入

- **位置**：[directory:15–25](<../../src/terminal/directory.rs#L15-L25>)、[session:574–599](<../../src/terminal/session.rs#L574-L599>)。
- **触发/原因**：用户在交互shell键入但未回车的`printf UNEXPECTED > marker; #`后点击切换目录。应用输入队列为空不等于readline行为空；注入cd末尾的CR把原输入一起提交。
- **影响**：自动执行用户尚未确认的命令，可能造成非预期副作用；不限于cd失败。不将用户自身输入误报为远端任意代码执行。
- **证据/复核**：w2真实Session/PTY实验输出`change_directory=Ok(())`、`partial_command_executed=true`；w1/w4/w5重跑确认。[复现源码](<w2-evidence/pty-prefix.rs>)、[日志](<w2-evidence/pty-prefix.log>)、[终端分项](<w2-terminal.md>)。
- **建议**：缺少shell协作且无法可靠证明空行时，不自动注入并提交；采用明确确认/告知，或可靠空行协议。不要以清空应用队列代替shell状态确认。

### F-04 · P2（低影响）· 父目录symlink切换不触发配置/主题刷新

- **位置**：[watch:40–53](<../../src/desktop/watch.rs#L40-L53>)、[77–79](<../../src/desktop/watch.rs#L77-L79>)。
- **触发/原因**：观察`current/palette.json`，current为目录symlink；原子更换current到新目录。inotify观察的是旧目录，symlink所在父目录未观察，不能唤醒refresh。
- **影响**：仍显示旧主题/设置，且不跟随新目录后续修改，直到其他有效事件/重启；无数据删除或外传。
- **证据/复核**：原观察器既有测试通过；新增隔离回归等待3秒通知失败，退出101；w5独立复制同实验确认，原对照0。[测试源](<w3-evidence/watch-tests.rs>)、[失败日志](<w3-evidence/watch-regression.log>)。
- **建议**：监视解析路径中symlink组件所在父目录并加入过滤目标，添加目录链接原子切换与后续修改用例。

### F-05 · P2（中低影响）· 开发入口跨文件系统卸载恢复失败

- **位置**：[install-dev:43–54](<../../scripts/install-dev.py#L43-L54>)，第51行`backup.replace(path)`；备份/入口目录分别来自73/74行不同XDG变量。
- **触发/原因**：已有桌面入口，state与data目录位于不同设备，安装后卸载；rename跨设备返回EXDEV。
- **影响**：卸载失败、原入口未恢复，但备份保留，**非不可逆数据丢失**。
- **证据/复核**：w6原函数在`/tmp`与`/dev/shm`临时目录复现errno18，w7独立重现；[一键缺陷断言](<evidence/w6/reproduce-exdev.py>)、[分项](<w6-nix-packaging.md>)。
- **建议**：复制备份到目标同目录临时文件后原子替换，保持symlink/元数据语义，成功后才清除备份和元数据；补跨设备回归。

## 4. 真实验证、工具结果与限制

### 全项目门禁

| 命令 | 退出码/结果 | 持久证据 |
|---|---|---|
| `nix develop --command cargo test --locked --workspace --features agents --lib --bins` | **0**；84 lib+183 agents+195 files+2 derive = **464 passed** | [完整Rust日志](<w3-evidence/cargo-workspace.log>) |
| `nix develop --command cargo test --locked --workspace --features gui-tests --lib --bins` | **0**；92+427+201+2 = **722 passed，2 ignored** | [完整GUI-feature日志](<w3-evidence/cargo-gui-workspace.log>) |
| `nix build --no-link .#checks.x86_64-linux.portal` | **0**，w7两次，第二次显式记录portal-check-exit=0 | [portal命令记录](<w7-portal-boundaries.md#命令与真实结果>) |
| Nix store环境`dbus-run-session ... python src/bin/files/portal/tests.py` | **0**，真实私有session D-Bus，**11 tests OK** | [portal记录](<w7-portal-boundaries.md>) |

GUI-feature测试不等于真实桌面测试，两轮Rust测试也不能相加当作独立覆盖总数。裸环境Python最初127，切换已有Nix环境后复现成功；不隐去环境失败，未安装依赖迎合检查。预期GTK fallback错误日志/第三方warning不等于测试失败。

### crapper / dryer / mutator 实际诊断

| 局部范围 | crapper/dryer（除媒体退出码未保留外均0；均无真实CRAP） | 有界实际变异/退出码 |
|---|---|---|
| shared derive/form/theme/watch | CC最高41；三对主题测试相似1.00/0.83/0.83 | watch选3项：**2 killed/1 survived，3** |
| agents协议 | execute CC47、watch_cached40、apply_event33；一对测试wrapper0.90 | tracking仅scan6 sites，**未执行** |
| agents配置/缓存/历史 | load CC32、update31；短领域动作相似1.00 | paths选11项：**9k/2s，3**；隔离7/7测试 |
| files传输删除保护 | 本项crapper/dryer原始记录未恢复，不填造 | line371选3项：**2k/1s，3** |
| terminal目录命令 | CC-only与dryer日志已归档 | 选4项：**4k/0s，0**，逐项核对断言失败非编译失败 |
| portal Python | picker_arguments CC18、selection_results12；无>=0.8候选 | 四行选16项：**11k/5s，3** |
| install-dev Python | main CC18；无>=0.8候选 | 选4项初始3k/1s（3），加隔离symlink测试后**4k/0s，0** |
| agents媒体 | CC-only48函数、无>=0.8候选 | 仅scan4 sites，**未执行** |
| Nix | 不支持；人工全部文件审查/求值 | 不支持，不伪造 |

原始命令/参数与日志链接分别在各分项中。`--no-coverage`用于有界选择，不代表位点真实covered；百分比仅局部选中变异kill ratio，不是行覆盖率。未运行全仓无限变异，未为指标改生产代码。

重要纠错：watch与transfer初次“全killed”可能被冷编译超时污染，采用保留子日志的最终2k/1s；portal首次16 survived因测试resolve穿透overlay导入未变异原件而无效，临时调整导入根后最终11k/5s。所有survivor均视为测试/诊断信号，**不直接等同缺陷**。复杂度高、重复度>=0.8亦不自动认定correctness问题。

## 5. 待验证风险（与确认问题分开）

1. [配置原子替换:101–122](<../../src/configuration.rs#L101-L122>)未同步父目录；突然断电的保存持久性及非协作写者检测到persist的TOCTOU未做故障注入。[持久化说明](<w5-persistence.md#待验证风险不是确认缺陷>)。
2. [缓存namespace:380–389](<../../src/bin/agents/connection/cache/store.rs#L380-L389>)按认证文件引用而非内容区分；同路径证书轮换是否应隔离旧缓存取决于授权契约，未证明越权。
3. [files设置保存:201–205](<../../src/bin/files/settings/storage.rs#L201-L205>)重新读expected但实例可能持有旧Preferences；多实例无关字段覆盖未动态重现、产品契约未确认。[目录内容:31–35](<../../src/bin/files/directory/content.rs#L31-L35>)限量读取前lstat→open特殊文件替换竞态也未触发。
4. [portal:130–133](<../../src/bin/files/portal.py#L130-L133>)信任本地picker的stdout总量、[148–170](<../../src/bin/files/portal.py#L148-L170>)handler内不验证sender：无限输出和真实跨应用越权均未证明，不能跳过上层portal/session bus边界推断漏洞。
5. catalog安装进度无界通道、remove无总deadline、icon解码像素/SVG资源上限、日志自由文本非结构redact，需恶意服务/图片/真实secret实验；daemon安装逻辑在外部bas，不能归因本仓已确认漏洞。源码锚点见[补审记录](<#2-覆盖矩阵与八人协作>)。
6. 点击file URL的平台打开行为与共享open_file防线一致性，尚无实机执行影响证明；不计入F-02或新增代码执行问题。

## 6. 残余盲区、供应链与最终保证边界

- Linux/NixOS为实际环境；无Windows/macOS实机、aarch64实机、NixOS VM登录、完整release打包、真实桌面/屏幕阅读器/IME与焦点遍历。Nix求值成功不等于这些平台构建/运行通过。
- 无真实行覆盖报告；CRAP<10未验证。重复度候选没有全量消除，按只审查要求保留。全仓测试不足以证明竞态/取消所有交错、负载极限或安全边界完备。
- 未实测掉电、长时间RPC流/背压、真实USB/polkit、远程文件系统、ACL/NSS异常、恶意同UID目录交换；第三方decoder/daemon/协议依赖不做完整内部审计。F-02仅辅助实验内存耗尽，未真实GUI OOM/外传。
- Cargo锁解析1049包：1044 registry条目均有checksum、3 git包均锁定完整rev（两个源），Nix提供固定output hash、nixpkgs固定rev+narHash。**固定不等于安全**。cargo-audit/cargo-deny不在PATH，未安装、未下载advisory DB；无依赖无已知漏洞/许可证完全兼容承诺。
- 文档/翻译/静态资源完成清单与关联检查而非逐行逐像素审计；复杂GUI和历史缓存测试部分为抽查，整个状态空间不可能由本轮穷尽。

## 7. 行动与交付索引

先同步目标/基线并加载ub-mode→自主认领/公开协商去重→读取实现/测试及信任链→隔离最小复现→独立复核→实际工具诊断并纠正误测→归档分项证据→总报告合并去重与边界说明。没有以报告代替修复，也没有进行行为变更。

分项：[files](<w1-files-operations.md>)、[terminal](<w2-terminal.md>)、[shared/derive/GUI](<w3-shared-ui.md>)、[协议](<ub-mode-agents-protocol.md>)、[持久化](<w5-persistence.md>)、[Nix/脚本](<w6-nix-packaging.md>)、[portal/设备](<w7-portal-boundaries.md>)、[媒体](<ub-mode-agents-media-w8.md>)。各分项包含实际命令、退出码、证据及额外限制。临时实验不作为唯一永久证据，关键源/日志均已存本目录。

**原审查建议后续顺序**：先为F-01/F-02补安全回归并修根因，再处理F-03/F-04/F-05；另开授权修复轮次，不在原审查中更改生产代码。结论是有限证据支持的审查结果，不是绝对无漏洞保证。

## 8. 用户授权修复轮次（goalRevision 1）

用户随后明确授权修复全部五项，原“只审查”约束由本轮目标取代；仍禁止合并用户当前分支、推送或部署。本轮仅在 Swarm 私有 accepted 分支集成。五项均先加真实安全回归、观察原根因失败，再最小修复并独立复核；不降低既有断言或把编译失败冒称安全回归红灯。

| 项目 | 修复状态及关键取舍 | 先失败后通过 / 独立复核 |
|---|---|---|
| F-01 | 已修复：逐项复制清单、描述符相对 no-follow 清理、非递归目录删除；变化保留源并报告部分成功 | 确定性新增/改写/替换回归退出101→16隔离测试/10真实bin测试通过；w7独立10/10 |
| F-02 | 已修复：所有会话拒绝本地路径/file URL，统一HTTP/data有界读取；显式本地附件保留，SVG禁用、GIF首帧 | 解析器接受/dev/zero退出101→10图片+4附件通过；w8独立执行最终测试并复核旁路测试修正 |
| F-03 | 已修复：不能证明空行则不注入cd/Enter；Files改为新标签页，保留原shell输入 | 真实PTY marker被执行退出101→12 session/6 GUI panel通过；w8独立21次安全探针通过 |
| F-04 | 已修复：祖先链接通知与缺失叶文件解析均更新watch | 首修与独立发现missing-leaf补修均先退出101；最终4项真实通知测试通过，w6同失败探针补修后通过 |
| F-05 | 已修复：EXDEV目标侧暂存复制后原子发布，失败保留备份 | 真实不同st_dev文件/链接2项EXDEV→8新回归+6历史测试通过；w6独立边界及完整CLI生命周期通过 |

详细修复提交、命令、证据日志、独立复核和工具限制集中在[修复摘要](<repair-summary.md>)。F-04独立复核发现的首修缺口未被忽略；追加失败回归和补修。F-02独立复核发现data旁路测试证据不充分后，以原始bytes重写探针并实际移除override确认失败。实际工具记录保留survivor及环境/编译错误历史，未为CRAP或重复率指标做无关重构。

**保证边界不扩张**：F-01最后stat→unlink仍非快照事务；F-02每消息缓存不是全进程内存预算且deadline不能抢占同步decoder；F-03没有shell协作故放弃原shell自动cd；F-04未证明所有平台/队列溢出；F-05未新增多进程锁或掉电事务。完整桌面实机、所有平台、真实coverage及CRAP<10仍未证明。最终整合树验证结果见修复摘要，不把第4节原审查测试数量冒充修复门禁。
