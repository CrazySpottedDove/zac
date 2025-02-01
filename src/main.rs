use std::thread::JoinHandle;

use anyhow::Result;
use clap::{ArgGroup, CommandFactory, Parser};
use rustyline::history::FileHistory;
use rustyline::Editor;
use zac::check_up;
use zac::command_async;
use zac::command_blocking;
use zac::completer;
use zac::completer::GenericHelper;
use zac::update;
use zac::{account, network, utils};
use zac::{begin, end, error, process, success, warning,try_or_log};
const CMD_NAME: &str = "zac";

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
    #[arg(short)]
    g: bool,
    /// 配置[用户，存储目录，是否 ppt 转 pdf，是否下载 mp4 文件]
    #[arg(short, long)]
    config: bool,
    /// 执行更新
    #[arg(long)]
    update: bool,
}

fn guarantee_login_and_check_new_version(login_ready: &mut bool, pre_login_thread_wrapper: &mut Option<JoinHandle<()>>, check_new_version_thread:&mut Option<JoinHandle<bool>>){
    if !*login_ready {
        let have_new_version = check_new_version_thread.take().expect("线程句柄不可用");
        if have_new_version.join().unwrap(){
            println!("\x1b[90m检查到可用的新版本，可使用 update 更新~\x1b[0m")
        }
        begin!("登录");
        let handle = pre_login_thread_wrapper.take().expect("线程句柄不可用");
        handle.join().unwrap();
        end!("登录");
        *login_ready = true;
    }
}

fn single_iterative_term(
    rl: &mut Editor<GenericHelper, FileHistory>,
    session: &network::Session,
    account: &mut account::Account,
    settings: &mut utils::Settings,
    login_ready: &mut bool,
    pre_login_thread_wrapper: &mut Option<JoinHandle<()>>,
    check_new_version_thread_wrapper:&mut Option<JoinHandle<bool>>
) -> Result<bool> {
    match rl.readline(&format!("{} > ", CMD_NAME)) {
        Ok(input) => match input.as_str() {
            "fetch" | "f" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::fetch(settings, session)?;
            }
            "submit" | "s" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::submit(session)?;
            }
            "upgrade" | "u" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::upgrade(session)?;
            }
            "which" | "w" => {
                command_async::which(session)?;
            }
            "task" | "t" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::task(session)?;
            }
            "grade" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::grade(session, &account.default)?;
            }
            "g" => {
                guarantee_login_and_check_new_version(login_ready, pre_login_thread_wrapper,check_new_version_thread_wrapper);
                command_async::g(session, &account.default)?;
            }
            "config" | "c" => {
                command_async::config(settings, account, session)?;
            }
            "help" | "h" => {
                Cli::command().print_help()?;
            }
            "update" => {
                update::update()?;
            }
            "v"| "version" => {
                success!("v{}", env!("CARGO_PKG_VERSION"));
            }
            _ => {
                warning!("无效命令，请重新输入");
            }
        },
        Err(rustyline::error::ReadlineError::Interrupted)
        | Err(rustyline::error::ReadlineError::Eof) => {
            return Ok(true);
        }
        Err(e) => {
            error!("输入错误：{}", e);
        }
    }
    Ok(false)
}

fn main() {
    let (mut settings, mut account, session) = check_up::all_up();
    let cli = Cli::parse();

    if cli.fetch {
        command_blocking::fetch(&account.default, &settings, &session);
    } else if cli.submit {
        command_blocking::submit(&session, &account.default);
    } else if cli.upgrade {
        command_blocking::upgrade(&session, &account.default);
    } else if cli.which {
        command_blocking::which(&session);
    } else if cli.task {
        command_blocking::task(&session, &account.default);
    } else if cli.config {
        command_blocking::config(&mut settings, &mut account, &session);
    } else if cli.grade {
        command_blocking::grade(&session, &account.default);
    } else if cli.g {
        command_blocking::g(&session, &account.default);
    } else if cli.update {
        try_or_log!(update::update(),"更新");
    } else {
        let mut pre_login_thread_wrapper = Some(command_async::pre_login(
            account.default.clone(),
            session.clone(),
        ));
        let mut new_version_check_thread_wrapper = Some(update::check_update());
        let mut login_ready = false;
        Cli::command().print_help().unwrap();
        process!("交互模式 Ctrl+C 退出");
        let mut rl = completer::build_generic_editor(completer::CommandType::MainCommand);
        loop {
            match single_iterative_term(
                &mut rl,
                &session,
                &mut account,
                &mut settings,
                &mut login_ready,
                &mut pre_login_thread_wrapper,
                &mut new_version_check_thread_wrapper
            ) {
                Ok(true) => {
                    success!("退出 zac");
                    break;
                }
                Err(e) => {
                    error!("{}", e);
                }
                _ => {}
            }
        }
    }
}
