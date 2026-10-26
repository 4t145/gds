set positional-arguments

# 直接求值开发环境，避免无 Git 的工作目录将 target/ 一并复制进 Nix store。
develop := "nix develop --impure --expr 'import ./shell.nix'"

# 列出常用开发命令。
default:
    @just --list

# 编译并启动开发版，可传目录或应用参数。
dev *args:
    {{develop}} --command cargo run --bin gds-files -- "$@"

# 进入完整的 Nix 开发环境。
shell:
    {{develop}}

# 构建开发版二进制。
build:
    {{develop}} --command cargo build --bin gds-files

# 检查全部 Rust 目标。
check:
    {{develop}} --command cargo check --all-targets

# 运行测试，可传测试名称等 Cargo 参数。
test *args:
    {{develop}} --command cargo test "$@"

# 格式化项目代码。
fmt:
    {{develop}} --command cargo fmt --all

# 只检查格式，不修改文件。
fmt-check:
    {{develop}} --command cargo fmt --all --check
