# F-03：终端目录切换修复

## 根因与修复

空应用输入队列、shell 进程组位于前台，均不能证明 shell 行编辑缓冲区为空。旧后端会把 `cd ...\r` 追加到尚未确认的输入，从而执行用户未按 Enter 的命令。

[PTY 后端](<../../src/terminal/backend/unix.rs>)现在拒绝自动目录命令；没有可信 shell integration 时，即使看似处于空提示符，也返回 `ShellBusy`，不发送命令、回车、清行或中断。保留 API 的路径校验、unsupported-shell 和关闭状态处理。

[Files 面板](<../../src/bin/files/app/terminal/panel.rs>)的目录动作改为在目标目录新建并选中终端标签页，保留原标签页、shell 和未确认输入。中英文标签明确说明“新终端标签页”。普通手工终端输入、手工 `cd`、前台作业和退出流程不变。不尝试通过 Ctrl-U 清掉用户输入，也不声称能在原 shell 安全自动切换。

## 先失败后通过

1. 先仅添加[真实 PTY 回归](<../../src/terminal/session/tests.rs>) `directory_change_does_not_submit_unconfirmed_shell_input`，生产代码保持基线。交互式 POSIX sh 输入 `printf EXECUTED > UNCONFIRMED_EXECUTED #`，无 Enter，等待回显且应用队列为空，然后调用目录切换。随后测试主动清行并通过 shell printf 往返作为顺序屏障。清行仅属于测试，不属于产品行为；使用行删除而非 Ctrl-C 信号，避免信号丢弃先前输入造成假通过。
2. `nix develop --command cargo test --locked --lib terminal::session::tests::directory_change_does_not_submit_unconfirmed_shell_input -- --exact --nocapture`：**退出101**，marker 已生成，准确失败在“目录切换提交了未确认输入”断言；不是编译失败或超时。诊断 job `bash-077b702f05a14ce6`。
3. 根因修复后，`nix develop --command cargo test --locked --lib terminal::session::tests -- --nocapture`：**退出0，12 passed**（包含隔离子进程的退出屏障测试）。诊断 job `bash-f4657fdb0823442b`。回归 marker 不生成；原交互测试保留手工 cd 和前台作业断言，改为断言自动请求拒绝。
4. GUI feature 面板测试改为验证新标签页的 cwd/选择状态和两个旧 shell 的身份、cwd 均保留。运行结果在候选提交前记录于公共 F-03 线程。

## 局部指标

`crapper --no-coverage` 对三个变更的生产文件退出0；后端目录处理 CC4、面板目录动作 CC3、Session 请求 CC6。覆盖率与 CRAP 均为 N/A，不据此声称 CRAP<10。`dryer --threshold 0.8` 同范围退出0，无候选重复；不等于全仓指标。诊断 job `bash` 对应本轮终端 complexity/duplication 工具输出。新增回归真实覆盖根因；基线重放本身构成恢复旧不安全逻辑时的失败证据，不冒称执行额外变异工具。

## 边界

实测为当前 NixOS/Linux 和 PATH 中 POSIX sh；不代表所有 shell/平台实测。回归覆盖已经排空的传输队列与未确认输入。没有新增 shell integration，故不支持原 shell 的自动目录切换。新 tab 会启动该目录的新 shell，而不是继承旧 shell 的环境变量或作业；这是明确的安全取舍。独立复核由 w8 执行并另行记录。未合并用户分支、未推送、未部署。
