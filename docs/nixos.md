# 在 NixOS 安装和使用 GDS Files

仓库导出 `packages.<system>.gds-files`、`apps.<system>.default` 和
`nixosModules.gds-files`，支持 `x86_64-linux` / `aarch64-linux`。

默认文件夹打开方式通过 `inode/directory` MIME 关联配置；应用中的文件选择窗口
通过 XDG Desktop Portal 配置。这是两个独立入口，模块可以同时启用。

## 接入系统 flake

在你的 NixOS 配置仓库的 `flake.nix` 中加入输入和模块，保留已有的其它配置：

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # 改成此仓库在本机的绝对路径。
    gds.url = "git+file:///home/atlas/Github/4t145/gds";
  };

  outputs = { nixpkgs, gds, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        gds.nixosModules.gds-files
        {
          programs.gds-files = {
            enable = true;
            defaultFileManager = true;
            portal.enable = true;
            # 默认已包括 niri；通用 portals.conf 也会配置。
            portal.desktops = [ "niri" ];
          };
        }
      ];
    };
  };
}
```

本地 Git flake 只包含 Git 已跟踪的文件。首次接入这次变更时，先在 GDS 仓库执行
以下命令；不要求提交：

```sh
git add flake.nix nix src/bin/files/portal.py src/bin/files/portal \
  src/bin/files/picker.rs src/bin/files/picker src/bin/files/app/picker.rs
```

这些文件推送到上游后，也可以把输入改成 `github:4t145/gds`。

模块默认使用本仓库锁定的 nixpkgs 构建 Files，不必添加
`gds.inputs.nixpkgs.follows = "nixpkgs"`。这样系统使用稳定版 nixpkgs 时也能使用
本项目要求的较新 Rust 工具链。需要定制包时可设置 `programs.gds-files.package`。

在系统配置目录执行（把 `myhost` 换成你的配置名）：

```sh
sudo nixos-rebuild switch --flake .#myhost
```

首次构建会下载 Rust 依赖并编译 GPUI。应用需要正常的图形会话和 GPU 驱动；模块
不替换现有 Niri、DMS 或显卡配置。重建后建议注销并重新进入图形会话，让 D-Bus
激活环境、桌面入口和 Portal 配置同时生效。

本地源码变更后，需要在系统配置仓库更新输入锁再重建：

```sh
nix flake update gds
sudo nixos-rebuild switch --flake .#myhost
```

## 默认打开文件夹

包安装 `io.github.fourt145.GdsFiles.desktop`，桌面入口的 Exec 为
`gds-files -- %f`，传入一个本地路径。模块设置系统的 `inode/directory` 默认关联。

如果 Home Manager 或已有用户配置设置了别的文件管理器，用户设置会优先于系统。
使用 Home Manager 时，在用户配置里同步设置：

```nix
xdg.mimeApps = {
  enable = true;
  defaultApplications."inode/directory" = [ "io.github.fourt145.GdsFiles.desktop" ];
  associations.added."inode/directory" = [ "io.github.fourt145.GdsFiles.desktop" ];
};
```

未使用 Home Manager 时，可以在图形会话中执行一次：

```sh
xdg-mime default io.github.fourt145.GdsFiles.desktop inode/directory
xdg-mime query default inode/directory
xdg-open "$HOME"
```

查询应返回 `io.github.fourt145.GdsFiles.desktop`，打开操作应启动 Files。
模块只关联目录，不关联普通文档或整个 `file:` 协议。Files 尚未实现
`org.freedesktop.FileManager1`，因此应用中直接调用该接口的“在文件夹中显示”
可能仍会启动其它文件管理器。

## 系统文件选择器

选择器使用独立的 `io.github.fourt145.GdsFiles.Picker` 窗口标识。在 Niri 配置中加入
以下规则，即可默认居中悬浮；普通 Files 浏览窗口保持原有布局：

```kdl
window-rule {
    match app-id=r#"^io\.github\.fourt145\.GdsFiles\.Picker$"#
    open-floating true
}
```

包内也提供同样的 `share/gds-files/niri.kdl`，可以由 Niri 配置 `include` 引入。
规则采用 [Niri 的 open-floating 设置](https://niri-wm.github.io/niri/Configuration%3A-Window-Rules.html#open-floating)。

启用 `portal.enable` 后，模块会安装 `xdg-desktop-portal-gds` 和 GTK 后端，
注册 D-Bus 按需激活服务，并把 FileChooser 接口的首选后端设置为 `gds`。
GTK 回退直接调用 GTK 后端，不会再次经过 GDS，也不会产生递归。

| 请求 | 使用的界面 |
| --- | --- |
| 单个已有文件，无过滤器或可表示的 glob 过滤器 | GDS Files |
| 单个或多个文件夹（例如 Zed 的“打开文件夹”） | GDS Files |
| 文件多选（例如 Zed 的“打开文件”） | GDS Files |
| 保存文件、批量保存 | GTK |
| MIME 类型过滤、额外 choices、不能无损转换的过滤表达式 | GTK |
| Files 启动失败或返回无效结果 | GTK，并在日志记录错误 |
| 用户取消 Files 窗口 | 取消请求，不打开 GTK |

Portal 会传递初始目录，优先显示请求的 `current_filter`，将选中路径转换成
`file://` URI，返回实际选中的过滤器和读写选项。空格、中文和非 UTF-8 Unix
文件名会正确编码。调用方关闭请求时，对应的选择窗口进程也会结束。

目前 GDS 窗口尚未接入调用方的窗口标题、确认按钮文字和外部父窗口句柄，因此
不会与调用方建立真正的模态父子关系。GTK 回退完整转发原请求的参数。

只有通过 Portal 发起选择请求的应用会使用这个设置。应用自己绘制的选择框
仍由应用控制。Firefox 可以在 `about:config` 中把
`widget.use-xdg-desktop-portal.file-picker` 设置为 `1`，然后重启并测试。
其它应用需要按其自身的 Portal 支持配置。

Niri 通常已有自己的桌面专用 Portal 配置，模块同时设置 `common` 和 `niri` 的
FileChooser 条目；其它桌面把 `portal.desktops` 改成对应的小写标识，例如
`[ "sway" ]`。模块不覆盖截屏、录屏等接口。如果现有系统或 Home Manager 已经
显式指定 FileChooser 后端，该设置可能优先于模块默认值，需要在原配置中改成
`[ "gds" ]`。用户目录下的 `~/.config/xdg-desktop-portal/*portals.conf`
也可能覆盖系统配置。

手动验证 Files 自身的选择模式：

```sh
gds-files --pick-file "$HOME" --filter '文本:*.txt,*.md'
```

确认后 stdout 输出一行 JSON，例如：

```json
{"path":"/home/atlas/example.txt","read_only":false,"filter_index":0}
```

退出码：`0` 选中，`1` 取消或启动失败，`2` 参数或输出错误。非 UTF-8 路径的
`path` 为 `null`，另有 `path_os.Unix` 字节数组。此 CLI 协议由 Portal 适配器
转换，不能直接把 `gds-files --pick-file` 写成 Portal 后端配置。

## 排查与验证

```sh
systemctl --user status xdg-desktop-portal.service
systemctl --user status xdg-desktop-portal-gds.service
journalctl --user -b -u xdg-desktop-portal -u xdg-desktop-portal-gds
```

GDS 服务按需激活，首次请求之前显示 inactive 属于正常情况。图形环境变化后，
如果服务没有继承当前会话的 Wayland / D-Bus 环境，可在图形会话的终端执行：

```sh
dbus-update-activation-environment --systemd WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP
systemctl --user restart xdg-desktop-portal-gds.service xdg-desktop-portal.service
```

仓库内验证包和独立 D-Bus 协议测试（新增文件需先纳入 Git）：

```sh
nix build .#gds-files
./result/bin/gds-files --help
nix build .#xdg-desktop-portal-gds
nix flake check
```

不安装也可以在图形会话运行 `nix run . -- "$HOME"`。
构建结果包含所需动态库包装，不依赖 `nix develop` 的 `LD_LIBRARY_PATH`。
Nix 打包时会跳过一个需要设置 setuid / setgid 权限的元数据测试，因为 Linux
构建沙箱禁止这类操作；其它 Rust 测试及 Portal 的独立 D-Bus 测试仍会执行。

协议依据：[NixOS MIME 选项](https://github.com/NixOS/nixpkgs/blob/02c087f3fe673c9e317f206a627cb6b910336dbe/nixos/modules/config/xdg/mime.nix)、
[Portal 后端配置](https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html)、
[FileChooser 后端接口](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.FileChooser.html)、
[Firefox Portal 偏好](https://searchfox.org/mozilla-central/source/modules/libpref/init/StaticPrefList.yaml)。
