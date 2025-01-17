mod network;
mod utils;
mod account;
mod command;

use clap::{Parser,ArgGroup};

#[derive(Parser)]
#[command(
    name = "zac",
    version = "0.0.1",
    about = "zac(zju-assistant-cli) 是一个用于获取或上传雪灾浙大资源的命令行工具",
    long_about = None,
    group(
        ArgGroup::new("commands")
            .required(true)
            .args(&["fetch", "submit", "upgrade", "config"])
    )
)]
struct Cli {
    /// 拉取课件。如果不知道该做什么，它会带着你做一遍
    #[arg(short, long)]
    fetch: bool,
    /// 提交作业，尚未完成
    #[arg(short, long)]
    submit: bool,
    /// 一般在升学期时用，更新课程列表
    #[arg(short, long)]
    upgrade: bool,
    /// 配置用户，存储目录，是否 ppt 转 pdf
    #[arg(short, long)]
    config: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.fetch {
        command::fetch();
    }

    if cli.submit {
        command::submit();
    }

    if cli.upgrade {
        command::upgrade();
    }

    if cli.config {
        command::config();
    }
}
