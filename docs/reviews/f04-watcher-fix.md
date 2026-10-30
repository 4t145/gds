# F-04 修复记录

## 范围与设计

共享 Sources 服务设置与主题观察者；仅监听文件最近的现存父目录会让 inotify 跟随目录符号链接的旧目标，无法看到链接所在目录中的原子替换。修复目标是同时观察路径祖先目录，事件仍按目标路径/祖先关系过滤；收到事件后既有 Observer 先 refresh 再发送 Files 更新，跟随新目标。

不改变用户配置、写入文件或轮询间隔，不部署或推送。只验证 Linux 真实 notify 行为；其他平台仍依赖 notify 后端，无跨平台实机结论。

## 行动与验证

1. 阅读 ub-mode、生产 Sources、Linux 测试与 Observer 调用链。
2. 先加入真实临时目录回归：创建两个目标目录，原子替换父目录链接，要求收到通知；refresh 后再修改新目标文件，要求继续通知。
3. 在 Nix 开发环境运行回归，生产实现尚未改动。完整工具日志留于会话工具记录。
4. 诊断运行 `crapper --no-coverage --threshold 10 src/desktop/watch.rs`、`dryer --threshold 0.8 src/desktop/watch.rs src/desktop/watch/tests.rs`、`mutator --scan src/desktop/watch.rs`。基线 refresh CC12；Cov/CRAP N/A（无覆盖率，不能宣称 CRAP<10），无重复候选，6 mutation sites（扫描不是执行变异测试）。

5. 回归先失败：Nix `cargo test --locked --lib desktop::watch::tests::watches_parent_symlink_replacement_and_new_target_changes -- --exact --nocapture`，退出 101，3 秒首个通知超时（job `bash-7d85779b76b9470d`）。
6. 最小根因修改：目录收集从最近现存父目录改为全部现存祖先目录，保留目标过滤，不增加轮询或独立状态。
7. 同一 Nix 环境 `cargo test --locked --lib desktop::watch::tests -- --nocapture`，退出 0，新旧两项均通过（job `bash-58ef2c8e4e594d9e`）。首次 fmt-check 退出1，仅新增测试换行格式差异；按 rustfmt 修正后重跑。

取舍：额外的非递归目录 watch 数量按路径深度增加，并由 BTreeSet 去重，非递归监听不会扫描目录。祖先路径发生变化会触发保守刷新；其它无关路径事件仍被过滤。不能消除 inotify 队列溢出、观察器平台错误或瞬间多次切换的中间快照丢失；既有错误日志与后续事件刷新行为保持不变。

## 独立复核发现与补充修复

w6 复核指出：叶文件在新旧目标都不存在时，单纯 canonicalize(file) 失败，缓存只含相同词法路径，refresh 会提前返回而仍观察旧 inode。新增缺失文件回归先失败（Nix 退出101，job `bash-ae503945854344fe`）；独立实验也失败。补充 `resolved_target` 将最近可 canonicalize 的祖先加上剩余路径，即使叶文件不存在也能检测目标切换、过滤新目标事件。新旧全部3项测试通过（job `bash-65be5361b8d546d6`）。按复核建议，后续写入前增加150ms静默事件排空，避免迟到的链接通知造成假阳性。

独立复核、全仓测试与 GUI/portal gate 由协作记录及综合修复报告汇总。
