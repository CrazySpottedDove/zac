use crate::{account, network};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

/// 成功信息打印
#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{} {}","✓".green() ,format!($($arg)*));
    })
}

/// 错误信息打印
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        use colored::*;
        eprintln!("{} {}","✗".red() ,format!($($arg)*));
    })
}

/// 警告信息打印
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{} {}","!".yellow() ,format!($($arg)*));
    })
}

/// 进程信息打印
#[macro_export]
macro_rules! process {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{} {}","⚙".blue() ,format!($($arg)*));
    })
}

/// 成功返回值，失败报 error
#[macro_export]
macro_rules! try_or_log {
    ($expr:expr, $err_msg:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                error!("{}：{}", $err_msg, e);
                return;
            }
        }
    };
}

/// 获取配置文件路径!
fn get_config_path() -> Result<PathBuf>{
    use std::env::var;

    #[cfg(target_os = "linux")]
    let home_dir = var("HOME")?;

    #[cfg(target_os = "windows")]
    let home_dir = var("USERPROFILE")?;

    let config_path = PathBuf::from(home_dir).join(".zac");
    if !config_path.exists() {
        fs::create_dir(&config_path)?;
    }

    #[cfg(debug_assertions)]
    success!("✓ 配置文件定位 -> {}", config_path.display());

    Ok(config_path)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub user: String,
    pub storage_dir: PathBuf,
    pub is_pdf: bool,
}

impl Settings {
    /// 读取设置文件!
    pub fn load(path_settings: &PathBuf) -> Result<Settings> {
        let data = fs::read_to_string(path_settings)?;
        let settings: Settings = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("读取设置文件");

        Ok(settings)
    }

    /// 设置默认用户!
    pub fn set_default_user(
        &mut self,
        path_settings: &PathBuf,
        user: &str,
    ) -> Result<()> {
        self.user = user.into();

        let json = serde_json::to_string(self)?;
        fs::write(path_settings, json)?;

        success!(
            "默认用户修改为 {} -> {}",
            self.user,
            path_settings.display()
        );

        Ok(())
    }

    /// 设置存储目录!
    pub fn set_storage_dir(
        &mut self,
        path_settings: &PathBuf,
        storage_dir: &str,
    ) -> Result<()> {
        self.storage_dir = PathBuf::from(storage_dir);

        let json = serde_json::to_string(self)?;
        fs::write(path_settings, json)?;

        success!(
            "存储目录修改为 {} -> {}",
            self.storage_dir.display(),
            path_settings.display()
        );

        Ok(())
    }

    /// 设置下载 ppt 文件格式!
    pub fn set_is_pdf(
        &mut self,
        path_settings: &PathBuf,
        is_pdf: bool,
    ) -> Result<()> {
        self.is_pdf = is_pdf;

        let json = serde_json::to_string(self)?;
        fs::write(path_settings, json)?;

        success!(
            "下载 ppt 文件格式修改为 {} -> {}",
            if is_pdf { "PDF" } else { "PPT" },
            path_settings.display()
        );

        Ok(())
    }
}

pub struct Config {
    pub accounts: PathBuf,
    pub settings: PathBuf,
    pub courses: PathBuf,
    pub selected_courses: PathBuf,
    pub activity_upload_record: PathBuf,
    // pub cookies: PathBuf,
}

impl Config {
    pub fn init() -> Result<Config> {
        let config_path = get_config_path()?;

        let accounts = config_path.join("accounts.json");
        if !accounts.exists() {
            Config::accounts_init(&accounts)?;
        }

        let settings = config_path.join("settings.json");
        if !settings.exists() {
            Config::settings_init(&settings)?;
        }

        let courses = config_path.join("courses.json");
        if !courses.exists() {
            Config::courses_init(&courses)?;
        }

        let selected_courses = config_path.join("selected_courses.json");
        if !selected_courses.exists() {
            Config::selected_courses_init(&selected_courses)?;
        }

        let activity_upload_record = config_path.join("activity_upload_record.json");
        if !activity_upload_record.exists() {
            Config::activity_upload_record_init(&activity_upload_record)?;
        }
        // let cookies = config_path.join("cookies.json");

        Ok(Config {
            accounts,
            settings,
            courses,
            selected_courses,
            activity_upload_record,
        })
    }

    /// 初始化账号文件!
    fn accounts_init(path_accounts: &PathBuf) -> Result<()> {
        let accounts = HashMap::<String, account::Account>::new();
        let json = serde_json::to_string(&accounts)?;
        fs::write(path_accounts, json)?;

        success!("账号初始化文件 -> {}", path_accounts.display());
        Ok(())
    }

    /// 初始化设置文件!
    fn settings_init(path_settings: &PathBuf) -> Result<()> {
        let settings = Settings {
            user: String::new(),
            storage_dir: PathBuf::from(""),
            is_pdf: false,
        };
        let json = serde_json::to_string(&settings)?;
        fs::write(path_settings, json)?;

        success!("初始化设置文件 -> {}", path_settings.display());
        Ok(())
    }

    /// 初始化课程列表文件!
    fn courses_init(path_courses: &PathBuf) -> Result<()> {
        let courses: HashMap<u32, String> = HashMap::new();
        let json = serde_json::to_string(&courses)?;
        fs::write(path_courses, json)?;

        success!("初始化课程列表文件 -> {}", path_courses.display());
        Ok(())
    }

    /// 初始化已选课程文件!
    fn selected_courses_init(path_selected_courses: &PathBuf) -> Result<()> {
        let selected_courses: Vec<network::CourseFull> = Vec::new();
        let json = serde_json::to_string(&selected_courses)?;
        fs::write(path_selected_courses, json)?;

        success!(
            "初始化已选课程文件 -> {}",
            path_selected_courses.display()
        );
        Ok(())
    }

    /// 初始化课件记录文件!
    fn activity_upload_record_init(path_activity_upload_record: &PathBuf) -> Result<()> {
        let activity_upload_record: Vec<u64> = Vec::new();
        let json = serde_json::to_string(&activity_upload_record)?;
        fs::write(path_activity_upload_record, json)?;

        success!(
            "已初始化课件记录文件 -> {}",
            path_activity_upload_record.display()
        );
        Ok(())
    }
}
