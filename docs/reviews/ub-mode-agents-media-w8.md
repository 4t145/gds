# w8 媒体边界审查（w4 接续归档）

## 来源与范围

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。原审查者 w8，自主认领附件、HTTP、图像、链接、音频、剪贴板及 chat resource；w8 因 activation_failed 离线后，w4 在 task revision 5→6 接续，仅归档报告/证据，不修改生产代码。

w8 已加载 ub-mode 并报告上述范围静态审查及工具运行；w4 独立复核图像协议可达性与依赖读取行为，没有声称重新逐行审查 w8 全部范围。其他媒体范围没有新增确认缺陷，不构成无缺陷保证。w3 独立确认本地性边界缺失；w4 协议复核另见[协议分项](<ub-mode-agents-protocol.md>)。

## 已确认：P1 / 高风险，远端 markdown 图像自动加载无界本地资源

### 位置与条件

- [images:244–248](<../../src/bin/agents/workspace/images.rs#L244-L248>) 捕获会话目录但不传递后端本地性。
- [images:372–375](<../../src/bin/agents/workspace/images.rs#L372-L375>) 将 `file:` URI 转为 Resource::Path；[379–398](<../../src/bin/agents/workspace/images.rs#L379-L398>) 也接受绝对路径/相对路径。
- 锁定依赖 `gpui-pre 0.3.9` 的 `src/elements/img.rs:634`：`Resource::Path(uri) => fs::read(uri.as_ref())?`，读取前没有普通文件类型或长度限制；同文件644行对HTTP正文 `read_to_end` 也没有长度上限。这里是依赖定位，不是本仓库生产源码行号。
- 远端 AgentMessageChunk 经 [conversation:158–164](<../../src/bin/agents/conversation.rs#L158-L164>) 和 [chat/content:114–131](<../../src/bin/agents/workspace/chat/content.rs#L114-L131>) 自动参与 markdown/image 渲染。攻击者须能控制已连接 daemon/agent 的输出或其转述的 markdown；不是任意未连接网络客户端均可触发。

示例载荷：`![](file:///dev/zero)`。图像识别发生在读取之后，因而内容不是有效图像不阻止先行读取。相比之下，[links:125–132](<../../src/bin/agents/workspace/links.rs#L125-L132>) 明确向链接上下文传递后端本地性判断。

### 影响与证据边界

已确认：远端 markdown 可导致客户端尝试无界读取本地特殊文件，耗尽进程可用内存/阻塞图像加载，构成可用性风险。**没有实测整 GUI 崩溃，没有证明本机文件内容被外传，不把 `fs::read` 返回错误等同于进程 abort。** P1 基于远端内容自动触发、不需点击和无界资源耗用；实际OS/allocator/内存限制下表现可能是可恢复的读取错误、内存压力或进程被系统终止。

w8 的隔离复现程序被保留于临时目录；w4 阅读后独立再运行相同操作（不是整 GUI 集成实验）：

```sh
(ulimit -v 131072; timeout 5s /tmp/gds-w8-metrics.9E1FHJ/unbounded_read /dev/zero)
```

结果退出 **0**，输出 `read error: out of memory`。程序将 `std::fs::read` 的错误打印后正常退出，故退出0不代表未耗尽限制。限制只作用于子进程，不触碰真实GUI，不启动服务。可重建的8行源文件见[复现程序](<w8-evidence/unbounded_read.rs>)；编译与重现命令见[复现日志](<w8-evidence/reproduction.log>)。

### 独立复核与最小建议

- w4：协议事件 session/cursor 检查不阻止合法消息包含 file URI，Agent chunk→markdown→图像缓存链可达；随后独立阅读锁定依赖634/644行并复跑受限读取实验。
- w3：独立确认 content_images 未传 is_local、resolve 后委托 ImageCacheItem；公共复核 thread `b49b8c9a-829d-40f5-be93-d2bd8967f16d`，messages `7f951510-546d-42f8-a7a8-58a56ad19ff7`（w4）、`e554cf39-02b6-480b-8607-abcea275a768`（w3）。
- 最小修复方向：远端内容默认禁止本机路径、设备与FIFO；打开后检查实际文件类型，并对读取字节数、下载、解码像素/动画帧等施加强制上限。单纯URI scheme校验、8KB metadata样本或HTTP超时不足以限制无界文件读取。

## 待验证风险（不计为新增确认缺陷）

1. 远端HTTP图像自动访问 loopback/private 地址的边界：源代码允许http/https，但未实际启动受控服务证明敏感响应读取、网络副作用或外传。
2. 本地 markdown 文件链接使用 `cx.open_url(file:)`，而非共享受保护 launcher；是否能执行 .desktop/其他可执行文件依赖桌面handler与用户点击，未验证，不能报成确认代码执行。
3. 巨型HTTP正文/解码炸弹：依赖无界 body/read 有源码证据，本轮未跑巨型下载或完整解码内存压力。

## ub-mode 工具与执行限制

目标阈值：CRAP <10、duplication <0.8。w8 临时指标仍在 `/tmp/gds-w8-metrics.9E1FHJ/.metrics`；原始完整命令与 shell exit 记录未存入公共消息，故以下如实区分可读取结果与 peer 自述，不补造退出码。

- crapper：w8 报告 complexity-only 48个函数，coverage/CRAP N/A。w4 读取[保留指标](<w8-evidence/crap.edn>)确认 `coverage nil / crap nil`；最高 CC：audio worker run24、Attachment::load16、links::Context::target15、images::resolve11。**不是CRAP分数或真实覆盖率**。
- dryer：w8 报告阈值0.8无候选；w4读取[保留结果](<w8-evidence/dry.edn>)为 `{:candidates []}`。原执行退出码未保留。
- mutator：w8 自述scan发现4站点；无保留scan stdout，**未执行媒体mutant，不提供kill rate或覆盖率**。全仓高风险有界变异证据由其他分项提供。
- w4 独立复跑受限读实验 exit0；随后在本工作树重新编译相同8行程序并重跑，结果亦 exit0，见保留日志。
- 归档时全局 `/tmp` glob 遇到 systemd-private 权限拒绝，未提升权限或绕过；改为读取已知媒体临时目录。没有安装依赖、修改配置或生产源码。

## 残余盲区

真实macOS/Windows剪贴板、Wayland compositor、音频硬件与故障清理未动态验证；媒体端到端GUI/OOM和所有来源组合未运行；无实际媒体变异执行。w8离线使完整原始命令日志不全，本归档只保留仍可核验的数据，不将口头覆盖声称替换为动态证据。
