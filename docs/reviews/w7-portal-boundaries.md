# w7：Portal、picker 与设备/剪贴板边界审查

## 基线和范围

审查基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`；报告同步至 `2d95ffe6f0bceef177c553ca018060980d0afd6d`（仅既有审查文档增加）。加载 `/home/atlas/.agents/skills/ub-mode/SKILL.md`，本轮遵循验证优先、边界检查、日志原则，不执行技能中的推送建议（owner 禁止）。未修改生产代码/配置/测试，未安装依赖。

|范围|实际检查|限制|
|---|---|---|
|[portal.py](<../../src/bin/files/portal.py>) 与 [现有测试](<../../src/bin/files/portal/tests.py>)|逐行审查选项翻译、非 UTF-8 路径、GTK fallback、Close 取消、子进程终止；真实私有 session D-Bus 测试|未连接实际桌面 portal/GTK 或真实 GUI；外层服务调用者身份校验未端到端验证|
|[picker.rs](<../../src/bin/files/picker.rs>)|CLI 解析、filter 编译、stdout 输出前段检查|没有本 worker 的真实 GUI 操作或完整状态机遍历；Rust workspace 测试由 w3 运行|
|[file_clipboard.rs](<../../src/bin/files/file_clipboard.rs>)、[protocol.rs](<../../src/bin/files/file_clipboard/protocol.rs>)、[wayland.rs](<../../src/bin/files/file_clipboard/wayland.rs>)|绝对路径/NUL、远端 URL 拒绝、64 MiB 上限、身份比较和 cut 完成更新、读超时|未运行真实 Wayland compositor；fallback/cut GUI 缓存未逐行深审；协议不提供原子 compare-and-set，代码已明确该限制|
|[storage/service.rs](<../../src/bin/files/storage/service.rs>)、[storage/linux.rs](<../../src/bin/files/storage/linux.rs>)|设备身份、全盘操作互斥、快照时序、UDisks 方法超时、部分卸载错误|未对物理 USB/光驱、polkit 授权、拔插竞态做真实设备测试；非 Linux 路径仅静态查看 cfg|

本范围没有新增已确认生产缺陷。不能把这一结论解释为不存在漏洞。

## 命令与真实结果

1. `nix build --no-link .#checks.x86_64-linux.portal`：退出 **0**（两次，第二次显式输出 `portal-check-exit=0`）；不是新安装依赖。
2. 使用该 check 的 `nix derivation show` 所列已有 store Python/dbus 运行现有测试：

```sh
DBUS=/nix/store/f88s1f2rgbz6nnkdry1hy25q0kaidiz1-dbus-1.16.2
PY=/nix/store/sklx8y7j4hm9fllj59r86dl1w3bkbclp-python3-3.14.8-env/bin/python
"$DBUS/bin/dbus-run-session" --config-file="$DBUS/share/dbus-1/session.conf" -- "$PY" src/bin/files/portal/tests.py
```

退出 **0**，`Ran 11 tests in 0.229s / OK`。日志含预期 invalid-json/错误文件类型 fallback 异常，以及第三方 dbus-next 的 DeprecationWarning/ResourceWarning，不误报为测试失败，也未据此认定生产泄漏。

3. 隔离复制原 portal 与测试至 `/tmp/gds-w7-portal-YgnPk8`，运行：

```sh
crapper --root "$D" --no-coverage "$D/portal.py"
dryer --root "$D" --threshold 0.8 "$D/portal.py"
mutator --root "$D" --scan "$D/portal.py"
mutator --root "$D" --mutate-all --no-coverage --lines 66,73,82,140 --max-workers 2 \
  --test-command "$DBUS/bin/dbus-run-session --config-file=$DBUS/share/dbus-1/session.conf -- $PY portal/tests.py" "$D/portal.py"
```

`D=/tmp/gds-w7-portal-YgnPk8`。前三项退出 **0**。crapper 是 **complexity-only**：`picker_arguments CC18`、`selection_results CC12`、`dispatch/pick CC6`，Cov/CRAP 均 **N/A**，不能宣称达到 CRAP<10；真实 CRAP 目标 <10。dryer 阈值 **0.8**，单个 portal 文件没有候选重复，不代表全仓 duplication<0.8。扫描 51 个变异点，仅选计数/路径/过滤器边界和取消清理四行的 16 点运行。

首次 mutator 全部 16 survived（退出 **3**）不能作为有效指标：隔离测试中的 `Path(__file__).resolve()` 穿透 symlink overlay，实际导入未变异的原副本。发现原因后仅在临时副本把 import 根设为 `os.getcwd()`，重跑 `--mutate-all`；最终 **11 killed / 5 survived**（退出 **3**，68.8% 仅为本次16点 mutation kill ratio，不是覆盖率）。生产测试文件未改变。五个 survivor：66行 `or→and`（缺空 selections 反例）；82行 `or→and`、`0→1`、`<=→<`、`<→<=`（缺 filter index 类型/上下界反例）。这是测试充分性缺口，不是已确认生产缺陷。`--no-coverage` 意味着未采集真实行覆盖。

持久证据：[复杂度](<w7-evidence/portal-complexity.edn>)、[重复度](<w7-evidence/portal-duplication.edn>)、[selection 变异](<w7-evidence/portal-mutation.edn>)、[chooser 变异](<w7-evidence/chooser-mutation.edn>)。

## 独立复核（不重复计算缺陷数）

### w1：跨设备移动删除未复制的新子文件，确认，P1

直接阅读 [remove_source:368–379](<../../src/bin/files/operations/transfer.rs#L368-L379>) 和 [copy/commit/remove:415–423](<../../src/bin/files/operations/transfer.rs#L415-L423>)，其仅比较根 dev/ino 后 `remove_dir_all`，没有验证目录子项是否仍为复制快照。

在 w1 隔离 harness `/tmp/gds-w1-transfer-NbmBKc` 运行 `nix develop <本 worktree> --command cargo test --offline audit_reproduction -- --nocapture`，退出 **0**，1 test passed，输出 `CONFIRMED: late-important absent from both source and destination after successful removal`。该测试在原 copy_root 后、原 commit/remove 前插入另一个 writer 创建子文件，调度点严格对应生产顺序。触发：跨设备移动、源目录仍被其他程序写入；影响：新子文件在源/目标均不存在且操作成功，永久丢失。普通同设备 rename 不走此路径。最小修复方向：删除阶段仅删除已验证、已复制且未变化的条目，并在变化时保留源并明确报告；不能仅比较根目录 inode。详见 w1 报告及 finding `a7c7222f-ef2a-4201-88af-da9b92c1e36d`；公共复核消息 `dbe74b5e-c9d9-49cb-a880-b045bff9d138`。

### w6：安装器跨文件系统恢复失败，确认，P2

直接阅读 [restore:43–54](<../../scripts/install-dev.py#L43-L54>)。用原 `save_original/restore`，目标位于 `/tmp`、备份位于 `/dev/shm`，`nix develop --command python3` 输出：`different_devices True; restore_errno 18; original_restored False backup_retained True`，退出 **0**（捕获预期 OSError）。裸 `python3` 首次运行退出 **127**（host 未提供），切换项目已有 Nix 环境后成功复现。触发：XDG_STATE_HOME 与 launcher/desktop 目录不同文件系统；影响：卸载报 EXDEV、旧入口不恢复，备份保留，不是不可逆数据丢失。建议复制备份到目标同目录临时文件再原子替换，成功后移除备份。公共复核消息 `bc4661ea-3014-403d-ae52-720d4d604b4b`，详细复现由 w6 报告维护。

## 残余风险（未确认）

- [portal.py:130–133](<../../src/bin/files/portal.py#L130-L133>) 对受信本地 picker 的 stdout 使用无界 communicate；若 picker 故障无限输出，会有内存压力。没有复现真实 gds-files 产生此输出，不作为缺陷。
- [portal.py:148–170](<../../src/bin/files/portal.py#L148-L170>) 不在 handler 内校验请求 sender；正常应由 session bus/上层 portal 限定调用边界。未复现跨应用越权，不宣称权限绕过。
- 设备操作 read→method 间天然存在外部状态改变窗口；现代码有身份复查。缺少真实热插拔压力测试，不能宣称可安全处理所有竞态。
- 未验证 actual parent-window/modal focus、sandbox 文档授权、真实 GTK SaveFiles、GTK fallback 取消、SIGTERM 与创建子进程交错。

## 操作约束和清理

仅新增审查文档/证据；隔离临时目录保留用于复核。测试产生的两个本 worktree `__pycache__` 字节码已核对绝对路径后删除。没有 push、部署、用户分支合并；同步仅在本 worker 私有 worktree 接受已审查文档 head。无仍运行后台作业。
