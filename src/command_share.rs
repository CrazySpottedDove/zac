use crate::utils::{MULTISELECT_PROMPT, SELECT_PROMPT};
use crate::{
    account, begin, completer, end, error, network, success, try_or_throw, utils, warning,
};
use dialoguer::{theme::ColorfulTheme, MultiSelect, Select};
use std::io::Write;

use anyhow::Result;
use std::thread::{self, JoinHandle};

pub fn fetch_core(
    settings: &utils::Settings,
    session: &network::Session,
    selected_courses: Vec<network::CourseFull>,
) -> Result<()> {
    let activity_upload_record =
        try_or_throw!(session.load_activity_upload_record(), "加载已下载课件记录");

    try_or_throw!(
        session.fetch_activity_uploads(selected_courses, activity_upload_record, settings,),
        "拉取新课件"
    );
    Ok(())
}

/// 1. 异步实现获取最新作业列表
/// 2. 选择需要上传的文件
/// 3. 异步实现上传文件到个人资料库
/// 4. 等待获取作业列表完成
/// 5. 选择需要上交的作业
/// 6. 询问是否备注
/// 7. 等待上传文件完成
/// 8. 发送上交作业请求
/// 9. 等待回复，报告结果
pub fn submit_core(session: &network::Session) -> Result<()> {
    // 1. 异步实现获取最新作业列表
    let session_cloned = session.clone();
    let get_homework_list_thread: JoinHandle<Result<Vec<network::Homework>>> =
        thread::spawn(move || {
            let home_work_list = try_or_throw!(session_cloned.get_homework_list(), "获取作业列表");
            Ok(home_work_list)
        });

    // 2. 选择需要上传的文件
    let file_path = completer::readin_path();
    if file_path == std::path::PathBuf::new() {
        return Ok(());
    }

    // 3. 异步实现上传文件到个人资料库
    let session_cloned = session.clone();
    let upload_file_thread: JoinHandle<Result<u64>> =
        thread::spawn(move || session_cloned.upload_file(&file_path));

    // 4. 等待获取作业列表完成
    begin!("获取作业列表");
    let homework_list = get_homework_list_thread.join().unwrap()?;
    end!("获取作业列表");

    // 5. 选择需要上交的作业
    let homework_name_list: Vec<String> = homework_list.iter().map(|hw| hw.name.clone()).collect();

    let selected_homework = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt(SELECT_PROMPT)
        .items(&homework_name_list)
        .interact_opt()
    {
        Ok(Some(index)) => &homework_list[index],
        _ => {
            warning!("取消选择作业");
            return Ok(());
        }
    };

    // 6. 询问是否备注
    let mut comment = String::new();
    println!("提交备注：(如不需要，直接回车)");
    std::io::stdin().read_line(&mut comment)?;
    comment = comment.trim().to_string();

    // 7. 等待上传文件完成
    begin!("上传文件到资料库");
    let upload_file_id = upload_file_thread.join().unwrap()?;
    end!("上传文件到资料库");

    // 8. 发送上交作业请求
    begin!("上交作业");
    try_or_throw!(
        session.handin_homework(selected_homework.id, upload_file_id, comment),
        "上交作业"
    );
    end!("上交作业");

    Ok(())
}

pub fn upgrade_core(session: &network::Session) -> Result<()> {
    begin!("获取学期映射表 & 课程列表");
    let (semester_map_result, course_list_result) = rayon::join(
        || session.get_semester_map_and_active_semester(),
        || session.get_course_list(),
    );
    let (semester_map, active_semester) = try_or_throw!(semester_map_result, "获取学期映射表");
    let course_list = try_or_throw!(course_list_result, "获取课程列表");
    end!("获取学期映射表 & 课程列表");

    let semester_course_map = network::Session::to_semester_course_map(course_list, semester_map);
    try_or_throw!(
        session.store_semester_course_map(&semester_course_map),
        "存储 学期->课程 映射表"
    );

    let active_semesters =
        network::Session::filter_active_semesters(&semester_course_map, &active_semester);
    let active_courses =
        network::Session::filter_active_courses(&semester_course_map, &active_semesters);

    try_or_throw!(
        session.store_active_courses(&active_courses),
        "存储活跃课程列表"
    );

    try_or_throw!(
        session.store_active_semesters(&active_semesters),
        "存储活跃学期列表"
    );

    Ok(())
}

fn config_help() {
    println!("\x1b[90m当前处于配置模式，直接输入子命令即可：\x1b[0m");
    println!("  \x1b[32madd-account (a)\x1b[0m        添加一个账户");
    println!("  \x1b[32mremove-account (r)\x1b[0m     删除一个账户");
    println!("  \x1b[32muser-default (u)\x1b[0m       设置默认用户");
    println!("  \x1b[32mstorage-dir (s)\x1b[0m        设置存储路径");
    println!("  \x1b[32mmp4-trashed (m)\x1b[0m        设置是否跳过下载 mp4 文件");
    println!("  \x1b[32mpdf-or-ppt (p)\x1b[0m         设置是否将 ppt 下载为 pdf");
    println!("  \x1b[32mlist-config (l)\x1b[0m        查看所有的配置");
    println!("  \x1b[32mhelp (h)\x1b[0m               显示此帮助");
    println!("  \x1b[33mCtrl + C\x1b[0m               退出配置模式");
}

/// 在 config 前，保证已经有了默认账号
/// 为了保证稳定性，任何切换默认账号的行为都要求重新刷新课程表
/// 这样的好处是不用返回线程了，可以直接返回新账号的会话
/// 而且如果想要使用后续功能，重新刷新课程表的操作是必要的
pub fn config_core(
    settings: &mut utils::Settings,
    account: &mut account::Account,
    session: &network::Session,
) -> Result<()> {
    config_help();

    let prompt = format!("zac/config > ");
    loop {
        let mut rl = completer::build_generic_editor(completer::CommandType::ConfigCommand);
        match rl.readline(&prompt) {
            Ok(cmd) => match cmd.as_str() {
                "add-account" | "a" => {
                    try_or_throw!(account.add_account(settings), "添加用户");
                    session.change_default_account(&account.default);
                }
                "remove-account" | "r" => {
                    let users: Vec<String> = account.accounts.keys().cloned().collect();

                    match Select::with_theme(&ColorfulTheme::default())
                        .with_prompt(SELECT_PROMPT)
                        .items(&users)
                        .default(0)
                        .interact_opt()
                    {
                        Ok(Some(index)) => {
                            let user_to_delete = &users[index];
                            if let Ok(is_default_changed) =
                                account.remove_account(settings, user_to_delete)
                            {
                                if is_default_changed {
                                    session.change_default_account(&account.default);
                                }
                            }
                        }
                        _ => {
                            warning!("取消删除账号");
                            continue;
                        }
                    }
                }
                "user-default" | "u" => {
                    let users: Vec<String> = account.accounts.keys().cloned().collect();
                    if users.len() == 1 {
                        warning!("只有一个账号 {}", users[0]);
                        continue;
                    }
                    match Select::with_theme(&ColorfulTheme::default())
                        .with_prompt(SELECT_PROMPT)
                        .items(&users)
                        .default(0)
                        .interact_opt()
                    {
                        Ok(Some(index)) => {
                            let user_to_set = &users[index];

                            if &settings.user == user_to_set {
                                warning!("该用户已经是默认用户");
                                continue;
                            }

                            try_or_throw!(settings.set_default_user(&user_to_set,), "设置默认用户");

                            account.default = account.accounts.get(user_to_set).unwrap().clone();

                            session.change_default_account(&account.default);
                        }
                        _ => {
                            warning!("取消设置默认账号");
                            continue;
                        }
                    }
                }
                "storage_dir" | "s" => {
                    println!("当前存储目录：{}", settings.storage_dir.display());
                    let storage_dir = completer::readin_storage_dir();
                    if storage_dir == "EXIT" {
                        continue;
                    }
                    try_or_throw!(settings.set_storage_dir(&storage_dir), "设置存储目录");
                }
                "mp4-trashed" | "m" => {
                    println!("当前值：{}", settings.mp4_trashed);
                    print!("是否跳过下载 mp4 文件？(y/n)");
                    std::io::stdout().flush().unwrap();
                    let mut is_pdf = String::new();
                    if std::io::stdin().read_line(&mut is_pdf).is_err() {
                        error!("读取指令失败");
                        continue;
                    }
                    match is_pdf.trim() {
                        "y" => try_or_throw!(
                            settings.set_mp4_trashed(true),
                            "设置是否跳过下载 mp4 文件"
                        ),
                        "n" => try_or_throw!(
                            settings.set_mp4_trashed(false),
                            "设置是否跳过下载 mp4 文件"
                        ),
                        _ => warning!("输入无效"),
                    }
                }
                "pdf-or-ppt" | "p" => {
                    println!("当前值：{}", settings.is_pdf);
                    print!("是否将 ppt 下载为 pdf？(y/n)");
                    std::io::stdout().flush().unwrap();
                    let mut is_pdf = String::new();
                    if std::io::stdin().read_line(&mut is_pdf).is_err() {
                        error!("读取指令失败");
                        continue;
                    }
                    match is_pdf.trim() {
                        "y" => try_or_throw!(settings.set_is_pdf(true), "设置下载 ppt 格式"),
                        "n" => try_or_throw!(settings.set_is_pdf(false), "设置下载 ppt 格式"),
                        _ => warning!("输入无效"),
                    }
                }
                "list-config" | "l" => {
                    try_or_throw!(settings.list(), "查看配置");
                }
                "help" | "h" | "" => {
                    config_help();
                }
                _ => {
                    warning!("未知的命令！");
                    config_help();
                }
            },
            Err(rustyline::error::ReadlineError::Interrupted) => {
                success!("退出配置模式");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                success!("退出配置模式");
                break;
            }
            Err(e) => {
                error!("输入错误：{e}");
            }
        }
    }
    Ok(())
}

/// 在 which 前，保证已经有了默认账号、课程列表
/// 选择课程
/// 允许啥课程都不选
pub fn which_core(session: &network::Session) -> Result<()> {
    let semester_course_map =
        try_or_throw!(session.load_semester_course_map(), "加载 学期->课程 映射表");

    let mut semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
    semester_list.sort_by(|b, a| network::compare_semester(a, b));

    let mut selected_courses = Vec::new();
    match MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(MULTISELECT_PROMPT)
        .items(&semester_list)
        .interact_opt()
    {
        Ok(Some(indices)) => {
            for index in indices {
                let semester = &semester_list[index];
                let course_list = semester_course_map.get(semester).unwrap();
                let course_names: Vec<String> =
                    course_list.iter().map(|c| c.name.clone()).collect();

                if course_names.is_empty() {
                    warning!("该学期没有课程可以选择");
                    continue;
                }

                match MultiSelect::with_theme(&ColorfulTheme::default())
                    .with_prompt(MULTISELECT_PROMPT)
                    .items(&course_names)
                    .interact_opt()
                {
                    Ok(Some(course_indices)) => {
                        for course_index in course_indices {
                            let course = &course_list[course_index];
                            selected_courses.push(network::CourseFull {
                                id: course.id,
                                semester: semester.clone(),
                                name: course.name.clone(),
                            });
                        }
                    }
                    _ => {
                        println!("取消选择本学期课程");
                        continue;
                    }
                }
            }
        }
        _ => {
            println!("取消选择课程");
            return Ok(());
        }
    }
    try_or_throw!(
        session.store_selected_courses(&selected_courses),
        "存储已选课程"
    );
    Ok(())
}

pub fn task_core(session: &network::Session) -> Result<()> {
    begin!("获取作业列表");
    let homework_list = try_or_throw!(session.get_homework_list(), "获取作业列表");
    end!("获取作业列表");

    if homework_list.is_empty() {
        println!("没有作业 :)");
        return Ok(());
    }

    for homework in homework_list {
        println!("  {}", homework.name);
    }
    Ok(())
}

pub fn grade_core(account: &account::AccountData, session: &network::Session) -> Result<()> {
    try_or_throw!(session.get_grade(&account), "获取成绩列表");
    Ok(())
}

pub fn g_core(account: &account::AccountData, session: &network::Session) -> Result<()> {
    try_or_throw!(session.get_g(&account), "获取成绩列表");
    Ok(())
}

pub fn polling_core(session: &network::Session, account: &account::AccountData) -> Result<()> {
    try_or_throw!(session.polling(account), "持续查询成绩");
    Ok(())
}
