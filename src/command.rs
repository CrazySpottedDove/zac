use crate::{account, error, network, process, success, try_or_log, utils, warning};
use dialoguer::{theme::ColorfulTheme, MultiSelect, Select};
use std::io::Write;
use std::path::PathBuf;

pub fn fetch() {
    process!("FETCH");
    let config = try_or_log!(utils::Config::init(), "初始化配置文件");

    let mut settings = try_or_log!(utils::Settings::load(&config.settings), "获取设置");

    // 处理没设置存储目录的情况
    if settings.storage_dir == PathBuf::new() {
        warning!("未设置存储目录 => 设置存储目录");
        print!("请输入存储目录：");
        std::io::stdout().flush().unwrap();
        let mut storage_dir = String::new();

        try_or_log!(std::io::stdin().read_line(&mut storage_dir), "读取存储目录");

        try_or_log!(
            utils::Settings::set_storage_dir(&mut settings, &config.settings, &storage_dir.trim()),
            "设置存储目录"
        );
    }

    let mut accounts = try_or_log!(
        account::Account::get_accounts(&config.accounts),
        "获取已知账号"
    );

    // 处理没有已知账号的情况
    if accounts.is_empty() {
        warning!("未发现已知的账号 => 创建账号");
        try_or_log!(
            account::Account::add_account(
                &config.accounts,
                &config.settings,
                &mut accounts,
                &mut settings,
            ),
            "添加用户"
        );
    }

    let default_account = try_or_log!(
        account::Account::get_default_account(&accounts, &settings.user),
        "获取默认账号"
    );

    let session = try_or_log!(network::Session::try_new(), "创建会话");

    try_or_log!(session.login(&default_account), "登录");

    // 加载已经选择的课程
    let mut selected_courses = try_or_log!(
        network::Session::load_selected_courses(&config.selected_courses),
        "加载已选课程"
    );

    // 处理没有已选课程的情况
    if selected_courses.is_empty() {
        warning!("还没有已经选择的课程！");
        let mut semester_course_map = try_or_log!(
            network::Session::load_semester_course_map(&config.courses),
            "加载课程列表"
        );

        let mut semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
        if semester_list.is_empty() {
            warning!("未发现课程 => 获取课程信息");
            let semester_map = try_or_log!(session.get_semester_map(), "获取学期列表");

            let course_list = try_or_log!(session.get_course_list(), "获取课程列表");

            try_or_log!(
                network::Session::store_semester_course_map(
                    &config.courses,
                    course_list,
                    semester_map,
                ),
                "存储 学期->课程 映射表"
            );

            semester_course_map = try_or_log!(
                network::Session::load_semester_course_map(&config.courses),
                "加载课程列表"
            );
            semester_list = semester_course_map.keys().cloned().collect();
        }
        selected_courses = Vec::new();
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
                            println!("取消选择课程");
                            continue;
                        }
                    }
                }
            }
            _ => {
                println!("取消选择学期");
            }
        }

        try_or_log!(
            network::Session::store_selected_courses(&config.selected_courses, &selected_courses),
            "存储已选课程"
        );
    }

    let activity_upload_record = try_or_log!(
        network::Session::load_activity_upload_record(&config.activity_upload_record),
        "加载已下载课件记录"
    );

    try_or_log!(
        network::Session::fetch_activity_uploads(
            &session,
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

pub fn upgrade() {
    let config = try_or_log!(utils::Config::init(), "初始化配置文件");

    let mut settings = try_or_log!(utils::Settings::load(&config.settings), "获取设置");

    let mut accounts = try_or_log!(
        account::Account::get_accounts(&config.accounts),
        "获取已知账号"
    );

    // 处理无已知账号情况
    if accounts.is_empty() {
        warning!("未发现已知的账号 => 创建账号");
        try_or_log!(
            account::Account::add_account(
                &config.accounts,
                &config.settings,
                &mut accounts,
                &mut settings,
            ),
            "添加用户"
        );
    }

    let default_account = try_or_log!(
        account::Account::get_default_account(&accounts, &settings.user),
        "获取默认账号"
    );

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
    println!("select-courses | sc：设置目标课程");
    println!("help | h  显示此帮助");
    println!("exit | q  退出配置模式\n");
}

pub fn config() {
    process!("CONFIG");

    let config = try_or_log!(utils::Config::init(), "初始化配置文件");

    let mut settings = try_or_log!(utils::Settings::load(&config.settings), "获取设置");

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
                let mut accounts = try_or_log!(
                    account::Account::get_accounts(&config.accounts),
                    "获取已知账号"
                );

                try_or_log!(
                    account::Account::add_account(
                        &config.accounts,
                        &config.settings,
                        &mut accounts,
                        &mut settings,
                    ),
                    "添加用户"
                );
            }
            "remove-account" | "rm" => {
                let mut accounts = try_or_log!(
                    account::Account::get_accounts(&config.accounts),
                    "获取已知账号"
                );

                let users: Vec<String> = accounts.keys().cloned().collect();
                if users.is_empty() {
                    warning!("没有账号可以删除");
                    continue;
                }

                match Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("删除账号 | Esc 退出")
                    .items(&users)
                    .default(0)
                    .interact_opt()
                {
                    Ok(Some(index)) => {
                        let user_to_delete = &users[index];

                        try_or_log!(
                            account::Account::remove_account(
                                &config.accounts,
                                &config.settings,
                                &mut accounts,
                                &mut settings,
                                user_to_delete,
                            ),
                            "删除用户"
                        );
                    }
                    _ => {
                        println!("取消删除账号");
                        continue;
                    }
                }
            }
            "set-default-user" | "set" => {
                let accounts = try_or_log!(
                    account::Account::get_accounts(&config.accounts),
                    "获取已知账号"
                );

                let users: Vec<String> = accounts.keys().cloned().collect();
                if users.is_empty() {
                    warning!("没有账号可以选择");
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

                        try_or_log!(
                            utils::Settings::set_default_user(
                                &mut settings,
                                &config.settings,
                                &user_to_set,
                            ),
                            "设置默认用户"
                        );
                    }
                    _ => {
                        println!("取消设置默认账号");
                        continue;
                    }
                }
            }
            "select_courses" | "sc" => {
                let mut semester_course_map = try_or_log!(
                    network::Session::load_semester_course_map(&config.courses),
                    "加载课程列表"
                );

                let mut semester_list: Vec<String> = semester_course_map.keys().cloned().collect();

                // 处理没有课程的情况
                if semester_list.is_empty() {
                    warning!("未发现课程 => 获取课程信息");

                    let mut accounts = try_or_log!(
                        account::Account::get_accounts(&config.accounts),
                        "获取已知账号"
                    );

                    // 处理没有已知账号的情况
                    if accounts.is_empty() {
                        warning!("未发现已知的账号 => 创建账号");
                        try_or_log!(
                            account::Account::add_account(
                                &config.accounts,
                                &config.settings,
                                &mut accounts,
                                &mut settings,
                            ),
                            "添加用户"
                        );
                    }

                    let default_account = try_or_log!(
                        account::Account::get_default_account(&accounts, &settings.user),
                        "获取默认账号"
                    );

                    let session = try_or_log!(network::Session::try_new(), "创建会话");
                    try_or_log!(session.login(&default_account), "登录");
                    let semester_map = try_or_log!(session.get_semester_map(), "获取学期列表");

                    let course_list = try_or_log!(session.get_course_list(), "获取课程列表");

                    try_or_log!(
                        network::Session::store_semester_course_map(
                            &config.courses,
                            course_list,
                            semester_map,
                        ),
                        "存储 学期->课程 映射表"
                    );

                    semester_course_map = try_or_log!(
                        network::Session::load_semester_course_map(&config.courses),
                        "加载课程列表"
                    );
                    semester_list = semester_course_map.keys().cloned().collect();
                }

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
                                    println!("取消选择课程");
                                    continue;
                                }
                            }
                        }
                    }
                    _ => {
                        println!("取消选择学期");
                    }
                }

                try_or_log!(
                    network::Session::store_selected_courses(
                        &config.selected_courses,
                        &selected_courses
                    ),
                    "存储已选课程"
                );
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
                            utils::Settings::set_is_pdf(&mut settings, &config.settings, true),
                            "设置下载 ppt 格式"
                        );
                    }
                    "n" => {
                        try_or_log!(
                            utils::Settings::set_is_pdf(&mut settings, &config.settings, false),
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
