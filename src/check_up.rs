use crate::{account, command_blocking, error, network, process, try_or_exit, utils, warning};
use std::collections::HashMap;
use std::path::PathBuf;
use std::io::Write;

#[cfg(debug_assertions)]
use crate::success;

/// 保证配置定位、配置文件(必须有存储目录)正确!
pub fn config_up() -> (utils::Config, utils::Settings) {
    #[cfg(debug_assertions)]
    process!("SETUP");

    let config = try_or_exit!(utils::Config::init(), "初始化配置文件");

    let mut settings = try_or_exit!(utils::Settings::load(&config.settings), "读取配置文件");

     // 处理没设置存储目录的情况
    if settings.storage_dir == PathBuf::new() {
        warning!("未设置存储目录 => 设置存储目录");
        print!("请输入存储目录：");
        std::io::stdout().flush().unwrap();
        let mut storage_dir = String::new();

        try_or_exit!(std::io::stdin().read_line(&mut storage_dir), "读取存储目录");

        try_or_exit!(
            utils::Settings::set_storage_dir(&mut settings, &config.settings, &storage_dir.trim()),
            "设置存储目录"
        );
    }

    #[cfg(debug_assertions)]
    success!("SETUP");

    (config, settings)
}

/// 保证至少有一个默认账号！
pub fn account_up(
    config: &utils::Config,
    settings: &mut utils::Settings,
) -> (HashMap<String, account::Account>, account::Account) {
    #[cfg(debug_assertions)]
    process!("ACCOUNTUP");

    let mut accounts = try_or_exit!(
        account::Account::get_accounts(&config.accounts),
        "获取已知账号"
    );

    // 处理没有已知账号的情况
    if accounts.is_empty() {
        warning!("未发现已知的账号 => 创建账号");
        try_or_exit!(
            account::Account::add_account(
                &config.accounts,
                &config.settings,
                &mut accounts,
                settings,
            ),
            "添加用户"
        );
    }

    let default_account = try_or_exit!(
        account::Account::get_default_account(&accounts, &settings.user),
        "获取默认账号"
    );

    #[cfg(debug_assertions)]
    success!("ACCOUNTUP");

    (accounts, default_account)
}

/// 保证已有课程列表！
pub fn course_up(
    config: &utils::Config,
    default_account: &account::Account,
){
    #[cfg(debug_assertions)]
    process!("COURSEUP");

    let  semester_course_map = try_or_exit!(
        network::Session::load_semester_course_map(&config.courses),
        "加载 学期->课程 映射表"
    );

    // 处理课程列表为空的情况
    if semester_course_map.is_empty() {
        warning!("无 学期->课程 映射表 => 获取学期课程列表");
        let session = try_or_exit!(network::Session::try_new(), "建立会话");

        try_or_exit!(session.login(default_account), "登录");

        let semester_map = try_or_exit!(session.get_semester_map(), "获取学期引射表");

        let course_list = try_or_exit!(session.get_course_list(), "获取课程列表");

        try_or_exit!(
            network::Session::store_semester_course_map(&config.courses, course_list, semester_map),
            "存储 学期->课程 映射表"
        );

    }

    #[cfg(debug_assertions)]
    success!("COURSEUP");

}

pub fn all_up()->(utils::Config, utils::Settings, HashMap<String, account::Account>, account::Account){
    let (config, mut settings) = config_up();
    let (accounts, default_account) = account_up(&config, &mut settings);
    course_up(&config, &default_account);
    (config, settings, accounts, default_account)
}

/// 切换默认账号后，重新登陆并刷新 学期->课程 映射表
pub fn after_change_default_account(
    config: &utils::Config,
    default_account: &account::Account,
) -> network::Session {
    process!("更换用户 => 更新 学期->课程 映射表与已选课程");
    let new_session = try_or_exit!(network::Session::try_new(), "建立会话");
    try_or_exit!(new_session.login(default_account), "登录");
    let semester_map = try_or_exit!(new_session.get_semester_map(), "获取学期映射表");

    let course_list = try_or_exit!(new_session.get_course_list(), "获取课程列表");

    try_or_exit!(
        network::Session::store_semester_course_map(&config.courses, course_list, semester_map,),
        "存储 学期->课程 映射表"
    );

    command_blocking::which(config);
    new_session
}
