use crate::success;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
pub const SELECT_PROMPT: &str = "↑/↓ 选择 | Enter 确认 | Esc 退出";
pub const MULTISELECT_PROMPT: &str = "↑/↓ 选择 | Space 选中 | Enter 确认 | Esc 退出";
pub const MAX_RETRIES: u64 = 3;

/// 获取配置文件路径!
pub fn get_config_path() -> Result<PathBuf> {
    use std::env::var;

    #[cfg(target_os = "linux")]
    let home_dir = var("HOME")?;

    #[cfg(target_os = "windows")]
    let home_dir = var("USERPROFILE")?;

    #[cfg(target_os = "macos")]
    let home_dir = var("HOME")?;

    let config_path = PathBuf::from(home_dir).join(".zac");
    if !config_path.exists() {
        fs::create_dir(&config_path)?;
    }

    #[cfg(debug_assertions)]
    success!("配置文件定位 -> {}", config_path.display());

    Ok(config_path)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub user: String,
    pub storage_dir: PathBuf,
    pub is_pdf: bool,
    pub mp4_trashed: bool,
    pub path_settings: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        Settings::default()
    }
}
impl Settings {
    fn default() -> Self {
        Settings {
            user: String::new(),
            storage_dir: PathBuf::from(""),
            is_pdf: false,
            mp4_trashed: false,
            path_settings: get_config_path().unwrap().join("settings.json"),
        }
    }
    /// 读取配置文件!
    pub fn load(path_settings: PathBuf) -> Result<Settings> {
        let data = fs::read_to_string(path_settings)?;
        let settings: Settings = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("读取配置文件");

        Ok(settings)
    }

    /// 设置默认用户!
    pub fn set_default_user(&mut self, user: &str) -> Result<()> {
        self.user = user.into();

        let json = serde_json::to_string(self)?;
        fs::write(&self.path_settings, json)?;

        success!(
            "默认用户修改为 {} -> {}",
            self.user,
            &self.path_settings.display()
        );

        Ok(())
    }

    /// 设置存储目录
    /// 保证输入的路径是存在的目录
    pub fn set_storage_dir(&mut self, storage_dir: &str) -> Result<()> {
        let path = PathBuf::from(storage_dir);

        self.storage_dir = path;

        let json = serde_json::to_string(self)?;
        fs::write(&self.path_settings, json)?;

        success!(
            "存储目录修改为 {} -> {}",
            self.storage_dir.display(),
            &self.path_settings.display()
        );

        Ok(())
    }

    /// 设置下载 ppt 文件格式!
    pub fn set_is_pdf(&mut self, is_pdf: bool) -> Result<()> {
        self.is_pdf = is_pdf;

        let json = serde_json::to_string(self)?;
        fs::write(&self.path_settings, json)?;

        success!(
            "下载 ppt 文件格式修改为 {} -> {}",
            if is_pdf { "PDF" } else { "PPT" },
            &self.path_settings.display()
        );

        Ok(())
    }

    pub fn set_mp4_trashed(&mut self, mp4_trashed: bool) -> Result<()> {
        self.mp4_trashed = mp4_trashed;
        let json = serde_json::to_string(self)?;
        fs::write(&self.path_settings, json)?;

        success!("跳过下载 mp4 文件：{}", mp4_trashed);

        Ok(())
    }

    pub fn list(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        println!("{}", json);
        Ok(())
    }
}

pub struct Config {}

impl Config {
    pub fn init() -> Result<(
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
    )> {
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

        let cookies = config_path.join("cookies.json");
        if !cookies.exists() {
            Config::cookies_init(&cookies)?;
        }

        let active_courses = config_path.join("active_courses.json");
        if !active_courses.exists() {
            Config::active_courses_init(&active_courses)?;
        }

        Ok((
            accounts,
            settings,
            courses,
            selected_courses,
            activity_upload_record,
            cookies,
            active_courses,
        ))
    }

    /// 初始化账号文件!
    fn accounts_init(path_accounts: &PathBuf) -> Result<()> {
        fs::write(path_accounts, "{}")?;

        success!("账号初始化文件 -> {}", path_accounts.display());
        Ok(())
    }

    /// 初始化设置文件!
    fn settings_init(path_settings: &PathBuf) -> Result<()> {
        let settings = Settings::default();
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
        fs::write(path_selected_courses, "[]")?;

        success!("初始化已选课程文件 -> {}", path_selected_courses.display());
        Ok(())
    }

    /// 初始化课件记录文件!
    fn activity_upload_record_init(path_activity_upload_record: &PathBuf) -> Result<()> {
        fs::write(path_activity_upload_record, "[]")?;
        success!(
            "已初始化课件记录文件 -> {}",
            path_activity_upload_record.display()
        );
        Ok(())
    }

    /// 初始化 cookies 文件！
    fn cookies_init(cookies: &PathBuf) -> Result<()> {
        fs::write(cookies, "{}")?;
        success!("初始化 cookies 文件 -> {}", cookies.display());
        Ok(())
    }

    /// 初始化 active_courses 文件！
    fn active_courses_init(active_courses: &PathBuf) -> Result<()> {
        fs::write(active_courses, "[]")?;
        success!("初始化 active_courses 文件 -> {}", active_courses.display());
        Ok(())
    }
}
