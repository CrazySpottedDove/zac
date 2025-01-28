mod account;
mod check_up;
mod command_async;
mod command_blocking;
mod command_share;
mod completer;
mod network;
mod utils;

use clap::{ArgGroup, CommandFactory, Parser};
use colored::Colorize;

#[cfg(feature = "pb")]
const CMD_NAME: &str = "zacpb";

#[cfg(not(feature = "pb"))]
const CMD_NAME: &str = "zac";

#[cfg(feature = "pb")]
const CMD_ABOUT: &str = "zacpb(zju-assistant-cli::progress-bar) 是一个用于获取或上传雪灾浙大资源的命令行工具。若想了解更多，见 https://github.com/CrazySpottedDove/zac";

#[cfg(not(feature = "pb"))]
const CMD_ABOUT: &str = "zac(zju-assistant-cli) 是一个用于获取或上传雪灾浙大资源的命令行工具。若想了解更多，见 https://github.com/CrazySpottedDove/zac";

#[derive(Parser)]
#[command(
    name = CMD_NAME,
    version,
    about = CMD_ABOUT,
    long_about = None,
    group(
        ArgGroup::new("commands")
            .required(false)
            .args(&["fetch", "submit", "upgrade", "which","task","grade","config"])
    )
)]
struct Cli {
    /// 拉取课件
    #[arg(short, long)]
    fetch: bool,
    /// 提交作业
    #[arg(short, long)]
    submit: bool,
    /// 更新课程列表，有新课时用
    #[arg(short, long)]
    upgrade: bool,
    /// 选择需要拉取的课程
    #[arg(short, long)]
    which: bool,
    /// 查看作业
    #[arg(short, long)]
    task: bool,
    /// 查看所有成绩
    #[arg(long)]
    grade: bool,
    /// 查看本学期成绩
    #[arg(long)]
    g: bool,
    /// 配置[用户，存储目录，是否 ppt 转 pdf，是否下载 mp4 文件]
    #[arg(short, long)]
    config: bool,
}

fn main() {
    let (config, mut settings, mut accounts, default_account) = check_up::all_up();
    let cli = Cli::parse();

    if cli.fetch {
        command_blocking::fetch(&config, &settings, &default_account);
    } else if cli.submit {
        command_blocking::submit(&config, &default_account);
    } else if cli.upgrade {
        command_blocking::upgrade(&config, &default_account);
    } else if cli.which {
        command_blocking::which(&config);
    } else if cli.task {
        command_blocking::task(&config, &default_account);
    } else if cli.config {
        command_blocking::config(&config, &mut settings, &mut accounts);
    } else if cli.grade {
        command_blocking::grade(&config, &default_account);
    } else if cli.g {
        command_blocking::g(&config, &default_account);
    } else {
        let mut pre_login_thread_wrapper = Some(command_async::pre_login(
            default_account,
            config.cookies.clone(),
        ));
        let mut new_session = None;
        Cli::command().print_help().unwrap();
        process!("交互模式 Ctrl+C 退出");
        let mut rl = completer::build_generic_editor(completer::CommandType::MainCommand);
        loop {
            match rl.readline(&format!("{} > ", CMD_NAME.blue())) {
                Ok(input) => match input.as_str() {
                    "fetch" | "f" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        if let Err(e) = command_async::fetch(&config, &settings, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "submit" | "s" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        if let Err(e) = command_async::submit(&config, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "upgrade" | "u" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        if let Err(e) = command_async::upgrade(&config, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "which" | "w" => {
                        if let Err(e) = command_async::which(&config) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "task" | "t" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        if let Err(e) = command_async::task(&config, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "grade" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        let default_account = try_or_exit!(
                            account::Account::get_default_account(&accounts, &settings.user),
                            "获取默认账号"
                        );

                        if let Err(e) = command_async::grade(&config, &default_account, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "g" => {
                        let session = new_session.get_or_insert_with(|| {
                            begin!("登录");
                            let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
                            let session = handle.join().unwrap();
                            end!("登录");
                            session
                        });
                        let default_account = try_or_exit!(
                            account::Account::get_default_account(&accounts, &settings.user),
                            "获取默认账号"
                        );
                        if let Err(e) = command_async::g(&config, &default_account, session) {
                            eprintln!("::>_<:: {}", e);
                        };
                    }
                    "config" | "c" => {
                        match command_async::config(&config, &mut settings, &mut accounts) {
                            Ok(s) => new_session = s,
                            Err(e) => {
                                eprintln!("::>_<:: {}", e)
                            }
                        };
                    }

                    "help" | "h" => {
                        Cli::command().print_help().unwrap();
                    }
                    _ => {
                        warning!("无效命令，请重新输入");
                    }
                },
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    #[cfg(feature = "pb")]
                    success!("退出 zacpb");
                    #[cfg(not(feature = "pb"))]
                    success!("退出 zac");
                    return;
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    #[cfg(feature = "pb")]
                    success!("退出 zacpb");
                    #[cfg(not(feature = "pb"))]
                    success!("退出 zac");
                    return;
                }
                Err(e) => {
                    error!("输入错误：{}", e);
                }
            }
        }
    }
}


