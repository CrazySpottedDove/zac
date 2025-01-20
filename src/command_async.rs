use crate::{
    account, command_share, error, network, process, success, try_or_exit, try_or_log, utils,
    warning,
};

use std::collections::HashMap;

use std::thread::{self, JoinHandle};

// 交互模式专用的预登录操作，希望减少用户等待登录时间
pub fn pre_login(default_account: account::Account) -> JoinHandle<network::Session> {
    thread::spawn(move || {
        #[cfg(debug_assertions)]
        process!("PRE_LOGIN");

        let session = try_or_exit!(network::Session::try_new(), "创建会话");

        try_or_exit!(session.login(&default_account), "登录");

        session
    })
}

/// 在 fetch 之前，应当保证预登录线程 join 成功或者现有 session 可用
pub fn fetch(config: &utils::Config, settings: &utils::Settings, session: &network::Session) {
    process!("FETCH");

    let selected_courses = try_or_log!(
        network::Session::load_selected_courses(&config.selected_courses),
        "加载已选课程"
    );

    // 没有已选课程，就提示用户选课
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        warning!("请运行 (which | w) 选择课程！");
        return;
    }

    try_or_log!(
        command_share::fetch_core(config, settings, session, selected_courses),
        "FETCH"
    );

    success!("FETCH");
}

pub fn submit(config: &utils::Config, session: &network::Session) {
    process!("SUBMIT");

    try_or_log!(command_share::submit_core(config, session), "SUBMIT");

    success!("SUBMIT");
}

pub fn upgrade(config: &utils::Config, session: &network::Session) {
    process!("UPGRADE");

    try_or_log!(command_share::upgrade_core(config, session), "UPGRADE");

    success!("UPGRADE");
}

pub fn config(
    config: &utils::Config,
    settings: &mut utils::Settings,
    accounts: &mut HashMap<String, account::Account>,
) -> Option<network::Session> {
    process!("CONFIG");

    let new_session_wrapper = match command_share::config_core(config, settings, accounts) {
        Ok(session_wrapper) => session_wrapper,
        Err(e) => {
            error!("CONFIG: {}", e);
            None
        }
    };

    success!("CONFIG");
    new_session_wrapper
}

pub fn which(config: &utils::Config) {
    process!("WHICH");

    try_or_log!(command_share::which_core(config), "WHICH");

    success!("WHICH");
}

pub fn task(config: &utils::Config, session: &network::Session) {
    process!("TASK");

    try_or_log!(command_share::task_core(config, session), "TASK");

    success!("TASK");
}