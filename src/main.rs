mod account;
mod check_up;
mod command_async;
mod command_blocking;
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
    /// 选择需要拉取的课程
    #[arg(short, long)]
    which: bool,
    /// 配置用户，存储目录，是否 ppt 转 pdf
    #[arg(short, long)]
    config: bool,
}

fn main() {
    let (config, mut settings, mut accounts, default_account) = check_up::all_up();
    let cli = Cli::parse();

    if cli.fetch {
        command_blocking::fetch(&config, &settings, &default_account);
    } else if cli.submit {
        command_blocking::submit();
    } else if cli.upgrade {
        command_blocking::upgrade(&config, &default_account);
    } else if cli.which {
        command_blocking::which(&config);
    } else if cli.config {
        command_blocking::config(&config, &mut settings, &mut accounts);
    } else {
        let mut pre_login_thread_wrapper = Some(command_async::pre_login(default_account));
        let mut new_session = None;
        process!("交互模式");
        Cli::command().print_help().unwrap();
        loop {
            process!("输入命令 (fetch|f, submit|s, upgrade|u, which|w, config|c, help|h) | (exit|q) 退出");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("读取输入失败");
            let input = input.trim();

            match input {
                "fetch" | "f" => {
                    let session = new_session.get_or_insert_with(|| {
                        // 取出 pre_login_thread 中的 JoinHandle（只取一次）
                        let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                        handle.join().unwrap()
                    });
                    command_async::fetch(&config, &settings, session);
                }
                "submit" | "s" => {
                    command_async::submit();
                }
                "upgrade" | "u" => {
                    let session = new_session.get_or_insert_with(|| {
                        // 取出 pre_login_thread 中的 JoinHandle（只取一次）
                        let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                        handle.join().unwrap()
                    });
                    command_async::upgrade(&config, session);
                }
                "config" | "c" => {
                    command_async::config(&config, &mut settings, &mut accounts);
                }
                "which" | "w" => {
                    command_async::which(&config);
                }
                "exit" | "q" => {
                    break;
                }
                "help" | "h" => {
                    Cli::command().print_help().unwrap();
                }
                _ => {
                    warning!("无效命令，请重新输入");
                }
            }
        }
    }
}
