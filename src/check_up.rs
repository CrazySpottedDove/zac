use crate::{
    account, begin, command_blocking, completer, end, error, network, process, try_or_exit, utils,
    warning,
};
use std::path::PathBuf;

#[cfg(debug_assertions)]
use crate::success;

/// 保证配置定位、配置文件(必须有存储目录)正确!
///
/// 返回值：
/// ```txt
/// (
///    (
///         path_accounts,
///         path_courses,
///         path_selected_courses,
///         path_activity_upload_record,
///         path_cookies,
///         path_active_courses,
///     ),
///     settings,
/// )
/// ```
pub fn config_up() -> (
    (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf, PathBuf, PathBuf),
    utils::Settings,
) {
    #[cfg(debug_assertions)]
    process!("SETUP");
    let (
        path_accounts,
        path_settings,
        path_courses,
        path_selected_courses,
        path_activity_upload_record,
        path_cookies,
        path_active_courses,
        path_active_semesters,
    ) = try_or_exit!(utils::Config::init(), "初始化配置文件");

    let mut settings = try_or_exit!(utils::Settings::load(path_settings), "读取配置文件");

    // 处理没设置存储目录的情况
    if settings.storage_dir == PathBuf::new() {
        let storage_dir = completer::readin_storage_dir();

        try_or_exit!(
            utils::Settings::set_storage_dir(&mut settings, &storage_dir),
            "设置存储目录"
        );
    }

    #[cfg(debug_assertions)]
    success!("SETUP");

    (
        (
            path_accounts,
            path_courses,
            path_selected_courses,
            path_activity_upload_record,
            path_cookies,
            path_active_courses,
            path_active_semesters,
        ),
        settings,
    )
}

/// 保证至少有一个默认账号！
pub fn account_up(path_accounts: PathBuf, settings: &mut utils::Settings) -> account::Account {
    #[cfg(debug_assertions)]
    process!("ACCOUNTUP");

    let account = try_or_exit!(
        account::Account::init(path_accounts, settings),
        "初始化账号"
    );

    #[cfg(debug_assertions)]
    success!("ACCOUNTUP");

    account
}

pub fn session_up(
    path_cookies: PathBuf,
    path_courses: PathBuf,
    path_active_courses: PathBuf,
    path_selected_courses: PathBuf,
    path_activity_upload_record: PathBuf,
    path_active_semesters: PathBuf,
) -> network::Session {
    #[cfg(debug_assertions)]
    process!("SESSIONUP");

    let session = try_or_exit!(
        network::Session::try_new(
            path_cookies,
            path_courses,
            path_active_courses,
            path_selected_courses,
            path_activity_upload_record,
            path_active_semesters
        ),
        "建立会话"
    );

    #[cfg(debug_assertions)]
    success!("SESSIONUP");

    session
}

/// 保证已有课程列表！
pub fn course_up(session: &network::Session, default_account: &account::AccountData) {
    #[cfg(debug_assertions)]
    process!("COURSEUP");

    let semester_course_map =
        try_or_exit!(session.load_semester_course_map(), "加载 学期->课程 映射表");

    // 处理课程列表为空的情况
    if semester_course_map.is_empty() {
        warning!("无 学期->课程 映射表 => 获取学期课程列表 & 活跃课程列表");
        begin!("获取学期课程列表 & 活跃课程列表");
        // let session = try_or_exit!(
        //     network::Session::try_new(config.cookies.clone()),
        //     "建立会话"
        // );

        try_or_exit!(session.login(default_account), "登录");

        let (semester_map , active_semester)= try_or_exit!(session.get_semester_map_and_active_semester(), "获取学期映射表");

        let course_list = try_or_exit!(session.get_course_list(), "获取课程列表");

        let semester_course_map =
            network::Session::to_semester_course_map(course_list, semester_map);

        let active_semesters = network::Session::filter_active_semesters(&semester_course_map, &active_semester);
        let active_courses = network::Session::filter_active_courses(&semester_course_map, &active_semesters);

        try_or_exit!(
            session.store_semester_course_map(&semester_course_map),
            "存储 学期->课程 映射表"
        );

        try_or_exit!(
            session.store_active_courses(&active_courses),
            "存储活跃课程列表"
        );

        try_or_exit!(
            session.store_active_semesters(&active_semesters),
            "存储活跃学期列表"
        );
        end!("获取学期课程列表 & 活跃课程列表");
    }

    #[cfg(debug_assertions)]
    success!("COURSEUP");
}

/// 在程序执行时，始终保证：
/// 1. 配置文件正确
/// 2. 至少有一个默认账号
/// 3. 有课程列表和活跃课程列表
pub fn all_up() -> (utils::Settings, account::Account, network::Session) {
    let (
        (
            path_accounts,
            path_courses,
            path_selected_courses,
            path_activity_upload_record,
            path_cookies,
            path_active_courses,
            path_active_semesters,
        ),
        mut settings,
    ) = config_up();
    let account = account_up(path_accounts, &mut settings);
    let session = session_up(
        path_cookies,
        path_courses,
        path_active_courses,
        path_selected_courses,
        path_activity_upload_record,
        path_active_semesters,
    );
    course_up(&session, &account.default);
    (settings, account, session)
}

impl network::Session {
    /// 切换默认账号后，重新登陆并刷新 学期->课程 映射表
    pub fn change_default_account(&self, default_account: &account::AccountData) {
        process!("更换用户 => 更新 学期->课程 映射表与已选课程");

        begin!("重新登录");
        try_or_exit!(self.relogin(default_account), "重新登录");
        end!("重新登录");

        begin!("获取学期课程列表 & 活跃课程列表");
        let (semester_map,active_semester) = try_or_exit!(self.get_semester_map_and_active_semester(), "获取学期映射表");

        let course_list = try_or_exit!(self.get_course_list(), "获取课程列表");

        let semester_course_map =
            network::Session::to_semester_course_map(course_list, semester_map);
        let active_semesters = network::Session::filter_active_semesters(&semester_course_map, &active_semester);
        let active_courses = network::Session::filter_active_courses(&semester_course_map, &active_semesters);

        try_or_exit!(
            self.store_semester_course_map(&semester_course_map),
            "存储 学期->课程 映射表"
        );

        try_or_exit!(
            self.store_active_courses(&active_courses),
            "存储活跃课程列表"
        );

        try_or_exit!(
            self.store_active_semesters(&active_semesters),
            "存储活跃学期列表"
        );

        end!("获取学期课程列表 & 活跃课程列表");

        command_blocking::which(self);
    }
}
