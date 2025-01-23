use crate::utils::{MULTISELECT_PROMPT, SELECT_PROMPT};
use crate::{
    account, begin, check_up, completer, end, error, network, success, try_or_exit, try_or_throw,
    utils, warning,
};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, MultiSelect, Select};

use std::collections::HashMap;
use std::io::Write;

use anyhow::Result;
use std::thread::{self, JoinHandle};




pub fn fetch_core(
    config: &utils::Config,
    settings: &utils::Settings,
    session: &network::Session,
    selected_courses: Vec<network::CourseFull>,
) -> Result<()> {


    let activity_upload_record = try_or_throw!(
        network::Session::load_activity_upload_record(&config.activity_upload_record),
        "加载已下载课件记录"
    );


    try_or_throw!(
        session.fetch_activity_uploads(
            &settings.storage_dir,
            &config.activity_upload_record,
            selected_courses,
            activity_upload_record,
            settings,
        ),
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
pub fn submit_core(config: &utils::Config, session: &network::Session) -> Result<()> {
    // 1. 异步实现获取最新作业列表
    let path_courses = config.courses.clone();
    let session_cloned = session.clone();
    let get_homework_list_thread: JoinHandle<Vec<network::Homework>> = thread::spawn(move || {
        let home_work_list = try_or_exit!(
            session_cloned.get_homework_list(&path_courses),
            "获取作业列表"
        );
        home_work_list
    });

    // 2. 选择需要上传的文件
    let file_path = completer::readin_path();
    if file_path == std::path::PathBuf::new() {
        return Ok(());
    }

    // 3. 异步实现上传文件到个人资料库
    let file_path_cloned = file_path.clone();
    let session_cloned = session.clone();
    let upload_file_thread: JoinHandle<u64> =
        thread::spawn(move || session_cloned.upload_file(&file_path_cloned).unwrap());

    // 4. 等待获取作业列表完成
    begin!("获取作业列表");
    let homework_list = get_homework_list_thread.join().unwrap();
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
    if std::io::stdin().read_line(&mut comment).is_err() {
        return Err(anyhow::anyhow!("读取备注失败"));
    }
    comment = comment.trim().to_string();

    // 7. 等待上传文件完成
    begin!("上传文件到资料库");
    let upload_file_id = upload_file_thread.join().unwrap();
    if upload_file_id == 0 {
        return Err(anyhow::anyhow!("上传文件到资料库失败"));
    }
    end!("上传文件到资料库");

    // 8. 发送上交作业请求
    begin!("上交作业");
    session
        .handin_homework(selected_homework.id, upload_file_id, comment)
        .unwrap();
    end!("上交作业");

    Ok(())
}

pub fn upgrade_core(config: &utils::Config, session: &network::Session) -> Result<()> {
    let semester_map = try_or_throw!(session.get_semester_map(), "获取学期映射表");

    let course_list = try_or_throw!(session.get_course_list(), "获取课程列表");

    let semester_course_map = network::Session::to_semester_course_map(course_list, semester_map);
    try_or_throw!(
        network::Session::store_semester_course_map(&config.courses, &semester_course_map),
        "存储 学期->课程 映射表"
    );

    let active_courses = network::Session::filter_active_courses(&semester_course_map);

    try_or_throw!(
        network::Session::store_active_courses(&config.active_courses, &active_courses),
        "存储活跃课程列表"
    );

    Ok(())
}

fn config_help() {
    println!("add-account | a：添加一个账户");
    println!("remove-account | r：删除一个账户");
    println!("user-default | u：设置默认用户");
    println!("storage-dir | s：设置存储路径");
    println!("mp4-trashed | m：设置是否跳过下载 mp4 文件");
    println!("pdf-or-ppt | p：设置是否将 ppt 下载为 pdf");
    println!("list-config | l：查看所有的配置");
    println!("help | h：显示此帮助");
    println!("Ctrl + C：退出配置模式\n");
}

/// 在 config 前，保证已经有了默认账号
/// 为了保证稳定性，任何切换默认账号的行为都要求重新刷新课程表
/// 这样的好处是不用返回线程了，可以直接返回新账号的会话
/// 而且如果想要使用后续功能，重新刷新课程表的操作是必要的
pub fn config_core(
    config: &utils::Config,
    settings: &mut utils::Settings,
    accounts: &mut HashMap<String, account::Account>,
) -> Result<Option<network::Session>> {
    config_help();
    let mut new_session_wrapper = None;
    let prompt = format!("{} > ", "(config)".blue());
    loop {
        let mut rl = completer::ConfigEditor::build();
        match rl.readline(&prompt) {
            Ok(cmd) => match cmd.as_str() {
                "add-account" | "a" => {
                    let new_default_account = try_or_throw!(
                        account::Account::add_account(
                            &config.accounts,
                            &config.settings,
                            accounts,
                            settings,
                        ),
                        "添加用户"
                    );
                    new_session_wrapper = Some(check_up::after_change_default_account(
                        config,
                        &new_default_account,
                    ));
                }
                "remove-account" | "r" => {
                    let users: Vec<String> = accounts.keys().cloned().collect();

                    match Select::with_theme(&ColorfulTheme::default())
                        .with_prompt(SELECT_PROMPT)
                        .items(&users)
                        .default(0)
                        .interact_opt()
                    {
                        Ok(Some(index)) => {
                            let user_to_delete = &users[index];

                            match account::Account::remove_account(
                                &config.accounts,
                                &config.settings,
                                accounts,
                                settings,
                                user_to_delete,
                            ) {
                                Some(new_default_account) => {
                                    new_session_wrapper =
                                        Some(check_up::after_change_default_account(
                                            config,
                                            &new_default_account,
                                        ));
                                }
                                None => {}
                            }
                        }
                        _ => {
                            warning!("取消删除账号");
                            continue;
                        }
                    }
                }
                "user-default" | "u" => {
                    let users: Vec<String> = accounts.keys().cloned().collect();
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

                            try_or_throw!(
                                utils::Settings::set_default_user(
                                    settings,
                                    &config.settings,
                                    &user_to_set,
                                ),
                                "设置默认用户"
                            );
                            let new_default_account = try_or_throw!(
                                account::Account::get_default_account(accounts, &settings.user),
                                "获取默认账号"
                            );
                            new_session_wrapper = Some(check_up::after_change_default_account(
                                config,
                                &new_default_account,
                            ));
                        }
                        _ => {
                            warning!("取消设置默认账号");
                            continue;
                        }
                    }
                }
                "storage_dir" | "s" => {
                    let storage_dir = completer::readin_storage_dir();
                    try_or_throw!(
                        utils::Settings::set_storage_dir(settings, &config.settings, &storage_dir),
                        "设置存储目录"
                    );
                }
                "mp4-trashed" | "m" => {
                    print!("是否跳过下载 mp4 文件？(y/n)");
                    std::io::stdout().flush().unwrap();
                    let mut is_pdf = String::new();
                    if std::io::stdin().read_line(&mut is_pdf).is_err() {
                        error!("读取指令失败");
                        continue;
                    }
                    match is_pdf.trim() {
                        "y" => {
                            try_or_throw!(
                                utils::Settings::set_mp4_trashed(settings, &config.settings, true),
                                "设置是否跳过下载 mp4 文件"
                            );
                        }
                        "n" => {
                            try_or_throw!(
                                utils::Settings::set_mp4_trashed(settings, &config.settings, false),
                                "设置是否跳过下载 mp4 文件"
                            );
                        }
                        _ => {
                            warning!("输入无效");
                        }
                    }
                }
                "pdf-or-ppt" | "p" => {
                    print!("是否将 ppt 下载为 pdf？(y/n)");
                    std::io::stdout().flush().unwrap();
                    let mut is_pdf = String::new();
                    if std::io::stdin().read_line(&mut is_pdf).is_err() {
                        error!("读取指令失败");
                        continue;
                    }
                    match is_pdf.trim() {
                        "y" => {
                            try_or_throw!(
                                utils::Settings::set_is_pdf(settings, &config.settings, true),
                                "设置下载 ppt 格式"
                            );
                        }
                        "n" => {
                            try_or_throw!(
                                utils::Settings::set_is_pdf(settings, &config.settings, false),
                                "设置下载 ppt 格式"
                            );
                        }
                        _ => {
                            warning!("输入无效");
                        }
                    }
                }
                "list-config" | "l" => {
                    try_or_throw!(settings.list(), "查看配置");
                }
                "help" | "h" | "" => {
                    config_help();
                }
                _=>{
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
                error!("输入错误：{}", e);
            }
        }
    }

    Ok(new_session_wrapper)
}

/// 在 which 前，保证已经有了默认账号、课程列表
/// 选择课程
/// 允许啥课程都不选
pub fn which_core(config: &utils::Config) -> Result<()> {
    let semester_course_map = try_or_throw!(
        network::Session::load_semester_course_map(&config.courses),
        "加载 学期->课程 映射表"
    );

    let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();

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
        network::Session::store_selected_courses(&config.selected_courses, &selected_courses),
        "存储已选课程"
    );
    Ok(())
}

pub fn task_core(config: &utils::Config,session: &network::Session)->Result<()>{
    begin!("获取作业列表");
    let homework_list = try_or_throw!(session.get_homework_list(&config.courses),"获取作业列表");
    end!("获取作业列表");

    for homework in homework_list{
        println!("  {}",homework.name);
    }
    Ok(())
}

pub fn grade_core(config: &utils::Config, account: &account::Account, session: &network::Session) -> Result<()> {
    try_or_throw!(session.get_grade(&config.courses,&account), "获取成绩列表");
    Ok(())
}
