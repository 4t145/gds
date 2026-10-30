# 授权修复摘要

本轮沿用既有审查，用户授权修复 F-01–F-05；仅在 Swarm 私有分支集成，**不合并用户当前分支、不推送、不部署**。原报告的审查阶段记录保持为历史证据，修复结果单独列出。

## F-01：只清理已复制且未观测到变化的源条目

- 集成：`363c1d02712147d7e7d89890b0db284f68836de9`；[实现与回归说明](<w1-files-operations.md#f-01-修复本轮>)。
- 用复制清单记录成功复制的条目及 dev/ino/mode/size/mtime/ctime；清理从目录描述符出发、逐级 no-follow 打开，只删除清单内核对通过的叶子，目录仅非递归删除。不再递归删除新出现、未复制的内容。
- **先红后绿**：确定性 copy→cleanup 屏障插入并发新增、原 inode 改写和替换 inode。旧实现错误报告成功，安全断言退出101；修复后16项隔离 production-module 测试、10项真实 gds-files transfer 测试通过。w7独立运行真实应用10项测试退出0（Finding `23f97a98-7a29-4d7b-b394-e38fdbd84093`）。
- 取舍：目标已提交而源发生变化时保留剩余源并报告不完整移动；更早已清理的已复制条目不会回滚。硬链接别名可能因首个 unlink 改变 ctime 而保守留下。metadata→unlinkat 最后窗口仍有 TOCTOU，不承诺任意恶意同UID写入、粗粒度时间戳/特殊FS下的原子跨设备移动；严格保证需要停止写者或文件系统快照/隔离。
- 工具实际结果：新 check CC7、remove_at CC12，Cov/CRAP N/A；dryer 阈值0.8无候选；3项新检查有界实际变异全部 killed、退出0，非全48位点或行覆盖率成绩。

## F-05：跨设备恢复开发入口

- 修复提交：`8b320d382412f38a02ea32d4fc3f7918d4efdc83`；集成：`2a1181c97c8a26e237cb1e7a66f05f44b06c39f3`。
- [实现](<../../scripts/install-dev.py>)仅捕获 `EXDEV`，在目标文件系统内建立私有临时目录，以 `copy2(..., follow_symlinks=False)` 复制备份，然后同文件系统原子替换；成功后才删除备份及元数据。普通同设备恢复仍使用原有 rename。
- **先红后绿**：[真实跨设备回归](<../../scripts/test_install_dev.py>)使用 `/dev/shm` 与 `/tmp` 并确认 `st_dev` 不同，原实现对普通文件和 dangling 相对 symlink 均报 errno 18。修复后8项测试全部通过，覆盖 mode/mtime、链接文本、原入口不存在、外部编辑保留、非EXDEV错误不降级、复制失败和发布失败保留备份且清理临时文件。
- 独立复核 w6：[原实现日志](<repair-evidence/gds-w6-baseline.log>)、[修复后日志](<repair-evidence/gds-w6-candidate.log>)；3项独立边界测试、完整 CLI install/reinstall/remove-all/re-remove 生命周期、8项新回归均通过，历史6项测试也通过。复核源：[边界实验](<repair-evidence/gds-w6-review-installer.py>)、[CLI实验](<repair-evidence/gds-w6-review-cli.py>)。
- 命令：Nix store Python 3.13.15 `-m unittest discover -s scripts -p test_install_dev.py -v`；w6 使用 Nix develop Python 3.14.8。裸 `python3` 不在 PATH，首次命令退出127，随后改用已存在解释器，未安装依赖掩盖失败。
- 取舍/边界：保持原 symlink 与元数据语义；不新增多进程 installer 锁、外部编辑 CAS 或 fsync/掉电事务保证。发布失败保留原入口和备份；发布完成后清理失败仍可能要求人工处理残留备份，不能宣称任意中断完全幂等。
- ub-mode 实际诊断：[crapper](<repair-evidence/gds-w5-crapper.log>)退出0，restore CC8、main CC18，Cov/CRAP N/A；[dryer](<repair-evidence/gds-w5-dryer.log>)阈值0.8无候选；[mutator](<repair-evidence/gds-w5-mutator.log>)仅选新根因分支与不跟随链接两处（55、63行），2 killed/0 survived，退出0。局部变异比例不是全仓覆盖率，未为指标重构无关 main。
