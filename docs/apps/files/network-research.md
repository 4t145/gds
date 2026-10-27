# Files 网络功能调研

调研日期：2026-10-27。目标环境：NixOS、Wayland、Niri。范围为访问远程文件、连接管理及桌面应用互操作。

远程访问应独立于桌面环境，在 GNOME、KDE、Niri 等环境使用相同的协议后端。选型同时要求有可核实的大型独立项目采用记录；按这一条件，建议优先验证 rclone 的独立进程方案，remotefs 保留为候选，GIO/GVfs 保留为可选的桌面集成路线。协议先覆盖 SFTP、SMB，并兼容已有网络挂载。

产品范围应同时覆盖连接、认证、浏览、传输和外部打开；仅列出挂载目录不足以构成网络功能。自动发现可以分阶段接入，但地址直连应始终可用。采用 GIO 本身也不要求在 KDE 下再实现 KIO；本项目优先评估独立后端，是为了减少对桌面服务的运行依赖。

以下对标依据上游文档及源码，未进行这些应用的现场 GUI 对比或服务器兼容性测试。功能建议与待验证项分别列出，不作为 Files 已实现能力。NixOS 依赖按仓库锁定的 nixpkgs `02c087f3fe673c9e317f206a627cb6b910336dbe` 核对，所含 GVfs 为 1.60.3。

## 经典文件管理器的实现

| 文件管理器 | 已核实的网络功能 | 对 Files 的参考价值 |
| --- | --- | --- |
| [GNOME Files / Nautilus](https://help.gnome.org/gnome-help/nautilus-connect.html) | 地址连接；局域网服务发现；最近服务器；SSH、SMB、FTP、WebDAV、NFS 等协议入口。当前上游网络视图按已连接、以前连接过、当前网络可用分组。[视图源码](https://github.com/GNOME/nautilus/blob/55ba360f0d71a3f05ec54b9f743d91b5424b0743/src/nautilus-network-view.c) | 网络页同时表达连接状态、历史和发现结果；协议操作复用 GIO/GVfs。 |
| [Dolphin](https://linux.kde.org/docs/network-shares/) | 地址直连；SMB 网络浏览、服务器共享列表及认证；添加到 Places；[KNetAttach](https://docs.kde.org/trunk_kf6/en/plasma-desktop/knetattach/using-knetattach.html) 提供 SSH、SMB、FTP、WebDAV 连接向导和最近连接。 | 高级用户可输入 URI，普通用户可填写连接表单；保存入口与当前连接分开。 |
| [Thunar](https://docs.xfce.org/xfce/thunar/unix-filesystem) | 通过 GVfs 访问远程文件系统，官方给出 SFTP 地址示例；按后端能力显示 Browse Network；侧栏支持网络入口和书签。[侧栏源码](https://github.com/xfce-mirror/thunar/blob/b1d2264ccae761e6bd1384fb992c4ea3d4564c44/thunar/thunar-shortcuts-model.c) | 非 GNOME 桌面也能使用 GVfs；后端可用性应直接影响入口。 |
| [macOS Finder](https://support.apple.com/en-sg/guide/mac-help/mchlp1140/mac) | 浏览网络电脑；按地址连接；访客或注册用户认证；连接后选择共享；最近服务器、收藏及侧栏入口。 | 服务器与共享目录是两个层级；身份选择和常用连接管理值得借鉴。 |
| [Windows File Explorer](https://support.microsoft.com/en-us/windows/experience/connectivity-networking/file-sharing-over-a-network-in-windows) | 网络发现；网络目录访问；映射网络驱动器；可选择登录时重新连接；另有向别人共享本机文件夹的管理入口。 | 用户需要稳定的常用入口；“访问共享”和“发布共享”应分别定义范围。 |

GNOME 帮助页仍用 Other Locations 描述入口，当前上游源码已有独立网络视图；产品设计应参考功能与状态模型，具体菜单位置需在目标发行版实测。表中的协议也依赖发行版提供的后端，不代表每个安装都具有相同能力。

这些实现共同指向五类需求：找到服务器、建立并认证连接、记住常用位置、操作文件、交给其它应用使用。复制移动的进度与失败处理同样影响网络体验，不能只比较协议数量。

## Files 的功能范围建议

| 能力 | 第一阶段交付范围 | 随后扩展 |
| --- | --- | --- |
| 协议 | SFTP、SMB；继续展示已有 NFS、CIFS、SSHFS 等挂载目录 | WebDAV / HTTPS；FTP、直接 NFS URI 按需求验证 |
| 连接入口 | `Ctrl+L` 输入 URI；网络页提供连接表单，字段包括协议、服务器、用户、端口和共享或路径 | 地址建议、SSH 配置中的主机候选 |
| 认证 | 用户名和密码；SMB 域和访客选项；SFTP 密钥、agent、主机身份问题的后端交互 | 更复杂的企业认证场景 |
| 保存连接 | 自定义名称、URI、最近连接、收藏；按需重连 | 用户显式选择的登录后自动连接 |
| 连接状态 | 连接中、等待认证、已连接、失败；可取消、重试、断开 | 更细的网络变化恢复策略 |
| 浏览 | 列表、隐藏项、标签页、前进后退、按位置语义返回上级；加载可取消 | 按能力提供目录监视、缩略图与远程搜索 |
| 文件操作 | 新建目录、重命名、复制、移动、删除；本地与远程互传；目录递归；进度、取消及部分失败报告 | 经验证的远程间传输优化、断点续传 |
| 应用互操作 | 已验证的 URI 打开或本地访问桥接；不能直接打开时提供下载入口 | 无本地桥接时的临时文件编辑及回传 |
| 自动发现 | 为发现结果保留模型和展示位置；地址直连独立可用 | 分别评估 DNS-SD、WS-Discovery 接入；GVfs 可作为可选发现来源 |

第一阶段指面向用户的完整交付范围，开发时先验证认证和只读浏览。自动发现完成之前，界面应区分“尚未支持发现”与“没有发现服务器”。密码认证之外的 SSH 配置和密钥场景须通过验证后再列入支持说明。

发布本机 SMB 共享涉及 Samba 服务和访问权限管理；在线账号、同步盘及离线同步也有各自的生命周期。这些能力可以单独立项，不与本次远程文件访问绑定。

### 网络页与连接管理

网络页建议保留一个“连接服务器”入口，并区分已连接、最近连接、发现的服务。常用位置继续使用现有收藏机制，避免在网络页和收藏页各维护一套互不关联的书签。

地址直连与表单连接应生成同一种位置对象。SMB 既能打开 `smb://server/share/path`，也应能先打开 `smb://server/` 选择共享。显示名可以修改；复制位置和持久化使用完整 URI，并排除密码。用户名、端口和共享是位置身份的一部分，不能只按主机名合并。

关闭标签页只关闭视图。断开作用于所选后端的连接或挂载，需要检查 Files 自己的活动任务；使用 GVfs 等共享会话后端时，还可能影响其它应用。移除最近记录、取消收藏和断开连接各有独立含义。

### 认证与凭据

独立后端路线需要 Files 自己组织认证界面、主机身份确认和凭据保存，调用协议库的认证与验证接口。若选择 GIO，其 `MountOperation` 提供用户名、密码、域、匿名身份、密码保存策略，以及 `ask-password`、`ask-question`、`aborted`、`show-processes` 等交互，可桥接到 GPUI 对话框。网络认证不应复用设备挂载的 polkit 授权框。[MountOperation 文档](https://docs.gtk.org/gio/class.MountOperation.html)

GVfs 1.60.3 的 SFTP 后端启动系统 SSH 客户端，并处理密码、密钥口令、主机身份问题和 SSH agent 环境；其凭据保存代码通过 libsecret 访问 Secret Service。这些行为可作为独立后端的功能对照，持久位置配置只保存连接信息。[SFTP 源码](https://github.com/GNOME/gvfs/blob/1.60.3/daemon/gvfsbackendsftp.c)、[凭据保存源码](https://github.com/GNOME/gvfs/blob/1.60.3/daemon/gvfskeyring.c)

验证时重点覆盖 `~/.ssh/config` 主机别名、`IdentityFile`、自定义端口、agent、首次主机密钥确认和密钥变更。它们依赖实际 SSH 配置、后端行为和会话环境，不能只凭“使用 OpenSSH”就承诺全部兼容。永久保存密码仅在可用的密钥服务和解锁流程验证通过后提供。

## 后端选择与能力边界

| 方案 | 能解决的问题 | 对本项目的代价与结论 |
| --- | --- | --- |
| remotefs | 统一的 Rust 远程文件接口，协议客户端独立提供；包含 SFTP、SMB、WebDAV 等实现 | 接口契合，但大型独立采用证据不足，保留为候选。 |
| rclone | 多协议存储访问及传输任务，提供 JSON 控制 API | 有 TrueNAS Cloud Sync 等独立采用记录，优先验证；需确认其存储操作模型是否满足文件管理器要求。 |
| GIO/GVfs | 已有 SFTP、SMB、DAV 等后端，提供文件接口、挂载认证、卷监视及 FUSE 桥接 | 需要 GLib 事件循环和 GVfs 会话服务，作为可选桌面集成路线。 |
| KIO | KDE 的资源访问体系；Dolphin 使用其网络能力；KIO FUSE 为不了解 KIO 的应用提供本地访问 | 接入 GPUI/Rust 需要额外处理 C++/Qt 边界及 KDE 依赖。适合作为行为参考。 |
| 自行组合 SFTP、SMB 等协议库 | 可以直接控制每种协议 | 应用需分别负责认证、连接生命周期、元数据、发现和跨协议操作，维护量较大。 |
| 只依赖已挂载路径 | 可以复用大量现有本地浏览逻辑 | 仍需要其它组件连接和认证；难以表达稳定 URI、共享列表和后端能力。适合作为已有挂载的兼容入口。 |

GVfs 官方把 FUSE 定位为对非 GIO 应用的有限访问支持；KIO FUSE 也明确提供在非 KIO 应用打开远程 URL 时的桥接。这支持 Files 将 URI 作为远程位置身份、把本地路径作为可选互操作能力的选择。[GVfs 架构说明](https://github.com/GNOME/gvfs/blob/1.60.3/README.md)、[KIO 介绍](https://api.kde.org/kio-index.html)、[KIO FUSE 用法](https://github.com/KDE/kio-fuse/blob/master/README.md)

### 独立于桌面的远程后端

remotefs 提供同步和异步接口、目录与元数据操作、流及能力报告。它定义公共契约，实际客户端由各协议包实现；使用它可以复用已有统一层，而不必从零组织每个协议。[项目说明](https://github.com/remotefs-rs/remotefs-rs)

当前 SSH 客户端提供 libssh2、libssh、russh 后端，SMB 客户端提供 libsmbclient、Rust 实现和 Windows WNet 等选择。Rust 实现提供了减少系统库依赖的选项，但并非默认客户端组合；须显式选择依赖特性，并验证 SSH 配置与身份校验、SMB 共享枚举、元数据、取消和禁止覆盖行为。[SSH 后端](https://github.com/remotefs-rs/remotefs-rs-ssh)、[SMB 后端](https://github.com/remotefs-rs/remotefs-rs-smb)

rclone 的 JSON 控制 API 提供目录列举、文件复制移动、创建删除、异步任务、统计和任务停止；其后端覆盖 SFTP、SMB、WebDAV 等服务。可以让 Files 管理一个私有控制端点上的 rclone 进程，统一承接远程访问，无需按桌面选择 GIO 或 KIO。SMB、SFTP 各自仍有协议限制，不能把接口统一等同于操作语义完全相同。[控制 API](https://rclone.org/rc/)、[存储能力列表](https://rclone.org/overview/)、[SMB 后端](https://rclone.org/smb/)、[SFTP 后端](https://rclone.org/sftp/)

两条路线均应先验证 SFTP 和 SMB 的实际工作流。认证交互、连接记录、任务界面仍属于 Files；自动发现和为外部应用提供本地路径是另两项能力，不由多协议读写接口自动解决。

### 采用记录与选型依据

remotefs 有真实应用使用：termscp 的开发文档说明该库由其文件传输模块演化而来，当前 Cargo 依赖也直接使用 remotefs 及其协议包。termscp 在 2026-10-27 查询时约有 3,100 个 GitHub stars；但两者的主要维护者均为 veeso，这属于同一作者生态内的应用验证，尚不足以提供大型独立项目的背书。[演化说明](https://docs.termscp.rs/en-US/developer/developer.html)、[实际依赖](https://github.com/veeso/termscp/blob/main/Cargo.toml)、[termscp 项目](https://github.com/veeso/termscp)、[remotefs 项目](https://github.com/remotefs-rs/remotefs-rs)

这轮对公开项目说明、依赖记录及下游的核查，未找到满足大型独立项目标准的 remotefs 采用证据。它不能因此被判定为不可靠，但现有证据不足以把它列为优先基础依赖；API 与 Rust 技术栈契合只解决了集成形式的问题。

rclone 的独立采用记录更明确：TrueNAS 将其用于 Cloud Sync，官方文档介绍相关传输及加密功能，当前中间件源码也直接调用 rclone 执行同步。这能支持对其真实存储工作负载应用经验的判断；它并不证明所有协议后端和本项目的交互式文件操作契约都已得到验证。[TrueNAS 文档](https://www.truenas.com/docs/scale/dataprotection/cloudsynctasks/managingcloudsynctasks/)、[TrueNAS 实现](https://github.com/truenas/middleware/blob/master/src/middlewared/middlewared/plugins/cloud_sync/sync.py)

### GIO 提供基础操作 Files 编排工作流

`GFile.copy` 支持进度和取消，但不递归复制目录；`GFile.delete` 删除目录时只接受空目录。递归遍历、多项任务、冲突命名、目录创建顺序、取消后的残留及最终报告，仍由 Files 组织。[复制接口](https://docs.gtk.org/gio/method.File.copy.html)、[文件接口](https://docs.gtk.org/gio/iface.File.html)

现有本地传输使用暂存和 Linux 的禁止覆盖重命名语义。远程后端是否能提供相同的提交保证，必须分别验证；预先检查“目标不存在”不能代替提交时禁止覆盖。尤其要验证同名竞争、写入流关闭失败、移动中的源删除失败，以及取消时已完成与未完成项目的区分。若后端不能满足产品当前的禁止覆盖契约，应限制相应操作或明确调整设计，不能声称与本地等价。

目录监视也可能不受支持；访问权限属性、Unix 权限、回收能力和可用空间都按返回信息展示。未知能力须保留未知状态，执行失败仍作为最终依据。[文件属性](https://docs.gtk.org/gio/file-attributes.html)、[目录监视接口](https://docs.gtk.org/gio/method.File.monitor_directory.html)

### 发现服务器与连接服务器

GVfs 的 network 后端聚合 SMB 浏览、DNS-SD 和 WSDD 结果，因此有条件复用 `network:///` 作为发现来源。发现出的目标 URI 应用于后续导航，不能把服务条目当成本地目录。[网络后端源码](https://github.com/GNOME/gvfs/blob/1.60.3/daemon/gvfsbackendnetwork.c)

这些发现方式有独立的服务与网络条件。GVfs 的 WSDD 实现先连接系统或用户 socket，必要时尝试启动 `wsdd`。安装了 SMB 文件访问后端，并不意味着 WS-Discovery 已经可用；DNS-SD 也需要核实 Avahi 运行情况。[WSDD 服务接入源码](https://github.com/GNOME/gvfs/blob/1.60.3/daemon/gvfswsddservice.c)

### 外部应用打开远程文件

建议按三个路径处理：能接收 URI 的应用使用 URI；有可用本地桥接路径的应用使用该路径；两者都不满足时提供下载。远程临时副本的编辑回传需要另行设计冲突检查和生命周期。

`GFile.get_path` 可能返回空，Files 不能为每个远程对象假定存在可供编辑器或终端使用的本地路径。KDE 的使用文档也专门区分了网络感知文件选择器与要求本地挂载的应用。[本地路径接口](https://docs.gtk.org/gio/method.File.get_path.html)、[KDE 应用兼容说明](https://linux.kde.org/docs/network-shares/)

Files 的 Portal 选择器目前返回本地文件路径。网络浏览接入后，选择器仍需单独处理调用方是否接受非本地 URI；不能直接向只接受本地文件的调用方返回 `sftp://`。

## GIO 路线的 NixOS 与 Niri 依赖

仓库的 NixOS 模块目前默认启用 UDisks2，尚未配置 GVfs。锁定版本的 `services.gvfs` 模块负责注册 D-Bus 和 systemd 服务、启用 FUSE，并设置 `GIO_EXTRA_MODULES`。[NixOS GVfs 模块](https://github.com/NixOS/nixpkgs/blob/02c087f3fe673c9e317f206a627cb6b910336dbe/nixos/modules/services/desktops/gvfs.nix)

需要注意两个包变体：`pkgs.gvfs` 默认 `gnomeSupport = false`，关闭 keyring、GOA 等功能；`pkgs.gnome.gvfs` 覆盖为 `gnomeSupport = true`，也是上述服务模块的默认包。带 GNOME 支持的构建包含 libsecret 和 glib-networking，但仍需会话中有可用的密钥服务。[GVfs 包定义](https://github.com/NixOS/nixpkgs/blob/02c087f3fe673c9e317f206a627cb6b910336dbe/pkgs/by-name/gv/gvfs/package.nix)、[GNOME 包变体](https://github.com/NixOS/nixpkgs/blob/02c087f3fe673c9e317f206a627cb6b910336dbe/pkgs/desktops/gnome/default.nix)

接入时分别验证以下条件：

- Files 能加载 GIO 的 GVfs 模块，SFTP 和 SMB 后端能经会话 D-Bus 激活。
- Niri 会话和被激活的服务获得正确的 SSH agent 环境。
- 若提供保存密码，Secret Service 可访问，锁定状态能正确处理；是否启用 GNOME Keyring 按用户现有服务决定。
- 若提供自动发现，Avahi、WSDD 及必要网络通信各自可用；发现失败不影响直接输入地址。
- 若提供外部应用桥接，GVfs FUSE 可用，且实际打开与保存行为通过验证。

仅在 Files 包装器里修改 PATH 不能保证由 D-Bus 激活的守护进程也能找到辅助程序。依赖配置需要同时覆盖应用和服务的运行环境。

## 与当前代码的衔接

| 现有部分 | 当前约束 | 建议调整 |
| --- | --- | --- |
| `app/place_pages/network.rs` | 读取 `/proc/self/mountinfo` 并展开 GVfs FUSE 挂载目录，输出 `Vec<PathBuf>` | 保留已有挂载发现；增加连接、历史、发现结果的数据模型，并识别同一资源的不同入口。 |
| `navigation.rs`、`directory.rs` | 历史、快照、条目身份以 `PathBuf` 表示；输入拒绝 URI | 增加 `FileLocation`，分别保存本地原始路径和远程 URI；本地解析规则继续保留。 |
| `directory.rs` 及详情读取 | 本地枚举时读取所有者、内容头和部分子项信息 | 远程先批量查询基础元数据；内容探测、子项计数和预览按需执行，控制额外往返。 |
| `operations/transfer.rs` | 本地路径、Linux 文件描述符和禁止覆盖重命名 | 保留本地执行器；增加选定的远程后端，复用上层冲突流程与结果呈现，分别验证提交语义。 |
| `file_clipboard/protocol.rs` | 公共 URI 列表只接受本地 `file:`，私有格式保存路径字节 | 扩展到远程 URI；保留非 UTF-8 本地路径兼容，验证与其它文件管理器的复制剪切互通。 |
| `places.rs` | 收藏、最近、固定位置及目录列设置使用原始 OS 路径 | 为远程位置增加持久化表示，兼容已有配置；密码不进入位置记录。 |
| `src/desktop.rs`、picker / Portal | 打开文件、打开终端与选择结果都依赖本地路径 | 增加 URI 打开或本地桥接的分派，并分别处理普通浏览和文件选择器的契约。 |

选择 remotefs 时，根据所选客户端建立异步运行时或后台线程；选择 rclone 时，通过结构化 API 管理请求和任务。若另行接入 GIO，可以让独立线程持有 GLib `MainContext`，与 GPUI 交换请求和事件。GLib 支持把 GIO 异步结果交给线程自己的主循环。[线程默认 MainContext](https://docs.gtk.org/glib/method.MainContext.push_thread_default.html)

位置与基础 I/O 可以放在公共库的 `src/filesystem.rs` 及其子模块；网络页、连接表单、选择和视图状态留在 Files 内。先围绕实际使用的操作建立边界，再根据其它应用的需求扩展公共接口。

## 验证顺序与交付条件

| 阶段 | 需要跑通的流程 | 通过后解决的问题 |
| --- | --- | --- |
| 接入验证 | 独立后端连接 SFTP / SMB → 认证 → 列目录 → 上传下载 → 取消或断开，并核对运行依赖 | 确认 remotefs 或 rclone 的接入可行性，再确定最终选型。 |
| 位置与浏览 | URI 导航、共享列表、标签历史、收藏和最近；失败保留原视图 | 远程位置成为稳定的浏览对象。 |
| 文件操作 | 目录上传下载、新建、重命名、移动和删除；冲突、取消、断网及部分失败 | 达到日常使用所需的文件操作完整度。 |
| 桌面互操作 | URI 应用、本地桥接应用、编辑保存、剪贴板跨应用、选择器边界 | 文件可以交给真实工作流使用。 |
| 网络发现 | DNS-SD 与 WSDD，服务缺失、重复入口及网络切换 | 完成无需记住地址的网络浏览体验。 |

关键验收场景建议使用一个 SSH/SFTP 服务和一个 SMB 服务，分别覆盖密码、密钥、访客或只读共享、中文与空格文件名、非默认端口、大目录、传输中断网、认证取消、同名竞争、写入关闭失败和源删除失败。验证取消后重新连接正常，旧请求不会更新新标签内容，失败任务不会报告成功。

最先要解决的技术问题是独立后端的协议兼容性、认证交互和远程禁止覆盖语义；最先要确定的产品问题是网络页的连接状态模型、第一批协议，以及外部应用打开远程文件的路径。
