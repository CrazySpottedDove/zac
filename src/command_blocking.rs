use crate::{
    account, begin, command_share, end, error, network, process, success, try_or_log, utils,
    warning,
};

pub fn fetch(
    default_account: &account::AccountData,
    settings: &utils::Settings,
    session: &network::Session,
) {
    process!("FETCH");

    begin!("登录");
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    let selected_courses = try_or_log!(session.load_selected_courses(), "加载已选课程");

    // 没有已选课程，就提示用户选课
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        warning!("请运行 zac (--which | -w) 选择课程！");
        return;
    }

    try_or_log!(
        command_share::fetch_core(settings, &session, selected_courses),
        "FETCH"
    );

    success!("FETCH");
}

pub fn submit(session: &network::Session, default_account: &account::AccountData) {
    process!("SUBMIT");

    begin!("登录");
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::submit_core(&session), "SUBMIT");

    success!("SUBMIT");
}

pub fn upgrade(session: &network::Session, default_account: &account::AccountData) {
    process!("UPGRADE");

    begin!("登录");
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::upgrade_core(&session), "UPGRADE");

    success!("UPGRADE");
}

pub fn config(
    settings: &mut utils::Settings,
    account: &mut account::Account,
    session: &network::Session,
) {
    process!("CONFIG");

    try_or_log!(
        command_share::config_core(settings, account, session),
        "CONFIG"
    );

    success!("CONFIG");
}

/// 选择课程
/// 允许啥课程都不选
pub fn which(session: &network::Session) {
    process!("WHICH");

    try_or_log!(command_share::which_core(session), "WHICH");

    success!("WHICH");
}

pub fn task(session: &network::Session, default_account: &account::AccountData) {
    process!("TASK");

    begin!("登录");

    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::task_core(session), "TASK");

    success!("TASK");
}

pub fn grade(session: &network::Session, default_account: &account::AccountData) {
    process!("GRADE");

    begin!("登录");
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::grade_core(default_account, session), "GRADE");

    success!("GRADE");
}

pub fn g(session: &network::Session, default_account: &account::AccountData) {
    process!("GRADE");

    begin!("登录");
    try_or_log!(session.login(&default_account), "登录");
    end!("登录");

    try_or_log!(command_share::g_core(default_account, &session), "GRADE");

    success!("GRADE");
}
