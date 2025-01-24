use crate::{
    account, begin, command_share, end, error, network, process, success, try_or_log, utils,
    warning,
};

use std::collections::HashMap;

pub fn fetch(
    config: &utils::Config,
    settings: &utils::Settings,
    default_account: &account::Account,
) {
    process!("FETCH");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );

    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    let selected_courses = try_or_log!(
        network::Session::load_selected_courses(&config.selected_courses),
        "加载已选课程"
    );

    // 没有已选课程，就提示用户选课
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        #[cfg(feature = "pb")]
        warning!("请运行 zacpb (--which | -w) 选择课程！");
        #[cfg(not(feature = "pb"))]
        warning!("请运行 zac (--which | -w) 选择课程！");
        return;
    }

    try_or_log!(
        command_share::fetch_core(config, settings, &session, selected_courses),
        "FETCH"
    );

    success!("FETCH");
}

pub fn submit(config: &utils::Config, default_account: &account::Account) {
    process!("SUBMIT");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );

    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::submit_core(config, &session), "SUBMIT");

    success!("SUBMIT");
}

pub fn upgrade(config: &utils::Config, default_account: &account::Account) {
    process!("UPGRADE");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::upgrade_core(config, &session), "UPGRADE");

    success!("UPGRADE");
}

pub fn config(
    config: &utils::Config,
    settings: &mut utils::Settings,
    accounts: &mut HashMap<String, account::Account>,
) {
    process!("CONFIG");

    try_or_log!(
        command_share::config_core(config, settings, accounts),
        "CONFIG"
    );

    success!("CONFIG");
}

/// 选择课程
/// 允许啥课程都不选
pub fn which(config: &utils::Config) {
    process!("WHICH");

    try_or_log!(command_share::which_core(config), "WHICH");

    success!("WHICH");
}

pub fn task(config: &utils::Config, default_account: &account::Account) {
    process!("TASK");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::task_core(config, &session), "TASK");

    success!("TASK");
}

pub fn grade(config: &utils::Config, default_account: &account::Account) {
    process!("GRADE");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    command_share::grade_core(config, default_account, &session).unwrap();

    success!("GRADE");
}

pub fn g(config: &utils::Config, default_account: &account::Account) {
    process!("GRADE");

    begin!("登录");
    let session = try_or_log!(
        network::Session::try_new(config.cookies.clone()),
        "创建会话"
    );
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    command_share::g_core(config, default_account, &session).unwrap();

    success!("GRADE");
}
