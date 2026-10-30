# w2 终端/PTY 审查（w5 接续证据落盘）

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。原调查与实验：w2；独立复核：w1、w4，接续复核和文档：w5。w2 运行期间因 activation_failed 离线，任务自动释放；w5 在 Task CAS revision6 接续，仅落审查报告，不改生产代码或既有测试。

## 确认问题 TERM-01（P2）：切换终端目录会执行尚未回车的输入

**位置**：[后端“空闲”判定](<../../src/terminal/backend/unix.rs#L320-L340>)、[自动附加回车](<../../src/terminal/directory.rs#L15-L26>)、[Session 请求入口](<../../src/terminal/session.rs#L574-L599>)。

**触发**：交互式 shell 中已键入命令但尚未按 Enter，字节已从应用输入队列进入 PTY/readline；用户调用显式 change_directory 动作。此时 shell 仍在前台，`input_bytes==0` 也不代表 shell 编辑缓冲区为空，后端允许追加带 CR 的 `cd`。

**用户影响**：此前尚未确认的命令被执行；新 `cd` 可被原有注释吞掉。潜在影响取决于用户未提交命令内容，可包括破坏性文件操作。该发现需已有本地未提交输入，不是未经认证远端代码执行，也不是路径引号转义问题。

**复现证据**：[原始 harness](<w2-evidence/pty-prefix.rs>) 直接引入真实 backend/session/directory 模块，向 PTY 写入不含回车的 `printf UNEXPECTED > marker; #`，确认 marker 尚不存在，调用 change_directory 后 marker 出现，主动 terminate 并 wait_stopped。原 harness 中绝对路径指向 w2 当时的 worktree；迁移复现时应改为当前基线源码路径并使用相同项目已构建依赖。无恶意数据或用户文件参与，marker 位于临时目录并清理。

w2 finding `3b01d32c-d339-4cfb-af21-daed4c455678` 记录原始复现成功。w1 公共消息 `b5199cab-116b-41ca-9d52-9535eea86ba7`、w4 公共消息 `9765bf95-faac-4c89-aa56-ea99853796dc` 均独立执行通过。w5 再次执行 `/tmp/gds-w2-audit/pty-prefix /run/current-system/sw/bin/bash`，**退出0**，输出 `change_directory=Ok(())` 与 `partial_command_executed=true`；[原输出](<w2-evidence/pty-prefix.log>)。

**最小修复建议**：没有可信 shell integration 的“空 prompt”状态时，仅填充/复制目录命令并让用户自行确认，不自动 Enter；若必须自动执行，应先取得明确空编辑缓冲区信号。不能以静默 Ctrl-U 丢弃用户正在编辑的内容作为替代。

## 覆盖、测试与工具

w2 自主范围为 shared terminal/PTY、agents authentication 和 local_terminal。其诊断扫描覆盖 [terminal 模块](<../../src/terminal/>)与 [authentication 模块](<../../src/bin/agents/authentication/>)，不等于每行人工阅读或每条路径动态执行。接续 w5 已逐行复核上述三个关键函数与原 harness；未冒充重做 w2 全部静态工作。

- w2 在 Nix shell 运行 `cargo test --locked --lib terminal::directory::tests`，[保留日志](<w2-evidence/cargo-terminal.log>)末尾显示 **2 passed / 82 filtered**。原任务由 w2 启动，w5 未取得其后台 job 的最终退出码，不补造退出码。全仓真实退出0验证另见 [w3 报告](<w3-shared-ui.md>)。
- w5 接续实际执行 `crapper --no-coverage src/terminal src/bin/agents/authentication`，退出 **0**；[输出](<w2-evidence/crapper.log>)最大 CC 为 key_bytes62、grid prepaint45、backend io_loop25。Cov/CRAP 全部 **N/A**。目标 **CRAP<10** 不因 complexity-only 结果而得到证明。
- `dryer --threshold 0.8 src/terminal src/bin/agents/authentication`，退出 **0**，[输出](<w2-evidence/dryer.log>)无候选重复；只针对这两个局部目录，不是全仓 duplication<0.8 证明。
- `mutator --scan src/terminal/directory.rs`，退出 **0**，[输出](<w2-evidence/mutator-scan.log>)。w2 另在隔离副本以两项原始目录命令单测执行4个局部 mutants，[原日志](<w2-evidence/mutator.log>)显示4 killed；[每项失败明细](<w2-evidence/mutant-0.log>)及同目录其余 mutant日志证实测试断言失败而非编译失败。保存 [harness](<w2-evidence/mutation-harness.rs>)、[check脚本](<w2-evidence/check.sh>)与 [变异数据](<w2-evidence/directory-mutation.edn>)。
- 原 check 脚本引用 w2 缓存 rlib 和 Nix rustc-wrapper 绝对路径，适合保留审计轨迹而非跨主机直接执行；迁移需改为本地已构建依赖路径。w5 未重跑这4个变异，不声称接续独立变异验证；没有真实 coverage，4/4不是覆盖率。更不证明 TERM-01 已有测试，因为这些测试只验证路径合法性/转义，不模拟已有 readline 输入。

## 边界和残余盲区

认证子进程、helper 握手、组清理、进程取消等范围在 w2 扫描中可见；现有全仓单测见 w3 日志。不应将其理解为 w5 独立真实认证或所有竞态验证通过。未确认第二项终端缺陷。

未验证：非 Unix/Windows PTY、macOS 行为、所有 shell 编辑模式、shell integration 协议、真实外部认证终端及用户登录流程、孤儿孙进程/极端进程组重用压力。复现只对当前 Linux+bash 成立；测试通过与局部指标不足以承诺终端无漏洞。

行动日志：读取 w2 finding/公共历史与原实验文件→对照真实三处源码和已有失败明细→在本机重新执行原 PTY 二进制→重新执行三项诊断并记录退出码→将证据仅保留于 reviews 文档树。无生产修复、推送或部署。
