# F-02：会话图片信任边界修复

## 根因与决策

消息中的图片地址不等于用户授权读取客户端文件。旧实现把绝对路径、相对路径和 file URL 交给 GPUI 通用读取器，后者先无界读取再识别图片。本修复将**所有会话来源**的本地路径、file URL、Resource::Path/Embedded 拒绝，不以本地 agent 连接冒充用户授权。用户主动选择/粘贴的附件仍走独立附件入口；普通文件 symlink 仍可使用，设备/FIFO 不可使用。

[图片缓存](<../../src/bin/agents/workspace/images.rs>)只允许 HTTP(S) 与内嵌 data 图片。[TextView](<../../src/bin/agents/workspace/markdown.rs>)使用已有 image_source override，让 Markdown/HTML/data 图片全部经过同一缓存，移除原先绕过缓存的内嵌图片所有权代码。[附件入口](<../../src/bin/agents/attachment.rs>)在 Unix 用 O_NONBLOCK/O_NOCTTY 打开后重新检查实际 handle 的普通文件类型与长度，避免初始 stat 后换成 FIFO 导致 open 阻塞。

## 资源边界与行为变化

[有界加载器](<../../src/bin/agents/workspace/images/loading.rs>)：

- HTTP 不信任 Content-Length，最多读取 8 MiB + 1 哨兵字节，超限拒绝；请求/等待正文的异步任务有 30 秒期限，移除引用取消任务。
- data URI 编码文本最多 32 MiB，解码后最多 8 MiB。
- 栅格解码最大 4096×4096，image decoder max_alloc 为 64 MiB；只解码静态首帧，再规范化为 PNG，避免动画帧无界累计。单缓存最多 16 个加载/完成条目。
- SVG 不再自动渲染；其外部资源与解码复杂度不纳入此安全边界。图片尺寸/容量超限、格式不支持时显示现有 fallback，新增中英文安全提示。
- 本地 Markdown 图片即使来自本地 agent 也不自动预览；请明确添加附件。正常 PNG/JPEG 等栅格附件保留。GIF 仅显示首帧。

这些限制不是整个进程的全局内存预算，很多独立消息仍可累计占用。30 秒异步期限不能抢占正在同步执行的 decoder；没有声称 CPU 隔离或真实 GUI OOM 压测。HTTP 可访问 loopback/private 网络的原有行为未改，本项不是 SSRF 修复。非 Unix 平台未动态验证；设备/FIFO 回归在 Linux/NixOS 上运行。

## 先失败后通过与验证

1. 先仅加入 `conversation_images_reject_client_files_and_special_files`，运行 Nix `cargo test --locked --features gui-tests --bin gds-agents conversation_images_reject_client_files_and_special_files -- --nocapture`。**exit 101**：第一项 `/dev/zero` 被原解析器接受（日志 `/tmp/gds-w2-f02-red.log`，job `bash-99e6ad5d09b14290`）。未对真实设备执行无界读取。
2. 根因修复后第一轮 5/6 通过；统一 HTTP/data map 后取消测试仍断言旧 inline map 应为空，改为断言只保留原 stable URI，未削弱取消效果。第二轮图片 **9/9**、附件 **4/4** 通过（exit 0，`/tmp/gds-w2-f02-green2.log`，job `bash-f3edd0369f364c73`）。
3. 回归覆盖真实 FIFO、symlink→设备、普通附件 symlink、合法本地 PNG Markdown 拒绝、无 Content-Length 超限 HTTP、data 字节/尺寸边界、ACP/Markdown 正常 PNG、fallback、引用变更取消和重复引用保留。
4. w8 独立阅读候选并运行 9+4 测试通过，发现原 data bypass 测试误用规范化 PNG asset key 的证据缺口；现改用原始 PNG bytes，手动移除 override 后测试因 native asset 被缓存而 **exit 101**，恢复 override 后重跑。日志见[旁路变异](<f02-evidence/bypass-mutant.log>)。新增缓存16条预算测试最初误在 GPUI 绘制阶段外调用 load，触发框架约束（9/10、exit101），已改为真实17张 Markdown 图片渲染。最终图片 **10/10**、附件 **4/4** 全通过，exit0（job `bash-29e51892eb614168`），见[最终测试](<f02-evidence/final.log>)。独立结论以 run Findings 为准，不把作者自测称作独立复核。

Nix shell 启动打印 `<nixpkgs>` 搜索路径警告并回退当前 bash；锁定 shell.nix 环境继续成功完成 Rust 构建/测试。不是将该警告视为测试失败，也未掩盖它。

## ub-mode

对本次生产范围运行 crapper --no-coverage（exit 0）：新加载器 CC≤8、resolve 从旧11降到4，附件 load 为19（包含原有内容分类逻辑）。无覆盖率采集，CRAP/Cov 为 N/A，**不能宣称达到 CRAP<10**。dryer --threshold 0.8（exit 0）无重复候选。`mutator --no-coverage --lines 22,28 --max-workers 1` 对 HTTP 状态判断、MAX+1 哨兵的三处目标变异全部 killed（exit 0）；unknown-length 测试用有效 PNG 补齐为超限正文，避免无效字节本身掩盖哨兵错误。共发现5站点，仅执行3处，不声称全范围100%。[复杂度](<f02-evidence/crapper.log>)、[重复检查](<f02-evidence/dryer.log>)、[目标变异](<f02-evidence/mutation.log>)与原始 EDN 均归档；这不是全仓变异覆盖率声明。
