# [ZAC (Zju-Assistant-Cli)](https://crazyspotteddove.github.io/projects/zac)

一个用于代替雪灾浙大网页端的命令行操作工具，支持拉取课件，提交作业。

## Why?

* 首先，雪灾浙大它很慢，我又是急性子！

* 其次，有前辈的项目 (<https://github.com/PeiPei233/zju-learning-assistant.git>) 可以参考，我又刚好想练习 rust！
* 再次，我是命令行爱好者!

## How to Use?

工具同时支持交互模式和一次性模式。

一次性模式用法如下：

```bash
Usage: zac [OPTIONS]

Options:
  -f, --fetch    拉取课件
  -s, --submit   提交作业
  -u, --upgrade  更新课程列表，有新课时用
  -w, --which    选择需要拉取的课程
  -t, --task     查看作业
      --grade    查看所有成绩
  -g             查看本学期成绩
  -p, --polling  持续查询本学期成绩
  -c, --config   配置[用户，存储目录，是否 ppt 转 pdf，是否下载 mp4 文件]
      --update   执行更新
  -h, --help     Print help
  -V, --version  Print version
```

更为推荐的方法是不加参数使用命令/直接双击（什？），进入交互模式。针对交互模式做了预登录，可以略微减少等待时间。

交互模式可用的命令和一次性模式一致。

```bash
当前处于交互模式，直接输入子命令即可：
  fetch (f)     拉取课件
  submit (s)    提交作业
  upgrade (u)   更新课程列表，有新课时用
  which (w)     选择需要拉取的课程
  task (t)      查看作业
  grade         查看所有成绩
  g             查看本学期成绩
  polling (p)   持续查询本学期成绩
  config (c)    配置 [用户，存储目录，是否 ppt 转 pdf，是否下载 mp4 文件]
  update        执行更新
  version (v)   显示版本信息
  help (h)      显示此帮助信息
  Ctrl + C      退出 zac
```

## Advanced Suggestions

作为命令行工具，推荐的是在终端中使用。本应用的主要耗时为登录雪灾浙大，而这一耗时可以通过保持应用开启避免。当这一耗时被避免，本应用可以保证所有相关操作速度快于雪灾浙大。而在资源占用上，应用待机消耗仅为 3MB，远低于浏览器消耗，相比终端本身的消耗也几乎可以忽略不计。因此，在终端上开一个分网格给 zac 是很好的方案。

另外，添加到环境变量，无需多言。

## Data Safety?

zac 工具所有网络请求仅指向雪灾浙大和 Github，其中有关 Github 的网络请求只与用户手动更新有关，保证所有个人信息都储存在本地。

## Is the Upload Function Relieable?

上传功能已通过 3.15 GB zip 文件测试。

## Completer Supported?

所有的命令和路径输入均实现了自动补全功能，可以使用 tab 或 → 补全。

## Glimpse of Running

* 获取作业列表，选择要拉取的文件

![alt text](./figures/task-which.png)

* 拉取课件

![alt text](./figures/fetch.png)

* 上传作业

![alt text](./figures/submit.png)

* 查看成绩

![alt text](./figures/grade.png)

## How to Download?

Release 中（Github 页面右侧）提供了构建好的、不同平台的 windows 版本、macos 版本和 linux 版本。

务必选择最新版本，它很有可能在提供新功能的同时修复旧功能的 bug，具体可见 commit 历史。

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
cargo build --release --target x86_64-pc-windows-gnu
```

如果直接在 Github 克隆本项目，则可以直接使用本项目的 workflows 文件获取不同平台构建结果。

## How to Update?

在 v0.1.1 版本后，zac 开始支持自更新功能，只需运行

```bash
zac --update
```

即可完成更新。

## Known Issues

* 当打开了 zac 并调整终端大小时，可能导致程序崩溃。该问题可能由 rustyline 引起，暂未找到解决方案。

```rust
assertion failed: fd != u32::MAX as RawFd
```

* zac 未购买 CA 签名，下载时可能被 defender 拦截，此为正常现象，请信任 zac（保留，加入白名单）。
