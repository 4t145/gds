# w4：agents 协议、权限与会话审查

## 基线与方法

- 审查基线：`2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。只新增本报告，不修改生产代码、配置或测试。
- 已加载全局 ub-mode：先证明行为、边界处验证、不把指标当缺陷。目标阈值 CRAP < 10、duplication < 0.8；本范围没有真实覆盖数据，不能声称满足 CRAP 阈值。
- 自主协调后取消与 w2 重叠的 terminal 任务；connection/cache、持久化由 w5 审查；catalog 后续补审亦交 w5。

## 覆盖矩阵

| 范围 | 已检查维度 | 检查方式与局限 |
|---|---|---|
| [connection](<../../src/bin/agents/connection.rs>) | 命令派发、取消/drop、重连、顺序、事件 session/cursor 边界 | 通读；请求有30秒期限；未模拟真实 daemon 网络故障 |
| [permissions](<../../src/bin/agents/connection/permissions.rs>)、[elicitations](<../../src/bin/agents/connection/elicitations.rs>)、[model](<../../src/bin/agents/connection/model.rs>) | 流订阅、错误可见性、session 检查、载荷解析 | 人工检查；读取 permission gRPC fixture 测试，未独立执行这些测试 |
| [tracking](<../../src/bin/agents/connection/tracking.rs>)、[foreground](<../../src/bin/agents/connection/foreground.rs>) | 提交后追踪、重新连接、前台 presence、终止 | 人工检查和 mutation-site scan；读取 tracking 正常/中断/重连/EOF/drop 测试 |
| [conversation](<../../src/bin/agents/conversation.rs>)、[protocol](<../../src/bin/agents/conversation/protocol.rs>)、[terminal presentation](<../../src/bin/agents/conversation/terminal.rs>) | 消息/工具/协议状态重建、终端 base64、输出保留上限 | 人工检查；这不是共享 PTY 进程审查 |
| [submission](<../../src/bin/agents/submission.rs>) | 不可变 turn/request、标题 preflight、重试边界 | 人工检查；没有发现已确认重复提交问题 |
| [transport](<../../src/bin/agents/settings/connections/transport.rs>)、[check](<../../src/bin/agents/settings/connections/check.rs>)、[certificates](<../../src/bin/agents/settings/connections/certificates.rs>)、[connections](<../../src/bin/agents/connections.rs>) | 平台 transport、mTLS 文件选择、超时/取消、异步 generation | 人工检查；未运行 macOS/Windows、真实 mTLS 环境 |

## 本范围结果

没有新增独立确认缺陷。检查到事件 session 与 cursor 校验、权限订阅 session 校验、失败状态与成功空快照区分、提交追踪 turn 匹配和 drop 时 task 清理等防线。以上只是源代码审查，不是无缺陷保证。复杂状态机、吞吐/背压以及攻击性 daemon 长时间行为仍需动态测试。

### 独立交叉复核 A：远端 markdown 图像可触达本地资源

此问题归入 w8 的媒体发现，不重复计数。

- [connection:1031–1058](<../../src/bin/agents/connection.rs#L1031-L1058>) 解码并验证 session/cursor，但允许正常 AgentMessageChunk 内容。
- [conversation:158–164](<../../src/bin/agents/conversation.rs#L158-L164>) 将 chunk 入 Agent 消息；[chat/content:65–71](<../../src/bin/agents/workspace/chat/content.rs#L65-L71>)、[114–131](<../../src/bin/agents/workspace/chat/content.rs#L114-L131>) 将内容送往 markdown 与 image cache，不分本地/远端后端。
- [images:244–248](<../../src/bin/agents/workspace/images.rs#L244-L248>) 无 backend 本地性判断；[372–375](<../../src/bin/agents/workspace/images.rs#L372-L375>) 将 file URI 转为本地路径。
- 结论：远端 agent/daemon 的 markdown `![](file:///dev/zero)` 在源码上可达图像加载器。w4 确认信任边界缺失与可达链，未独立运行 GUI/OOM。依赖层无限读取与受限子进程复现由 w8 提供，不声称图像内容已被外传。
- 最小建议：远端内容默认禁止本地资源；所有来源施加普通文件/字节/像素/下载限制，禁止设备/FIFO 等特殊文件。
- 公共复核记录：thread `b49b8c9a-829d-40f5-be93-d2bd8967f16d`，message `7f951510-546d-42f8-a7a8-58a56ad19ff7`。

### 独立交叉复核 B：切换目录提交未按回车的 shell 命令

此问题归入 w2 的 PTY 发现，不重复计数。

- [directory:15–25](<../../src/terminal/directory.rs#L15-L25>) 在 cd 命令末尾追加 CR；[session:574–599](<../../src/terminal/session.rs#L574-L599>) 只检查输入队列已排空及目录变更状态。队列空不代表交互式 shell/readline 缓冲为空。
- 阅读 w2 隔离 harness 后，w4 独立执行 `/tmp/gds-w2-audit/pty-prefix /run/current-system/sw/bin/bash`，退出码 **0**，输出：

```text
change_directory=Ok(())
partial_command_executed=true
```

- 触发条件：shell 存在未回车的 `printf UNEXPECTED > marker; #` 后用户点击切换目录。操作将该原输入一并提交；潜在影响是意外执行用户尚未确认的命令，而不仅是 cd 失败。
- 最小建议：无法可靠判断 readline 空行时不要自动注入并提交 shell 文本；需要显式确认并说明会提交当前行，或使用具备 shell 协作的安全空行协议。
- w4 未修改或重新编译 peer harness；这是对同一真实 Session 可执行实验的独立再运行，不是第二份独立实现。确认 finding `3b01d32c-d339-4cfb-af21-daed4c455678`，公共复核 message `9765bf95-faac-4c89-aa56-ea99853796dc`。

## 指标与命令日志

诊断输出目录 `/tmp/gds-w4-protocol-6HfPaU`，均为隔离临时目录；不依赖该目录永久保留，关键结果如下。

1. `pwd; git status --short; git ls-files; ...`：退出 **0**，确认独立 worktree 与 clean baseline，发现全局技能。
2. `command -v crapper dryer mutator cargo nix; ...`：退出 **0**，工具已存在，没有安装依赖。
3. `crapper --help; dryer --help; mutator --help`：退出 **0**，确认支持 Rust 与扫描/覆盖限制。
4. `crapper --root /tmp/gds-w4-protocol-6HfPaU --no-coverage <connection.rs permissions.rs tracking.rs conversation.rs protocol.rs submission.rs>`：退出 **0**。最高 CC：execute **47**、connection::watch_cached **40**、Conversation::apply_event **33**、watch_permissions **17**、apply_protocol_update **17**、track **15**。工具输出所有 coverage 和 CRAP 均 **N/A**；不得把这些 CC 值称为 CRAP。
5. `dryer --root /tmp/gds-w4-protocol-6HfPaU --threshold 0.8 <以上文件另加 elicitations.rs>`：退出 **0**。唯一候选相似度 **0.90**：[connection:863–867](<../../src/bin/agents/connection.rs#L863-L867>) 与 [permissions:5–14](<../../src/bin/agents/connection/permissions.rs#L5-L14>)。均为 `#[cfg(test)]` wrapper，未作为功能缺陷，亦未机械重构。
6. 首次 `mutator --root <临时目录> --scan <工作树tracking绝对路径>`：退出 **0** 但明确输出 outside-root skipped、No source files。该次不能算 mutation 检查成功。
7. 复制 tracking 原文件进入临时目录后 `mutator --root /tmp/gds-w4-protocol-6HfPaU --scan tracking.rs`：退出 **0**，确认6个 mutation sites：line21 `0→1`、27 删除`!`、40 `!=→==`、43 `||→&&`、43 `<=→<`、57 删除`!`。仅扫描，**未执行 mutant、不提供 kill rate 或覆盖率**；高风险有界实际变异由其他 peer 的 files、PTY、cache 隔离实验记录。
8. `/tmp/gds-w2-audit/pty-prefix /run/current-system/sw/bin/bash`：退出 **0**，见复核B。

## 残余盲区

- 本 worker 没有独立执行 Rust workspace suite；w3 另有 Nix shell 全工作区验收证据，汇总时应引用其真实日志，不能当作本 worker 的覆盖率。
- 没有实测 gRPC 流洪泛、全局背压/内存极限或网络代理；没有进行此范围实际 mutation execution。
- TLS/Target/ACP 解码部分来自外部 bas 依赖。生产安全不能仅由本仓库 wrapper 推定；本审查未尝试外部 daemon 授权绕过或宣称其安全。
- 没有运行 macOS/Windows GUI、远端真实 daemon、mTLS 双端证书轮换与长时间压力实验。
