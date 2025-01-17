mod account;
mod command;
mod network;
mod utils;

use clap::{ArgGroup, CommandFactory, Parser};

#[cfg(feature = "pb")]
const CMD_NAME: &str = "zacpb";

#[cfg(not(feature = "pb"))]
const CMD_NAME: &str = "zac";

#[derive(Parser)]
#[command(
    name = CMD_NAME,
    version,
    about = "zac(zju-assistant-cli) 是一个用于获取或上传雪灾浙大资源的命令行工具",
    long_about = None,
    group(
        ArgGroup::new("commands")
            .required(false)
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
    } else if cli.submit {
        command::submit();
    } else if cli.upgrade {
        command::upgrade();
    } else if cli.config {
        command::config();
    } else {
        process!("交互模式");
        Cli::command().print_help().unwrap();
        loop {
            process!("输入命令 (fetch|f, submit|s, upgrade|u, config|c) | (exit|q) 退出");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("读取输入失败");
            let input = input.trim();

            match input {
                "fetch" | "f" => {
                    command::fetch();
                }
                "submit" | "s" => {
                    command::submit();
                }
                "upgrade" | "u" => {
                    command::upgrade();
                }
                "config" | "c" => {
                    command::config();
                }
                "exit" | "q" => {
                    break;
                }
                _ => {
                    warning!("无效命令，请重新输入");
                }
            }
        }
    }
}
