# w6：Nix、开发安装脚本与依赖边界审查

## 基线与方法

基线 `2cb3e9f29f5a64eedc371fafa25fd07309b9be4f`。人工通读全部 8 个 Nix 子目录文件，以及 [flake](../../flake.nix)、[锁定输入](../../flake.lock)、[shell](../../shell.nix)、[justfile](../../justfile)、[安装脚本](../../scripts/install-dev.py)、[manifest](../../Cargo.toml)；解析 [Cargo.lock](../../Cargo.lock) 的来源和完整性字段。覆盖打包/模块默认行为、运行时依赖、开发命令参数、备份与恢复、平台声明和供应链可重复性。Nix 不在 crapper/dryer/mutator 支持语言内，因此 Nix 仅人工审查和求值，不声称具备 Nix 覆盖率。

遵循全局 ub-mode：实际加载技能、先重现再定性、隔离实验、不改生产代码。阈值 CRAP<10、duplication<0.8；本项无真实行覆盖率，CRAP 明确 N/A，不将复杂度或重复候选当缺陷。

## 确认发现（P2，优先级低于不可逆数据丢失）

### DEV-EXDEV：不同文件系统上的原桌面入口无法卸载恢复

- 位置：[安装脚本:43–54](../../scripts/install-dev.py#L43-L54)，具体失败为第51行 `backup.replace(path)`；备份目录来自第73行 XDG_STATE_HOME，桌面目录来自第74行 XDG_DATA_HOME，因此二者不必同设备。
- 触发：用户已有桌面入口，安装开发入口后执行 `just uninstall-dev files`；XDG_STATE_HOME 与 XDG_DATA_HOME 在不同文件系统（例如 state 指向单独挂载目录）。
- 用户影响：恢复抛 `OSError: [Errno 18] Invalid cross-device link`，卸载退出失败，仍保留开发入口而非原桌面入口。原备份保留，因此**不是不可逆数据丢失**。launcher 与 state 跨设备、且存在先前受管 launcher 时也适用。
- 复现：[脚本](evidence/w6/reproduce-exdev.py) 使用原始 `save_original`、`write`、`restore`，仅操作 `/tmp` 与 `/dev/shm` 临时目录。在仓库根目录运行：`nix develop --impure --expr 'import ./shell.nix' --command python3 docs/reviews/evidence/w6/reproduce-exdev.py`。基线输出 `restore_errno 18 / original_restored False / backup_retained True`，退出0（脚本断言缺陷存在，非表示恢复成功）。
- 独立复核：w7 在公共线程 `86030771-794a-4f0e-8bfe-8d11568aee14` 消息 `bc4661ea-3014-403d-ae52-720d4d604b4b` 使用原函数、不同设备再次得到 errno18，确认备份仍在。
- 最小建议：把备份复制到目标目录内的临时文件（保留普通文件元数据与符号链接语义），原子替换目标，完成后再删除备份及元数据；增加跨设备回归。不能直接以非原子复制覆盖目标而引入新的中断损坏窗口。

## 实际命令、结果和工具证据

| 命令/实验 | 退出码 | 结果/解释 |
|---|---:|---|
| 裸 `python3` 首次尝试 | 127 | 外层 PATH 无 Python；随后用项目 Nix shell 成功，未安装依赖 |
| `nix eval --json .#packages.x86_64-linux.gds-files.meta.platforms` | 0 | Linux 平台元数据可求值；不等同所有平台编译通过 |
| `nix eval --json .#packages.aarch64-linux.gds-agents.name` | 0 | `gds-agents-0.1.0`，不等同 aarch64 实机构建 |
| `nix eval --json .#checks.x86_64-linux.portal.name` | 0 | `gds-portal-tests`；实际 portal build 由 w7 独立执行并报告0 |
| Nix shell Python 跨设备复现 | 0 | 确认 EXDEV，见上文 |
| `python3 -m unittest -v`（隔离拷贝） | 0 | 初始5项测试通过；加入符号链接边界测试后6项通过 |
| `crapper --root . --no-coverage install_dev.py` | 0 | main CC18、restore CC6、save_original CC4，其余CC1；Cov/CRAP 均 N/A |
| `dryer --root . --threshold 0.8 install_dev.py` | 0 | 单文件内无重复候选；不意味着全仓无重复 |
| `mutator --root . --no-coverage --lines 34,38,45,47,50 --max-workers 1 --test-command 'python3 -m unittest -q' install_dev.py` | 3 | 仅4个实际选中变异，3 killed/1 survived；发现隔离测试缺少外部符号链接保护断言 |
| 同上加 `--mutate-all`，补一项隔离测试 | 0 | 4 killed/0 survived。不是35个全部变异已执行，更不是生产测试覆盖率 |
| Nix shell `tomllib` 解析 Cargo.lock | 0 | 1049包：1044 registry、3 git、2 workspace；registry checksum 缺失0 |

诊断在 `/tmp/gds-w6-installer-audit` 内原样复制安装脚本运行；[初始日志](evidence/w6/baseline.log)、[最终测试日志](evidence/w6/baseline-final.log)、[复杂度](evidence/w6/crapper.log)、[重复度](evidence/w6/dryer.log)、[首轮变异](evidence/w6/mutator.log)、[复测变异](evidence/w6/mutator-final.log)、[隔离测试](evidence/w6/test_installer.py) 已保留。重跑测试应把生产脚本复制为临时目录 `install_dev.py`，再复制该测试到同一目录，避免修改项目测试树。

## 供应链事实、排除项与盲区

- Git 依赖 bas-client、gds-uim、gds-uim-macros 均锁定完整 commit；两个源分别对应 manifest 固定 rev，Nix 为 bas-client/gds-uim 提供固定 output hash。nixpkgs 由固定 rev+narHash 锁定。未观察到本仓解锁的 Git 分支依赖或下载后无校验直接执行的打包脚本。
- 包装器明确提供 xdg-utils/xdg-terminal-exec、terminfo、图形动态库；agents 额外 ALSA。module 是显式 enable，portal 也是显式 opt-in；默认文件管理器 MIME 采用 mkDefault，说明用户 MIME 设置优先。未因此确认权限提升问题。
- `cargo-audit`/`cargo-deny` 不在 PATH，未下载 advisory DB，未安装工具或改配置。因此**没有作出依赖无已知漏洞或许可证完全兼容的结论**；pin/checksum 只提供来源可重复性，不证明依赖本身安全。
- 未进行 NixOS VM 启动、真实桌面 DBus activation、完整两应用 Nix release 构建、aarch64 交叉编译、非 Linux 实机验证。flake 仅导出 x86_64-linux/aarch64-linux；广泛 `meta.platforms = linux` 不是其他平台已验证承诺。
- 安装脚本没有自带测试；新增测试只在审查证据临时目录。没有模拟断电/fsync 持久性、多进程并发安装、恶意同 UID 修改备份，不能将这些未测场景升格为确认缺陷。
