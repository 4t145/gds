# w5：Agents 持久化、设置与历史缓存审查

## 基线、方法与覆盖

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。已读取全局 ub-mode；本轮优先验证行为和边界，不为复杂度/重复指标机械修改代码，且用户审查限定优先于技能的推送建议。未安装依赖、未改生产代码/配置/既有测试。

人工逐行阅读：[原子文件写入](<../../src/configuration.rs>)、[Agents 配置存储](<../../src/bin/agents/configuration/store.rs>)及其测试、[连接配置](<../../src/bin/agents/configuration/connections.rs>)、[终端偏好初始化](<../../src/bin/agents/configuration/terminal.rs>)、[缓存接口](<../../src/bin/agents/connection/cache.rs>)、[缓存 SQL/分页/命名空间/淘汰](<../../src/bin/agents/connection/cache/store.rs>)、[缓存路径校验](<../../src/bin/agents/connection/cache/store/paths.rs>)、[缓存测试前250行](<../../src/bin/agents/connection/cache/tests.rs#L1-L250>)、[ACP 页面](<../../src/bin/agents/settings/acp.rs>)与[持久化](<../../src/bin/agents/settings/acp/persistence.rs>)、[设置窗口关闭处理](<../../src/bin/agents/settings/window.rs#L225-L258>)、[历史浏览器](<../../src/bin/agents/history.rs>)、[主题加载](<../../src/bin/agents/theme/loader.rs>)、[主题偏好解析](<../../src/bin/agents/theme/preferences.rs>)。

| 维度 | 检查结论/边界 |
| --- | --- |
| 配置数据完整性 | section 级冲突拒绝，保留顶层未知字段；临时文件同步后原子替换，拒绝符号链接和只读祖先 |
| 凭据/缓存隔离 | namespace hash 包含 target 枚举 Debug 和 TLS 引用，未读取/存储密钥内容；私有目录/文件权限与硬链接检查 |
| 异步/并发 | 有界缓存队列、专用线程串行执行；SQL transaction 获取写预约；ACP debounce/flush 与单个 revision 写入链 |
| 历史/GUI | generation 过滤过期响应；历史发现并发上限2，删除确认回调再次核验 current/active |
| 性能 | 缓存总序列化字节限额512MiB及LRU；复杂函数需真实覆盖后再判断维护风险 |
| 兼容/测试 | 隔离实验仅 Linux/Unix；完整 Toasty/GPUI/daemon 集成、macOS/Windows、掉电持久化未在本分项验证 |

## 确认发现

本分项没有达到“触发、影响、可复现证据均充分”的新增确认缺陷。不能将这一结果解释为无漏洞或全范围已测通过。

## 待验证风险（不是确认缺陷）

1. [原子替换后没有同步父目录](<../../src/configuration.rs#L101-L122>)：临时文件 `sync_all` 不等同 rename 目录项掉电持久化。在突然断电场景，已提示保存成功的配置是否会回退依赖文件系统/挂载语义；未开展真实断电或故障注入，不能声称已复现数据丢失。若产品需要 crash-durable save，应在 persist 后同步父目录并加故障测试。
2. [外部编辑检测与 persist 之间](<../../src/configuration.rs#L105-L120>)存在非协作写者的 TOCTOU 窗口。`.lock` 只约束遵守同一锁协议的写者；未做可控竞争复现，不将其提升为确认缺陷。说明并发保证范围，或使用平台适合的强一致存储接口。
3. [缓存 namespace](<../../src/bin/agents/connection/cache/store.rs#L380-L389>)只含认证文件引用而非证书内容；同路径身份轮换是否应隔离旧缓存，取决于 daemon session 授权与缓存展示契约，本轮无足够证据证明越权。

## 实际诊断与有界验证

所有命令都在私有 worktree 或隔离临时目录执行，未全仓无限变异。`command -v` 确认 crapper/dryer/mutator/cargo/nix 已存在；`rustc --version` 为1.95.0，退出0。三个工具 `--help` 实际执行并阅读，退出0。

- `crapper --no-coverage src/bin/agents/configuration src/bin/agents/connection/cache src/bin/agents/settings/acp src/bin/agents/history.rs src/bin/agents/theme`，退出 **0**。最坏 CC：缓存 load32、history update31、缓存 open25/store24、配置 save19。全部 Cov/CRAP **N/A**；阈值目标 **CRAP<10** 未验证，不能把 CC 当 CRAP。[输出](<w5-evidence/crapper.log>)。
- `dryer --threshold 0.8` 加上述相同路径，退出 **0**。只报 [select_agent](<../../src/bin/agents/history.rs#L176-L182>) / [select_workspace](<../../src/bin/agents/history.rs#L184-L190>) score1.00；这是两个不同领域动作的短结构相似，不是行为缺陷，不要求机械合并。目标 duplication<0.8 本局部并未全满足。[输出](<w5-evidence/dryer.log>)。
- `mutator --scan src/bin/agents/connection/cache/store/paths.rs`，退出 **0**，发现18 sites。[输出](<w5-evidence/mutator-scan.log>)。
- 隔离目录复制未经改动的 [paths 源码](<w5-evidence/paths.rs>)，使用 [harness](<w5-evidence/harness.rs>) 编译测试。harness 仅替代外围 Error 类型及 Linux `O_NOFOLLOW=0x20000` 常量，不替代被测逻辑；无项目依赖。`rustc --edition 2024 --test harness.rs -o test-paths && ./test-paths` 退出 **0**，**7/7** 通过：新建私有目录/数据库、公开父目录拒绝、祖先符号链接拒绝、数据库符号链接拒绝、公开数据库/硬链接拒绝、公开 sidecar 拒绝、已有内容不截断。
- 同隔离目录 `mutator --root . --no-coverage --lines 17,34,59,65 --max-workers 1 --test-command 'rustc --edition 2024 --test harness.rs -o test-paths && ./test-paths' paths.rs`，退出 **3**。实际执行 **11** mutants，**9 killed / 2 survived**；18为扫描总量，不是执行数。81.8%仅为选中变异杀伤比例；`--no-coverage` 把站点视为 covered，并未测真实覆盖率。[变异数据](<w5-evidence/paths-mutation.edn>)。

两个 survivor 都是 `|| -> &&`（17、59行）：本组 symlink 用例仍受目录创建/open 的后续拒绝保护，因此通过不证明漏洞，也不能机械推断测试遗漏。实验没有对其他源文件大范围变异。

复现实验：把本分项的 harness 和 paths 复制到新临时目录，再执行上述 rustc/mutator 命令。测试会在 `/tmp/gds-w5-path-test-*` 创建独立小目录，避免触碰用户数据。该隔离实验不是项目完整测试，也不能替代真实 SQLite、GUI 生命周期、RPC 集成测试。完整 workspace Cargo 测试与 portal Nix 检查由其他 peer 单独记录，不在此声称通过。

## 行动日志与残余盲区

1. 同步目标/任务、确认基线与跟踪树、读取 ub-mode；自主认领此范围。
2. 阅读上述实现/测试，追踪 settings section 合并、路径防护、缓存窗口与写入串行化。
3. 公共讨论协调 w3 避免 connection/cache 重复范围（w3 转 shared UI/derive）；无生产修改。
4. 运行复杂度、重复扫描；创建隔离 harness，以真实行为断言验证高风险文件路径拒绝与内容保留；执行11个有界变异。
5. 记录未复现风险，不宣称完整 CRAP/coverage 或全项目测试通过；输出本中文证据分项。

未实测：掉电恢复、恶意同UID目录交换竞态、Windows ACL、macOS目录权限、SQLite首次多进程建库、完整 ACP quit RPC超时/服务不可达、真实图形窗口关闭/重开。缓存单测其余部分仅通过工具扫描并非逐行人工阅读；这些均属于残余盲区。
