use crate::{
    account, command_share, error, network, process, success, try_or_exit, try_or_throw, utils,
    warning,
};

use anyhow::Result;
use std::thread::{self, JoinHandle};

// 交互模式专用的预登录操作，希望减少用户等待登录时间
pub fn pre_login(
    default_account: account::AccountData,
    session: network::Session,
) -> JoinHandle<()> {
    thread::spawn(move || {
        #[cfg(debug_assertions)]
        process!("PRE_LOGIN");
        try_or_exit!(session.login(&default_account), "登录");
    })
}

/// 在 fetch 之前，应当保证预登录线程 join 成功或者现有 session 可用
pub fn fetch(settings: &utils::Settings, session: &network::Session) -> Result<()> {
    process!("FETCH");

    let selected_courses = try_or_throw!(session.load_selected_courses(), "加载已选课程");

    // 没有已选课程，就提示用户选课
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        warning!("请运行 (which | w) 选择课程！");
        return Ok(());
    }

    try_or_throw!(
        command_share::fetch_core(settings, session, selected_courses),
        "FETCH"
    );

    success!("FETCH");

    Ok(())
}

pub fn submit(session: &network::Session) -> Result<()> {
    process!("SUBMIT");

    try_or_throw!(command_share::submit_core(session), "SUBMIT");

    success!("SUBMIT");

    Ok(())
}

pub fn upgrade(session: &network::Session) -> Result<()> {
    process!("UPGRADE");

    try_or_throw!(command_share::upgrade_core(session), "UPGRADE");

    success!("UPGRADE");

    Ok(())
}

pub fn config(
    settings: &mut utils::Settings,
    account: &mut account::Account,
    session: &network::Session,
) -> Result<()> {
    process!("CONFIG");
    try_or_throw!(
        command_share::config_core(settings, account, session),
        "CONFIG"
    );

    success!("CONFIG");
    Ok(())
}

pub fn which(session: &network::Session) -> Result<()> {
    process!("WHICH");

    try_or_throw!(command_share::which_core(session), "WHICH");

    success!("WHICH");

    Ok(())
}

pub fn task(session: &network::Session) -> Result<()> {
    process!("TASK");

    try_or_throw!(command_share::task_core(session), "TASK");

    success!("TASK");

    Ok(())
}

pub fn grade(session: &network::Session, default_account: &account::AccountData) -> Result<()> {
    process!("GRADE");

    try_or_throw!(
        command_share::grade_core(&default_account, session),
        "GRADE"
    );

    success!("GRADE");

    Ok(())
}

pub fn g(session: &network::Session, default_account: &account::AccountData) -> Result<()> {
    process!("GRADE");

    try_or_throw!(command_share::g_core(default_account, session), "GRADE");

    success!("GRADE");

    Ok(())
}
