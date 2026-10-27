# 在 NixOS 安装 GDS

GDS 是原生桌面应用套件，目前包含 Files 和 Agents，支持 `x86_64-linux` / `aarch64-linux`。各应用有独立的软件包、启动入口和 NixOS 模块。

| 应用 | 软件包 / 启动入口 | NixOS 模块 | 启用选项 |
| --- | --- | --- | --- |
| Files | `gds-files` | `nixosModules.gds-files` | `programs.gds-files.enable` |
| Agents | `gds-agents` | `nixosModules.gds-agents` | `programs.gds-agents.enable` |

`packages.<system>.gds` 和 `packages.<system>.default` 包含所有应用的二进制、桌面入口和资源。启动时需显式选择应用；仓库不提供默认应用。

## 构建、安装和运行

在仓库目录执行：

```sh
# 构建整个套件，或单独构建应用：
nix build
nix build .#gds-files
nix build .#gds-agents

# 安装到当前用户的 Nix profile：
just install
just install files
just install agents
# 等价命令分别为：
# nix profile install .#gds
# nix profile install .#gds-files
# nix profile install .#gds-agents

# 在图形会话中直接运行：
nix run .#gds-files -- "$HOME"
nix run .#gds-agents -- --compact
```

套件和单应用包提供相同的文件，profile 中选其中一种安装方式即可。构建产物包含所需动态库包装，不依赖开发环境。

## 接入系统 flake

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    gds.url = "github:4t145/gds";
    # 本地开发可改为 git+file:///absolute/path/to/gds。
  };

  outputs = { nixpkgs, gds, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        gds.nixosModules.default
        {
          programs.gds-files.enable = true;
          programs.gds-agents.enable = true;
        }
      ];
    };
  };
}
```

`nixosModules.default` 导入各应用模块，不自动启用应用。只需一个应用时，也可仅导入对应的 `nixosModules.gds-files` 或 `nixosModules.gds-agents`。

模块默认使用 GDS 锁定的 nixpkgs 构建软件包，无需设置 `gds.inputs.nixpkgs.follows`。可通过各自的 `programs.<应用>.package` 定制包。仅安装套件包不会启用应用的系统集成；这些设置由对应模块负责。

在系统配置目录执行（把 `myhost` 换成自己的配置名）：

```sh
nix flake update gds
sudo nixos-rebuild switch --flake .#myhost
```

本地 Git flake 仅包含 Git 已跟踪的文件，新增的 Nix 定义和资源需要先加入 Git。首次构建会下载 Rust 依赖并编译图形栈。

## 应用集成

- [Files：默认文件管理器、设备和文件选择 Portal](apps/files/nixos.md)
- [Agents：独立前端和 Bas 服务连接](apps/agents/migration.md#nix-安装入口)

各应用的打包定义和模块位于 `nix/apps/<应用>/`，共用 `nix/package.nix` 的 Rust 构建和运行库包装。flake 按该目录生成应用包、启动入口和模块；Cargo feature 从 `Cargo.toml` 中对应二进制的 `required-features` 读取。
