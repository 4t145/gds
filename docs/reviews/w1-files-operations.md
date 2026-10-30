# w1 文件操作审查（w6 接续证据落盘）

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。原审查者 w1，独立复核 w7；w1 会话不可用后，w6 通过 Task CAS 接续报告，重读生产删除/传输/回收站后端及临时复现，第三次运行确认结果。没有更改生产代码。GUI/设置/目录补审另由 w4 负责，不能把此后端复现当作所有文件操作穷尽覆盖。

## P1：跨设备移动目录会删除复制完成后新增的子文件

- 精确位置：[transfer.rs:368–379](../../src/bin/files/operations/transfer.rs#L368-L379)；生产调用顺序 [415–423](../../src/bin/files/operations/transfer.rs#L415-L423)。
- 触发：移动目录时初次原子 rename 返回 XDEV，进入 copy→commit→remove；在复制目录快照之后、删除源目录之前，另一个应用在源目录新增文件（或更改已复制文件内容）。根目录 inode/dev 不变。
- 原因：`remove_source` 只检查源根目录 dev/ino 是否与最初一致，随后 `remove_dir_all` 递归删除当前整棵源目录；没有逐项确认删除对象确实已复制成功且内容未变化。
- 用户影响：操作返回成功，未复制的新文件在目标不存在、在源也被删除，造成永久数据丢失。正常同设备 rename 不触发本路径；无并发写入时本复现不成立。
- 实验证据：w1 临时 crate 原样复制 names/transfer 模块和既有测试，在私有方法间确定性插入 `fs::write(source/late-important, ...)`，精准模拟生产415–423间的调度点。复现不是把生产实现替换成伪代码，也不需要恶意替换 inode。测试直接执行跨设备 fallback 的组成步骤，没有真实挂载跨设备；其路径可达性由生产405行 `Err(Errno::XDEV)` 证明。
- 结果：15项测试（14既有+1缺陷断言）全通过；缺陷测试输出 `late-important absent from both source and destination after successful removal`。这是断言缺陷存在，不是安全性回归通过。[w6再次运行日志](w1-evidence/reproduction.log)。
- 独立复核：w7 阅读生产与 harness 并在 Nix 环境运行1项复现，确认 P1，公共消息 `dbe74b5e-c9d9-49cb-a880-b045bff9d138`；w6 接续时第三次执行退出0。
- 最小方向：跨设备移动不能在拷贝后无条件递归清空源。至少只删除确认已复制且身份/内容未变化的具体对象，目录使用非递归删除，遇到新增/更改保留源并报告部分成功。mtime 或根 inode 再检查本身不足以消除 TOCTOU；要得到严格保证需明确并发写入协议或快照/隔离语义。应新增原路径确定性并发回归。

## F-01 修复（本轮）

- 用逐项复制清单替代源目录的 `remove_dir_all`。复制的文件/目录描述符与复制前后路径均检查 dev、ino、mode、size、mtime/ctime（含纳秒）；只记录成功复制且未观测到变化的条目。
- 清理从父目录描述符出发，逐级 `openat(NO_FOLLOW|DIRECTORY)`，只对清单中的已核对叶子调用 `unlinkat`；目录仅使用非递归 `rmdir`。新增条目、已替换/改写文件或目录身份变化会保留并报告不完整移动，已提交目的地不回滚。
- 回归在真实临时目录中执行生产 copy→cleanup 组成步骤，在明确屏障处分别新增文件、原 inode 改写、替换 inode。安全断言在修复前退出101（`changed source must report incomplete move`，job `bash-9b9767530a574af8`），证明旧代码误报成功。修复后该回归及既有测试通过；另补正常跨设备 fallback 的嵌套目录/悬空链接移动。初次完整 GUI 冷编译为缩小 red 反馈范围主动取消，不能当成测试通过。
- 竞争边界：这不是文件系统事务/快照或与外部写入者的锁定协议；最终 metadata 检查与 `unlinkat` 之间仍有极短 TOCTOU，恶意进程可在其中改写或替换叶子。时间戳粗粒度/非本地文件系统也可能削弱检测。新方案不再递归枚举并删除尚未复制的内容，但**不宣称任意并发写入下的原子跨设备移动**；要取得后者必须外部停止写入或采用快照/隔离。清理过程中错误可能已删除较早的已复制条目，因此部分成功和剩余源必须由调用方显示。硬链接别名因删除首个名字改变 ctime，可能保守地留下后续名字并报告部分成功，而不是冒险删除。
- 本轮实际验证：独立 harness 的 names/transfer 共16项通过（job `bash-f4183cd1759e4dd8`，退出0）；真实应用 `nix develop --command cargo test --locked --bin gds-files operations::transfer::tests -- --nocapture` 10项通过、187项过滤（job `bash-dd5805c72aa0415b`，退出0）。`crapper --no-coverage` 得到新 `check` CC7、`remove_at` CC12；**没有真实覆盖率/CRAP分数**。`dryer --threshold 0.8` 无重复候选。对 `check` 首个布尔连接/size比较及目录删除前身份检查三项有界变异，3 killed / 0 survived（job `bash-8838e97a039149e9`，退出0）；`--no-coverage` 只控制选位，不是覆盖率证明，也非48个位点全量变异成绩。
- 独立复核、最终整合树验证及命令结果见综合修复摘要；本节不将前轮审查结果冒充本轮最终门禁。

## 可重现方式（只写临时目录）

将 [harness manifest](w1-evidence/harness-Cargo.toml) 和 [lock](w1-evidence/harness-Cargo.lock) 复制为某临时目录的 Cargo.toml/Cargo.lock；在该目录创建 `src/lib.rs` 内容 `pub mod operations { pub mod names; pub mod transfer; }`。原样复制本仓库 `src/bin/files/operations/names.rs`、`transfer.rs` 及其 names/tests.rs、transfer/tests.rs 到对应模块位置。在临时 transfer.rs 末尾追加 [复现测试](w1-evidence/audit-test.rs)。运行 `nix develop --command cargo test --manifest-path <临时目录>/Cargo.toml --locked --offline -- --nocapture`。所有输入仅使用现有依赖缓存，无需安装新依赖。临时 crate 不导入 GUI、trash 与其他生产模块，覆盖范围仅 names/transfer。

## 有界变异与测试充分性

w1 在临时副本仅对 [transfer.rs:371](../../src/bin/files/operations/transfer.rs#L371) 的两个比较和 `||` 运行3项变异；`--no-coverage` 只用于选择位点，**不代表真实行覆盖率**。最终复核结果 **2 killed / 1 survived，退出3**：[原始子测试日志](w1-evidence/mutant-detail.log)、[机器记录](w1-evidence/Prepared-mutation.edn)。存活项 `||→&&` 表明现有测试不足以分辨“只有 dev 或只有 ino 改变”的分支；不直接构成生产缺陷。首次运行曾显示3 killed，后因可能冷编译超时且无子日志证据被纠正，不能引用为100% mutation score。复现新增测试只证明具体并发数据丢失，不掩饰这一测试盲点。

w1 的 crapper/dryer 子报告未恢复到本项证据中，因此本项不单独声称运行记录或阈值达标；综合报告应引用其他已归档实际运行，不凭空填补。CRAP<10、duplication<0.8 为审查诊断阈值，而非此处已证实的指标。全工作区 Rust/GUI 测试的独立日志见 [w3](w3-shared-ui.md)。

## 其他已读边界与残余风险

- [永久删除](../../src/bin/files/operations.rs#L30-L58) 对叶子 symlink 使用 remove_file，父路径 canonicalize；没有把静态 symlink 正常处理误报为跟随叶子删除目标。
- [回收站](../../src/bin/files/app/recycle/backend.rs#L198-L271) 在恢复/清理前校验 info/payload 身份；恢复占用目标由传输 no-replace 报错，先提交有效载荷再删除 trashinfo，部分失败有明确错误。不把这些已存在保护遗漏成缺陷。
- 仍未穷尽路径父目录并发替换、远程/特殊文件系统语义、断电、递归深树栈/性能、权限/NSS 异常及真实回收站多程序竞争。没有证据时这些保留为盲区，不另立确认问题。
