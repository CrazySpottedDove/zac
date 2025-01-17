# ZAC (Zju-Assistant-Cli)

一个用于代替雪灾浙大网页端的命令行操作工具，支持拉取课件，（提交作业：待实现）

## Why?

* 首先，雪灾浙大它很慢，我又是急性子！

* 其次，有前辈的项目 (<https://github.com/PeiPei233/zju-learning-assistant.git>) 可以参考，我又刚好想练习 rust！
* 再次，我是命令行爱好者!

## How to Use?

```help
Usage: zac < --fetch| --submit| --upgrade| --config>

Options:
  -f, --fetch    拉取课件。如果不知道该做什么，它会带着你做一遍
  -s, --submit   提交作业，尚未完成
  -u, --upgrade  一般在升学期时用，更新课程列表
  -c, --config   配置用户，存储目录，是否 ppt 转 pdf
  -h, --help     Print help
  -V, --version  Print version
```

## How to Download?

Release中提供了构建好的 windows 版本和 linux 版本。

## How to Build?

在 linux 环境中克隆本项目。

对于 linux 版本，首先确保你拥有 rustc， cargo，然后在项目根目录使用

```bash
cargo build --release
```

如果希望构建 windows 版本，则确保你拥有交叉编译工具 x86_64-w64-mingw32-gcc，并通过 rustup 添加目标 x86_64-pc-windows-gnu

```bash
# 示例：arch 系列
sudo pacman -S rustup mingw-w64-gcc
rustup update stable
rustup target add x86_64-pc-windows-gnu
```

然后，在项目根目录使用

```bash
cargo build --target x86_64-pc-windows-gnu --release
```
