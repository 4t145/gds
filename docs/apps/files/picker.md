# 文件选择器模式

普通启动 `gds-files [DIRECTORY]` 仍是文件管理器。加 `--pick-file` 启动单文件选择器，确认后输出结果并退出，不调用默认应用打开文件。

```sh
gds-files --pick-file "$HOME/Documents" \
  --filter '文本文件:*.txt,*.md' \
  --filter '所有文件:*' \
  --read-only
```

使用项目的 Nix 开发环境：

```sh
nix develop --impure --expr 'import ./shell.nix' --command \
  cargo run --bin gds-files -- --pick-file --filter '图片:*.png,*.jpg,*.webp'
```

## 参数与界面

- `--pick-file`：启用单文件选择模式。
- `DIRECTORY`：可选初始目录，缺省为主目录。
- `--filter '[名称:]模式[,模式…]'`：可重复指定过滤组，模式匹配文件名；没有指定时显示所有文件。目录仍可浏览，名称搜索与类型过滤叠加。
- `--read-only`：默认勾选“只读打开”，界面仍可更改。
- `--`：后面的参数只解释为目录，供以 `-` 开头的目录名使用。

右下角终端入口替换为类型过滤、只读勾选、取消和确认。选择文件后确认；目录只能进入，不能作为结果。`Alt+A` 确认，`Alt+C` 取消；Enter / 双击文件走确认流程，目录继续导航。菜单、输入编辑和模态对话框优先处理其自身取消，不由选择器抢走。

“只读打开”只作为结果标记返回，由调用程序决定如何打开文件；选择器不修改文件权限，也不替调用程序强制只读。

## 输出协议

确认成功时，stdout 只有一行 JSON，并在退出前 flush：

```json
{"path":"/absolute/path/report.txt","read_only":true}
```

返回绝对路径，不通过 canonicalize 替换用户选中的符号链接路径。对于不能表示成 UTF-8 的路径，`path` 为 null，另返回可恢复的原生 `OsString` 编码。例如 Unix：

```json
{"path":null,"path_os":{"Unix":[47,116,109,112,47,255]},"read_only":false}
```

调用方应同时检查退出码：`0` 表示确认成功，`1` 表示取消或直接关窗；输出失败返回 `2`。参数或启动失败也以非零状态结束。非成功退出时，不应使用 stdout 中的内容。诊断写入 stderr。

取消或直接关窗不输出选择结果。`--help` / `--version` 是普通命令行查询，其 stdout 不属于选择结果协议。
