use crate::{account, check_up, error, network, process, success, try_or_log, utils, warning};
use dialoguer::{theme::ColorfulTheme,Select,MultiSelect};
use std::collections::HashMap;
use std::io::Write;
pub fn fetch(config: &utils::Config, settings: &utils::Settings, default_account: &account::Account) {
    process!("FETCH");

    let session = try_or_log!(network::Session::try_new(), "创建会话");

    try_or_log!(session.login(&default_account), "登录");

    let selected_courses = try_or_log!(
        network::Session::load_selected_courses(&config.selected_courses),
        "加载已选课程"
    );

    // 没有已选课程，就提示用户选课
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        warning!("请运行 (zac | zacpb) (--which | -w) 选择课程！");
        return;
    }

    let activity_upload_record = try_or_log!(
        network::Session::load_activity_upload_record(&config.activity_upload_record),
        "加载已下载课件记录"
    );

    try_or_log!(
        session.fetch_activity_uploads(
            &settings.storage_dir,
            &config.activity_upload_record,
            selected_courses,
            activity_upload_record,
            settings.is_pdf,
        ),
        "拉取新课件"
    );

    success!("FETCH");
}

pub fn submit() {
    process!("SUBMIT");
    success!("SUBMIT");
}

pub fn upgrade(config: &utils::Config, default_account: &account::Account) {
    process!("UPGRADE");
    let session = try_or_log!(network::Session::try_new(), "创建会话");

    try_or_log!(session.login(&default_account), "登录");

    let semester_map = try_or_log!(session.get_semester_map(), "获取学期映射表");

    let course_list = try_or_log!(session.get_course_list(), "获取课程列表");

    try_or_log!(
        network::Session::store_semester_course_map(&config.courses, course_list, semester_map),
        "存储 学期->课程 映射表"
    );

    success!("UPGRADE");
}

fn config_help() {
    println!("可输入的命令：");
    println!("add-account | a：添加一个账户");
    println!("remove-account | rm：删除一个账户");
    println!("set-default-user | set：设置默认用户");
    println!("set-is-pdf | pdf：设置是否将 ppt 下载为 pdf");
    println!("help | h  显示此帮助");
    println!("exit | q  退出配置模式\n");
}

pub fn config(
    config: &utils::Config,
    settings: &mut utils::Settings,
    accounts: &mut HashMap<String, account::Account>,
) {
    process!("CONFIG");

    config_help();

    // 2) 进入交互循环
    loop {
        print!("(config) > ");
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            error!("读取指令失败");
            continue;
        }

        let cmd = input.trim();

        match cmd {
            "exit" | "q" => {
                success!("退出配置模式");
                break;
            }
            "add-account" | "a" => {
                let new_default_account = try_or_log!(
                    account::Account::add_account(
                        &config.accounts,
                        &config.settings,
                        accounts,
                        settings,
                    ),
                    "添加用户"
                );
                check_up::after_change_default_account(config, &new_default_account);
            }
            "remove-account" | "rm" => {
                let users: Vec<String> = accounts.keys().cloned().collect();

                match Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("删除账号 | Esc 退出")
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
                                check_up::after_change_default_account(config, &new_default_account);
                            }
                            None => {}
                        }
                    }
                    _ => {
                        println!("取消删除账号");
                        continue;
                    }
                }
            }
            "set-default-user" | "set" => {
                let users: Vec<String> = accounts.keys().cloned().collect();
                if users.len() == 1 {
                    warning!("只有一个账号 {}", users[0]);
                    continue;
                }

                match Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("设置默认账号 | Esc 退出")
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

                        try_or_log!(
                            utils::Settings::set_default_user(
                                settings,
                                &config.settings,
                                &user_to_set,
                            ),
                            "设置默认用户"
                        );

                        let new_default_account = try_or_log!(
                            account::Account::get_default_account(accounts, &settings.user),
                            "获取默认账号"
                        );
                        check_up::after_change_default_account(config, &new_default_account);
                    }
                    _ => {
                        println!("取消设置默认账号");
                        continue;
                    }
                }
            }

            "set-is-pdf" | "pdf" => {
                print!("是否将 ppt 下载为 pdf？(y/n)");
                std::io::stdout().flush().unwrap();
                let mut is_pdf = String::new();
                if std::io::stdin().read_line(&mut is_pdf).is_err() {
                    error!("读取指令失败");
                    continue;
                }
                match is_pdf.trim() {
                    "y" => {
                        try_or_log!(
                            utils::Settings::set_is_pdf( settings, &config.settings, true),
                            "设置下载 ppt 格式"
                        );
                    }
                    "n" => {
                        try_or_log!(
                            utils::Settings::set_is_pdf(settings, &config.settings, false),
                            "设置下载 ppt 格式"
                        );
                    }
                    _ => {
                        warning!("输入无效");
                    }
                }
            }
            "" => { /* 空输入，忽略 */ }
            "help" | "h" => {
                config_help();
            }
            _ => {
                println!("未知命令: {cmd}");
            }
        }
    }
    success!("CONFIG");
}


/// 选择课程
/// 允许啥课程都不选
pub fn which(
    config: &utils::Config
) {
    process!("WHICH");
    let semester_course_map = try_or_log!(
        network::Session::load_semester_course_map(&config.courses),
        "加载 学期->课程 映射表"
    );

    let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();

    let mut selected_courses = Vec::new();
    match MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("选择学期 | Esc 退出")
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
                    .with_prompt("选择课程 | Esc 退出")
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
            return;
        }
    }
    try_or_log!(
        network::Session::store_selected_courses(&config.selected_courses, &selected_courses),
        "存储已选课程"
    );
    success!("WHICH");
}
