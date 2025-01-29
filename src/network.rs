use crate::{
    account, begin, end, error, process, success, try_or_exit, try_or_throw, utils, waiting,
    warning,
};
use ::serde::{Deserialize, Serialize};
use anyhow::anyhow;
use anyhow::Result;
use colored::ColoredString;
use colored::Colorize;
use cookie_store::CookieStore;
use num::ToPrimitive;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

const LOGIN_URL: &str = "https://zjuam.zju.edu.cn/cas/login";
const PUBKEY_URL: &str = "https://zjuam.zju.edu.cn/cas/v2/getPubKey";
const HOME_URL: &str = "https://courses.zju.edu.cn";
const GRADE_SERVICE_URL: &str = "http://appservice.zju.edu.cn/zdjw/cjcx/cjcxjg";
const GRADE_URL: &str = "http://appservice.zju.edu.cn/zju-smartcampus/zdydjw/api/kkqk_cxXscjxx";
#[cfg(feature = "pb")]
use {
    colored::*,
    indicatif::{MultiProgress, ProgressBar, ProgressStyle},
    std::io::Write,
};

fn rsa_no_padding(src: &str, modulus: &str, exponent: &str) -> String {
    let m = num::BigUint::parse_bytes(modulus.as_bytes(), 16).unwrap();
    let e = num::BigUint::parse_bytes(exponent.as_bytes(), 16).unwrap();

    let input_nr = num::BigUint::from_bytes_be(src.as_bytes());

    let crypt_nr = input_nr.modpow(&e, &m);

    crypt_nr
        .to_bytes_be()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

// #[derive(Debug)]
struct State {
    cookie_store: Arc<CookieStoreMutex>,
    path_cookies: PathBuf,
}

impl State {
    /// 建立新的 cookie_store
    pub fn try_new(path_cookies: PathBuf) -> anyhow::Result<State> {
        #[allow(deprecated)]
        let cookie_store = match File::open(&path_cookies) {
            Ok(file) => match CookieStore::load_json(std::io::BufReader::new(file)) {
                Ok(cookie_store) => cookie_store,
                Err(_) => CookieStore::default(),
            },
            Err(_) => {
                File::create(&path_cookies)?;
                CookieStore::default()
            }
        };
        let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));
        Ok(State {
            cookie_store,
            path_cookies,
        })
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path_cookies)
        {
            Ok(f) => f,
            Err(e) => {
                error!(
                    "open {} for write failed. error: {}",
                    self.path_cookies.display(),
                    e
                );
                return;
            }
        };

        let store = self.cookie_store.lock().unwrap();
        #[allow(deprecated)]
        if let Err(e) = store.save_json(&mut file) {
            error!(
                "save cookies to path {} failed. error: {}",
                &self.path_cookies.display(),
                e
            );
        }
    }
}

#[derive(Clone)]
pub struct Session {
    #[allow(dead_code)] // just make clippy happy
    state: Arc<State>,
    client: Client,
}

impl Session {
    /// 建立新的会话!
    pub fn try_new(path_cookies: PathBuf) -> Result<Session> {
        let state = State::try_new(path_cookies)?;
        let state = Arc::new(state);
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (X11; Linux x86_64; rv:88.0) Gecko/20100101 Firefox/88.0"
                .parse()
                .unwrap(),
        );

        let client = Client::builder()
            .cookie_provider(state.cookie_store.clone())
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(1200))
            .build()?;

        #[cfg(debug_assertions)]
        success!("建立会话");

        Ok(Session { state, client })
    }
    fn login_core(&self, account: &account::Account) -> Result<()> {
        let (execution, (modulus, exponent)) = rayon::join(
            || {
                let res = try_or_exit!(self.client.get(LOGIN_URL).send(), "连接登录页");
                let text = res.text().unwrap();
                let re =
                    regex::Regex::new(r#"<input type="hidden" name="execution" value="(.*?)" />"#)
                        .unwrap();
                let execution = re
                    .captures(&text)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str()))
                    .ok_or(anyhow!("Execution value not found"))
                    .unwrap()
                    .to_string();
                execution
            },
            || {
                let res = try_or_exit!(self.client.get(PUBKEY_URL).send(), "获取公钥");
                let json: Value = try_or_exit!(res.json(), "解析公钥");
                let modulus = json["modulus"]
                    .as_str()
                    .ok_or(anyhow!("Modulus not found"))
                    .unwrap()
                    .to_string();
                let exponent = json["exponent"]
                    .as_str()
                    .ok_or(anyhow!("Exponent not found"))
                    .unwrap()
                    .to_string();
                (modulus, exponent)
            },
        );

        let rsapwd = rsa_no_padding(&account.password, &modulus, &exponent);

        let params = [
            ("username", account.stuid.as_str()),
            ("password", &rsapwd),
            ("execution", &execution),
            ("_eventId", "submit"),
            ("authcode", ""),
            ("rememberMe", "true"),
        ];
        let res = try_or_throw!(self.client.post(LOGIN_URL).form(&params).send(), "提交登录");
        #[cfg(debug_assertions)]
        println!("{:?}", res);
        if res.url().to_string() == "https://zjuam.zju.edu.cn/cas/login" {
            return Err(anyhow!("请检查学号-密码正确性及你的网络连接状态"));
        }

        rayon::join(
            || {
                try_or_exit!(self.client.get(HOME_URL).send(), "连接雪灾浙大主页");
            },
            || {
                try_or_exit!(
                    self.client.get(GRADE_SERVICE_URL).send(),
                    "连接成绩查询主页"
                );
            },
        );
        Ok(())
    }

    /// 登录!
    pub fn login(&self, account: &account::Account) -> Result<()> {
        let (zcourse_query_wrapper, zgrade_query_wrapper) = rayon::join(
            || {
                let res = try_or_exit!(self.client.get(HOME_URL).send(), "连接雪灾浙大主页");
                res.url().query().map(|q| q.to_owned())
            },
            || {
                let res = try_or_exit!(
                    self.client.get(GRADE_SERVICE_URL).send(),
                    "连接成绩查询主页"
                );
                res.url().query().map(|q| q.to_owned())
            },
        );
        if zcourse_query_wrapper.is_none() && zgrade_query_wrapper.is_none() {
            return Ok(());
        }
        self.login_core(account)
    }

    /// 重新登录!
    pub fn relogin(&self, account: &account::Account) -> Result<()> {
        self.state.cookie_store.lock().unwrap().clear();
        self.login_core(account)
    }

    /// 获取学期映射表!
    pub fn get_semester_map(&self) -> Result<HashMap<u64, String>> {
        let res = self
            .client
            .get("https://courses.zju.edu.cn/api/my-semesters?")
            .send()?;

        let json: Value = res.json()?;
        let semester_map: Result<HashMap<u64, String>> = json["semesters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let sid = c["id"].as_u64().unwrap();
                let name = c["name"].as_str().unwrap_or_default().to_string();
                Ok((sid, name))
            })
            .collect();

        success!("获取学期映射表");
        semester_map
    }

    /// 获取课程列表!
    pub fn get_course_list(&self) -> Result<Vec<Course>> {
        let res = self.client.get("https://courses.zju.edu.cn/api/my-courses?conditions=%7B%22status%22:%5B%22ongoing%22,%22notStarted%22%5D,%22keyword%22:%22%22,%22classify_type%22:%22recently_started%22,%22display_studio_list%22:false%7D&fields=id,name,semester_id&page=1&page_size=1000").send()?;

        let json: Value = res.json()?;
        let course_list: Vec<Course> = json["courses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| Course {
                id: c["id"].as_u64().unwrap(),
                sid: c["semester_id"].as_u64().unwrap(),
                name: c["name"].as_str().unwrap().to_string(),
            })
            .collect();

        success!("获取课程列表");
        Ok(course_list)
    }

    pub fn to_semester_course_map(
        course_list: Vec<Course>,
        semester_map: HashMap<u64, String>,
    ) -> HashMap<String, Vec<CourseData>> {
        let mut semester_course_map: HashMap<String, Vec<CourseData>> = HashMap::new();
        for course in course_list {
            if let Some(semester_name) = semester_map.get(&course.sid) {
                let course_data = CourseData {
                    id: course.id,
                    name: course.name,
                };
                semester_course_map
                    .entry(semester_name.clone())
                    .or_insert_with(Vec::new)
                    .push(course_data);
            }
        }
        semester_course_map
    }
    /// 存储学期-课程映射表!
    pub fn store_semester_course_map(
        path_courses: &PathBuf,
        semester_course_map: &HashMap<String, Vec<CourseData>>,
    ) -> Result<()> {
        std::fs::write(
            path_courses,
            serde_json::to_string(&semester_course_map).unwrap(),
        )?;

        #[cfg(debug_assertions)]
        success!("存储 学期->课程 映射表");
        Ok(())
    }

    /// 加载学期-课程映射表!
    pub fn load_semester_course_map(
        path_courses: &PathBuf,
    ) -> Result<HashMap<String, Vec<CourseData>>> {
        let data = fs::read_to_string(path_courses)?;
        let semester_course_map: HashMap<String, Vec<CourseData>> = serde_json::from_str(&data)?;

        Ok(semester_course_map)
    }

    /// 存储已选课程!
    pub fn store_selected_courses(
        path_selected_courses: &PathBuf,
        selected_courses: &Vec<CourseFull>,
    ) -> Result<()> {
        std::fs::write(
            path_selected_courses,
            serde_json::to_string(selected_courses)?,
        )?;

        #[cfg(debug_assertions)]
        success!("存储已选课程");
        Ok(())
    }

    /// 加载已选课程!
    pub fn load_selected_courses(path_selected_courses: &PathBuf) -> Result<Vec<CourseFull>> {
        let data = fs::read_to_string(path_selected_courses)?;
        let selected_courses: Vec<CourseFull> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载已选课程");

        Ok(selected_courses)
    }

    /// 存储已下载课件记录!
    pub fn store_activity_upload_record(
        path_activity_upload_record: &PathBuf,
        activity_upload_record: &Vec<u64>,
    ) -> Result<()> {
        std::fs::write(
            path_activity_upload_record,
            serde_json::to_string(activity_upload_record)?,
        )?;

        success!(
            "存储已下载课件记录 -> {}",
            path_activity_upload_record.display()
        );
        Ok(())
    }

    /// 加载已下载课件记录!
    pub fn load_activity_upload_record(path_activity_upload_record: &PathBuf) -> Result<Vec<u64>> {
        let data = fs::read_to_string(path_activity_upload_record)?;
        let activity_upload_record: Vec<u64> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载已下载课件记录");

        Ok(activity_upload_record)
    }

    /// 拉取活动！
    fn fetch_activities(&self, course_id: u64, course_name: &str) -> Result<Vec<Value>> {
        let url = format!(
            "https://courses.zju.edu.cn/api/courses/{}/activities",
            course_id
        );
        for attempt in 1..=utils::MAX_RETRIES {
            match self.client.get(&url).send() {
                Ok(res) => match res.json::<Value>() {
                    Ok(json) => {
                        if let Some(activities) = json["activities"].as_array() {
                            #[cfg(debug_assertions)]
                            success!("{}::activities", course_name.trim());

                            return Ok(activities.clone());
                        } else {
                            println!("{:#?}", json);
                            warning!(
                                "retry {}/{}: {} 的返回 json 无 activities 字段",
                                attempt,
                                utils::MAX_RETRIES,
                                course_name
                            );
                        }
                    }
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        warning!(
                            "retry {}/{}: {} 的返回无法解析为 json: {}",
                            attempt,
                            utils::MAX_RETRIES,
                            course_name,
                            e
                        );
                    }
                },
                Err(e) => {
                    warning!(
                        "retry {}/{}: {} 的请求失败: {}",
                        attempt,
                        utils::MAX_RETRIES,
                        course_name,
                        e
                    );
                }
            }
        }
        Err(anyhow!("{} 的请求失败", course_name))
    }

    /// 拉取下载任务！
    fn fetch_download_tasks(
        &self,
        selected_courses: Vec<CourseFull>,
        activity_upload_record: &Vec<u64>,
        settings: &utils::Settings,
    ) -> Result<Vec<(String, String, u64, String)>> {
        #[cfg(debug_assertions)]
        let start = std::time::Instant::now();

        let num = selected_courses.len();
        let pool = ThreadPoolBuilder::new().num_threads(num).build()?;

        // 使用线程池执行并行操作
        let tasks: Vec<(String, String, u64, String)> = pool.install(|| {
            selected_courses
                .par_iter()
                .filter_map(|selected_course| {
                    // 尝试获取活动列表，如果失败则记录错误并跳过此课程
                    let activities =
                        match self.fetch_activities(selected_course.id, &selected_course.name) {
                            Ok(acts) => acts,
                            Err(e) => {
                                error!("拉取课程的 activities：{}", e);
                                return None;
                            }
                        };

                    let local_tasks: Vec<(String, String, u64, String)> = activities
                        .iter()
                        .filter_map(|activity| activity["uploads"].as_array())
                        .flat_map(|uploads| uploads.iter())
                        .filter_map(|upload| {
                            // 提取 reference_id，如果不存在则跳过
                            let id = upload["reference_id"].as_u64()?;
                            if activity_upload_record.contains(&id) {
                                return None;
                            }

                            // 提取文件名，如果不存在则使用默认值
                            let name = upload["name"].as_str()?.to_string();

                            // 根据设置决定是否跳过 mp4 文件
                            if settings.mp4_trashed
                                && PathBuf::from(&name)
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .unwrap_or("")
                                    .to_lowercase()
                                    == "mp4"
                            {
                                return None;
                            }

                            // 构建任务元组
                            Some((
                                selected_course.semester.clone(),
                                selected_course.name.clone(),
                                id,
                                name,
                            ))
                        })
                        .collect();
                    // 如果没有任务，则返回 None，否则返回任务列表
                    if local_tasks.is_empty() {
                        None
                    } else {
                        Some(local_tasks)
                    }
                })
                .flatten()
                .collect()
        });

        #[cfg(debug_assertions)]
        println!("fetch_activities: {:?}", start.elapsed());

        Ok(tasks)
    }

    /// 拉取新课件！
    #[cfg(feature = "pb")]
    pub fn fetch_activity_uploads(
        &self,
        path_download: &PathBuf,
        path_activity_upload_record: &PathBuf,
        selected_courses: Vec<CourseFull>,
        mut activity_upload_record: Vec<u64>,
        settings: &utils::Settings,
    ) -> Result<()> {
        begin!("更新课件信息");
        let tasks =
            self.fetch_download_tasks(selected_courses, &activity_upload_record, settings)?;

        if tasks.is_empty() {
            warning!("没有新课件");
            return Ok(());
        }
        end!("更新课件信息");

        waiting!("拉取新课件");
        let multi_pb = Arc::new(MultiProgress::new());
        // 进度条样式
        let pb_style = ProgressStyle::with_template(
            "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("=>-");

        // 用自定义线程池将并发限制为 4
        let pool = ThreadPoolBuilder::new().num_threads(4).build()?;
        pool.install(|| {
            let successful_uploads: Vec<u64> = tasks
                .par_iter()
                .filter_map(|(semester, course_name, upload_id, file_name)| {
                    let pb = multi_pb.add(ProgressBar::new(0));
                    pb.set_style(pb_style.clone());
                    pb.set_message(format!("{} {}", "⚙".blue(), file_name));
                    match Session::download_upload(
                        self,
                        &path_download.join(semester).join(course_name),
                        *upload_id,
                        file_name,
                        settings.is_pdf,
                        pb,
                    ) {
                        Ok(_) => Some(*upload_id),
                        Err(e) => {
                            error!("下载 {} ：{}", file_name, e);
                            None
                        }
                    }
                })
                .collect();
            if !successful_uploads.is_empty() {
                activity_upload_record.extend(successful_uploads);
                match Session::store_activity_upload_record(
                    path_activity_upload_record,
                    &activity_upload_record,
                ) {
                    Ok(_) => {}
                    Err(e) => error!("存储下载课件记录：{}", e),
                }
            }
        });

        Ok(())
    }

    /// 下载一个upload文件！
    #[cfg(feature = "pb")]
    pub fn download_upload(
        &self,
        path_download: &PathBuf,
        id: u64,
        name: &str,
        is_pdf: bool,
        pb: ProgressBar,
    ) -> Result<()> {
        let download_url = if is_pdf {
            let mut retries = 0;
            loop {
                let json:Value = self.get(format!("https://courses.zju.edu.cn/api/uploads/reference/document/{}/url?preview=true",id)).send()?.json().or_else(|e| {
                        error!("json失败：{}", e);
                        Err(e)
                    })?;

                let Some(status) = json["status"].as_str() else {
                    return Err(anyhow::anyhow!("json 不含 status 字段"));
                };
                if status == "ready" {
                    let Some(url) = json["url"].as_str() else {
                        return Err(anyhow::anyhow!("json 不含 url 字段"));
                    };
                    break url.to_string();
                }

                retries += 1;
                if retries == 3 {
                    error!("雪灾浙大一直准备不好 {}", name);
                    return Ok(());
                }
            }
        } else {
            format!(
                "https://courses.zju.edu.cn/api/uploads/reference/{}/blob",
                id
            )
        };

        let mut res = self.get(&download_url).send()?;

        fs::create_dir_all(path_download)?;

        // 修改文件名的拓展名与下载链接一致
        let file_name = if is_pdf {
            let extension = std::path::Path::new(&download_url)
                .extension()
                .unwrap()
                .to_str()
                .unwrap();
            std::path::Path::new(name)
                .with_extension(extension)
                .to_string_lossy()
                .to_string()
        } else {
            name.to_string()
        };

        let mut file = File::create(std::path::Path::new(path_download).join(&file_name))?;

        let total_size = res
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|l| l.to_str().ok().and_then(|s| s.parse::<u64>().ok()))
            .unwrap_or(0);

        if total_size > 0 {
            pb.set_length(total_size);
        } else {
            pb.set_length(0);
        }

        let mut buffer = [0; 8192];

        loop {
            let bytes = res.read(&mut buffer)?;
            if bytes == 0 {
                break;
            }
            file.write_all(&buffer[..bytes])?;
            pb.inc(bytes as u64);
        }

        pb.finish_with_message(format!("{} {}", "✓".green(), file_name));
        Ok(())
    }

    /// 拉取新课件！
    #[cfg(not(feature = "pb"))]
    pub fn fetch_activity_uploads(
        &self,
        path_download: &PathBuf,
        path_activity_upload_record: &PathBuf,
        selected_courses: Vec<CourseFull>,
        mut activity_upload_record: Vec<u64>,
        settings: &utils::Settings,
    ) -> Result<()> {
        begin!("更新课件信息");
        let tasks =
            self.fetch_download_tasks(selected_courses, &activity_upload_record, settings)?;

        if tasks.is_empty() {
            warning!("没有新课件");
            return Ok(());
        }
        end!("更新课件信息");

        // 用自定义线程池将并发限制为 4
        waiting!("拉取新课件");
        #[cfg(debug_assertions)]
        let start = std::time::Instant::now();
        let pool = ThreadPoolBuilder::new().num_threads(4).build()?;
        pool.install(|| {
            let successful_uploads: Vec<u64> = tasks
                .par_iter()
                .filter_map(|(semester, course_name, upload_id, file_name)| {
                    #[cfg(debug_assertions)]
                    process!("{} :: {}", course_name, file_name);

                    match self.download_upload(
                        &path_download.join(semester).join(course_name),
                        *upload_id,
                        file_name,
                        settings.is_pdf,
                    ) {
                        Ok(_) => Some(*upload_id),
                        Err(e) => {
                            error!("下载 {} ：{}", file_name, e);
                            None
                        }
                    }
                })
                .collect();
            if !successful_uploads.is_empty() {
                success!("拉取新课件");
                activity_upload_record.extend(successful_uploads);
                match Session::store_activity_upload_record(
                    path_activity_upload_record,
                    &activity_upload_record,
                ) {
                    Ok(_) => {}
                    Err(e) => error!("存储下载课件记录：{}", e),
                }
            }
        });
        #[cfg(debug_assertions)]
        println!("下载课件: {:?}", start.elapsed());

        Ok(())
    }

    /// 下载一个upload文件！
    #[cfg(not(feature = "pb"))]
    pub fn download_upload(
        &self,
        path_download: &PathBuf,
        id: u64,
        name: &str,
        is_pdf: bool,
    ) -> Result<()> {
        let download_url = if is_pdf {
            let mut retries = 0;
            loop {
                let json:Value=self.get(format!("https://courses.zju.edu.cn/api/uploads/reference/document/{}/url?preview=true",id)).send()?.json().or_else(|e| {
                        error!("json失败：{}", e);
                        Err(e)
                    })?;
                let Some(status) = json["status"].as_str() else {
                    return Err(anyhow::anyhow!("json 不含 status 字段"));
                };
                if status == "ready" {
                    let Some(url) = json["url"].as_str() else {
                        return Err(anyhow::anyhow!("json 不含 url 字段"));
                    };
                    break url.to_string();
                }
                retries += 1;
                if retries == 3 {
                    error!("雪灾浙大一直准备不好 {}", name);
                    return Ok(());
                }
            }
        } else {
            format!(
                "https://courses.zju.edu.cn/api/uploads/reference/{}/blob",
                id
            )
        };

        let mut res = self.get(&download_url).send()?;

        fs::create_dir_all(path_download)?;

        // 修改文件名的拓展名与下载链接一致
        let file_name = if is_pdf {
            let extension = std::path::Path::new(&download_url)
                .extension()
                .unwrap()
                .to_str()
                .unwrap();
            std::path::Path::new(name)
                .with_extension(extension)
                .to_string_lossy()
                .to_string()
        } else {
            name.to_string()
        };

        let mut file = File::create(std::path::Path::new(path_download).join(&file_name))?;

        // 流式下载，避免大文件问题
        res.copy_to(&mut file).map_err(|e| {
            error!("下载失败：{}", e);
            e
        })?;

        success!("{} -> {}", file_name, path_download.display());
        Ok(())
    }

    /// 上传文件到个人资料库
    pub fn upload_file(&self, file_path: &PathBuf) -> Result<u64> {
        #[cfg(debug_assertions)]
        process!("上传文件：{}", file_path.display());

        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        let file_size = file_path.metadata().unwrap().len();
        let payload = json!({
            "embed_material_type": "",
            "is_marked_attachment": false,
            "is_scorm": false,
            "is_wmpkg": false,
            "name": file_name,
            "parent_id": 0,
            "parent_type": null,
            "size": file_size,
            "source": ""
        });
        const POST_URL: &str = "https://courses.zju.edu.cn/api/uploads";
        #[cfg(debug_assertions)]
        process!("已准备好发送上传请求");

        let mut res;
        let mut json: Option<Value> = None; // 使用 Option 包装

        for attempt in 1..=utils::MAX_RETRIES {
            res = self.client.post(POST_URL).json(&payload).send()?;
            let content = res.text()?;
            match serde_json::from_str::<Value>(&content) {
                Ok(json_unjudged) => {
                    #[cfg(debug_assertions)]
                    println!("POST response as JSON: {:#?}", json_unjudged);
                    if json_unjudged["errors"].is_object() {
                        error!("雪灾浙大不支持{}的文件类型", file_name);
                        return Ok(0);
                    }
                    json = Some(json_unjudged);
                    break;
                }
                Err(_) => {
                    #[cfg(debug_assertions)]
                    warning!("POST attempt {}/{} Failed", attempt, utils::MAX_RETRIES);
                }
            }
            #[cfg(debug_assertions)]
            warning!("retry {}/{}: 上传请求失败", attempt, utils::MAX_RETRIES);
        }

        #[cfg(debug_assertions)]
        process!("上传请求已被接受");
        if json.is_none() {
            error!("上传请求失败");
            return Ok(0);
        }

        let json = json.unwrap(); // 断言 json 已被赋值

        let upload_url = json["upload_url"].as_str().unwrap();
        let id = json["id"].as_u64().unwrap();
        #[cfg(debug_assertions)]
        println!("Upload URL: {}", upload_url);

        let mut file = File::open(file_path)?;
        let mut file_content = Vec::new();
        file.read_to_end(&mut file_content)?;
        let file_name = json["name"].as_str().unwrap();

        let file_part = multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = multipart::Form::new().part("file", file_part);

        let res = self.client.put(upload_url).multipart(form).send()?;

        if res.status().is_success() {
            #[cfg(debug_assertions)]
            success!("上传文件");
        } else {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            error!("上传状态码：{}，响应内容：{}", status, text);
        }

        Ok(id)
    }

    /// 将 学期 -> 课程 映射表转换为活跃课程列表
    pub fn filter_active_courses(
        semester_course_map: &HashMap<String, Vec<CourseData>>,
    ) -> Vec<CourseData> {
        let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
        let filtered_semester_list = filter_latest_group(&semester_list);

        let courses: Vec<CourseData> = filtered_semester_list
            .iter()
            .map(|semester| semester_course_map.get(semester).unwrap().clone())
            .flatten()
            .collect();

        courses
    }

    /// 加载活跃课程
    pub fn load_active_courses(path_active_courses: &PathBuf) -> Result<Vec<CourseData>> {
        let data = fs::read_to_string(path_active_courses)?;
        let active_courses: Vec<CourseData> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载活跃课程");

        Ok(active_courses)
    }

    /// 存储活跃课程
    pub fn store_active_courses(
        path_active_courses: &PathBuf,
        active_courses: &Vec<CourseData>,
    ) -> Result<()> {
        fs::write(path_active_courses, serde_json::to_string(active_courses)?)?;

        #[cfg(debug_assertions)]
        success!("存储活跃课程");

        Ok(())
    }

    /// 获取作业列表
    ///
    /// homework: id, name, ddl, description
    pub fn get_homework_list(&self, path_active_courses: &PathBuf) -> Result<Vec<Homework>> {
        let courses = try_or_throw!(
            Session::load_active_courses(path_active_courses),
            "加载活跃课程"
        );
        let num = courses.len();
        let pool = ThreadPoolBuilder::new().num_threads(num).build()?;
        let all_homeworks :Vec<Homework> = pool.install(||{
            courses.par_iter().filter_map(|course|{
                let url = format!("https://courses.zju.edu.cn/api/courses/{}/homework-activities?page=1&page_size=100&reloadPage=false",course.id);
                let mut homeworks:Vec<Homework> =Vec::new();
                for attempt in 1..=utils::MAX_RETRIES{
                    let session = self.clone();
                    match session.client.get(&url).send(){
                        Ok(res)=> {
                            match res.json::<Value>(){
                            Ok(json)=>{
                                if let Some(homeworks_unwashed) = json["homework_activities"].as_array(){
                                    #[cfg(debug_assertions)]
                                    success!("{}::homeworks", course.name);
                                    homeworks.extend(homeworks_unwashed
                                        .iter()
                                        .filter(|hw| hw["is_in_progress"].as_bool().unwrap())
                                        .map(|hw| {
                                            let description_html = hw["data"]["description"].as_str().unwrap_or("");
                                            let description = html2text::from_read(description_html.as_bytes(), 80).unwrap();
                                            let id = hw["id"].as_u64().unwrap();
                                            let ddl = format_ddl(hw["deadline"].as_str().unwrap());
                                            let status = hw["submitted"].as_bool().unwrap();
                                            use colored::Colorize;
                                            let status_signal = if status {
                                                "✓".green()
                                            } else {
                                                "!".yellow()
                                            };
                                            let ddl = if status{
                                                ddl.green()
                                            }else{
                                                ddl.yellow()
                                            };
                                            let name = format!(
                                                "{} {}::{}\n\t{}\n\t{}",
                                                status_signal,
                                                course.name,
                                                hw["title"].as_str().unwrap(),
                                                ddl,
                                                description
                                            );
                                            Homework { id, name }
                                        })
                                        .collect::<Vec<Homework>>());
                                    break;
                                }
                            },
                            Err(e)=>{
                                #[cfg(debug_assertions)]
                                warning!(
                                    "retry {}/{}: {} 的返回无法解析为 json: {}",
                                    attempt,
                                    utils::MAX_RETRIES,
                                    course.name,
                                    e
                                );
                            }
                        }},
                        Err(e) => {
                            warning!(
                                "retry {}/{}: {} 的请求失败: {}",
                                attempt,
                                utils::MAX_RETRIES,
                                course.name,
                                e
                            );
                        }
                    }
                }
                if homeworks.is_empty(){
                    None
                }else{
                    Some(homeworks)
                }
            }).flatten().collect()
        });

        Ok(all_homeworks)
    }

    /// 上交作业
    pub fn handin_homework(
        &self,
        homework_id: u64,
        file_id: u64,
        mut comment: String,
    ) -> Result<()> {
        let handin_url = format!(
            "https://courses.zju.edu.cn/api/course/activities/{}/submissions",
            homework_id
        );

        if !comment.is_empty() {
            comment = format!("<p>{}<br></p>", comment);
        }
        let payload = json!({
            "comment":comment,
            "is_draft":false,
            "mode":"normal",
            "other_resources":[],
            "slides":[],
            "uploads":[file_id],
            "uploads_in_rich_text":[]
        });
        #[cfg(debug_assertions)]
        process!("已准备好发送提交作业请求");

        let mut res;
        let mut json: Option<Value> = None; // 使用 Option 包装

        for attempt in 1..=utils::MAX_RETRIES {
            res = self.client.post(&handin_url).json(&payload).send()?;
            let content = res.text()?;
            match serde_json::from_str::<Value>(&content) {
                Ok(json_unjudged) => {
                    #[cfg(debug_assertions)]
                    println!("SUBMIT POST response as JSON: {:#?}", json_unjudged);
                    if json_unjudged["errors"].is_array() {
                        error!("上交作业失败");
                        return Ok(());
                    }
                    json = Some(json_unjudged);
                    break;
                }
                Err(_) => {
                    #[cfg(debug_assertions)]
                    warning!("POST attempt {}/{} Failed", attempt, utils::MAX_RETRIES);
                }
            }
            #[cfg(debug_assertions)]
            warning!("retry {}/{}: 上传请求失败", attempt, utils::MAX_RETRIES);
        }

        #[cfg(debug_assertions)]
        process!("上交作业请求已被接受");
        if json.is_none() {
            error!("上传作业失败");
        }

        Ok(())
    }

    /// 查询成绩的核心内容
    fn query_grades(&self, form: Value) -> Result<Vec<Value>> {
        let res = try_or_throw!(self.client.post(GRADE_URL).form(&form).send(), "查询成绩");
        let json: Value = res.json()?;
        let grade_json = match json["data"]["list"].as_array() {
            Some(grade_json) => grade_json.to_owned(),
            None => {
                let again_res = try_or_throw!(
                    self.client.get(GRADE_SERVICE_URL).send(),
                    "连接成绩查询主页"
                );
                if again_res.url().query().map(|q| q.to_owned()).is_none() {
                    let res =
                        try_or_throw!(self.client.post(GRADE_URL).form(&form).send(), "查询成绩");
                    let json: Value = res.json()?;
                    json["data"]["list"].as_array().unwrap().to_owned()
                } else {
                    println!("{:?}", again_res);
                    return Err(anyhow!("无法获取成绩"));
                }
            }
        };
        Ok(grade_json)
    }

    /// 获取成绩 并打印全部
    pub fn get_grade(&self, path_courses: &PathBuf, account: &account::Account) -> Result<()> {
        let form = json!({
            "xh":account.stuid
        });
        begin!("查询成绩");
        let grade_json = self.query_grades(form)?;
        end!("查询成绩");

        let semester_course_map = Session::load_semester_course_map(path_courses)?;
        let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
        let filtered_semester_list = filter_latest_group(&semester_list);
        let filtered_semester_group_list: Vec<(&str, &str)> = filtered_semester_list
            .iter()
            .map(|semester| split_semester(semester))
            .collect();
        let xn_set: std::collections::HashSet<&str> = filtered_semester_group_list
            .iter()
            .map(|(xn, _)| *xn)
            .collect();
        let xq_set: std::collections::HashSet<&str> = filtered_semester_group_list
            .iter()
            .map(|(_, xq)| *xq)
            .collect();
        let mut weight_sum = 0.0;
        let mut credit_sum = 0.0;
        let mut weight_sum_semester = 0.0;
        let mut credit_sum_semester = 0.0;
        let mut weight_sum_year = 0.0;
        let mut credit_sum_year = 0.0;

        let all_grade_list: Vec<Grade> = grade_json
            .iter()
            .filter_map(|grade_json| {
                let obj = grade_json.as_object()?;
                let name = obj["kcmc"].as_str()?;
                let xq = obj["xq"].as_str()?;
                let xn = obj["xn"].as_str()?;
                let credit = obj["xf"].as_str()?;
                let gpa = obj["jd"].as_f64()?;
                let grade = obj["cj"].as_str()?;
                let gpa_str = format_gpa_str(gpa);
                let credit_num: f64 = credit.parse().unwrap();
                let name_str = format_class_name(name, credit_num);
                weight_sum += gpa * credit_num;
                credit_sum += credit_num;
                if xn_set.contains(xn) {
                    weight_sum_year += gpa * credit_num;
                    credit_sum_year += credit_num;
                    if xq_set.contains(xq) {
                        weight_sum_semester += gpa * credit_num;
                        credit_sum_semester += credit_num;
                    }
                }
                return Some(Grade {
                    name: name_str.to_string(),
                    grade: grade.to_string(),
                    credit: credit.to_string(),
                    gpa: gpa_str.to_string(),
                });
            })
            .collect();

        let avg_gpa = weight_sum / credit_sum;
        let avg_gpa_semester = weight_sum_semester / credit_sum_semester;
        let avg_gpa_year = weight_sum_year / credit_sum_year;
        println!("{}", create_table(&all_grade_list));
        println!(
            "学期均绩：{:.2}/{:.1}",
            avg_gpa_semester, credit_sum_semester
        );
        println!("学年均绩：{:.2}/{:.1}", avg_gpa_year, credit_sum_year);
        println!(" 总均绩 ：{:.2}/{:.1}", avg_gpa, credit_sum);
        Ok(())
    }

    /// 获取成绩 并打印本学期
    pub fn get_g(&self, path_courses: &PathBuf, account: &account::Account) -> Result<()> {
        let form = json!({
            "xh":account.stuid
        });
        begin!("查询成绩");
        let grade_json = self.query_grades(form)?;
        end!("查询成绩");

        let semester_course_map = Session::load_semester_course_map(path_courses)?;
        let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
        let filtered_semester_list = filter_latest_group(&semester_list);
        let filtered_semester_group_list: Vec<(&str, &str)> = filtered_semester_list
            .iter()
            .map(|semester| split_semester(semester))
            .collect();
        let xn_set: std::collections::HashSet<&str> = filtered_semester_group_list
            .iter()
            .map(|(xn, _)| *xn)
            .collect();
        let xq_set: std::collections::HashSet<&str> = filtered_semester_group_list
            .iter()
            .map(|(_, xq)| *xq)
            .collect();
        let mut weight_sum = 0.0;
        let mut credit_sum = 0.0;
        let mut weight_sum_semester = 0.0;
        let mut credit_sum_semester = 0.0;
        let mut weight_sum_year = 0.0;
        let mut credit_sum_year = 0.0;

        let grade_list: Vec<Grade> = grade_json
            .iter()
            .filter_map(|grade_json| {
                let obj = grade_json.as_object()?;
                let xq = obj["xq"].as_str()?;
                let xn = obj["xn"].as_str()?;
                let gpa = obj["jd"].as_f64()?;
                let credit = obj["xf"].as_str()?;
                let credit_num: f64 = credit.parse().unwrap();
                weight_sum += gpa * credit_num;
                credit_sum += credit_num;
                if xn_set.contains(xn) {
                    weight_sum_year += gpa * credit_num;
                    credit_sum_year += credit_num;
                    if xq_set.contains(xq) {
                        let name = obj["kcmc"].as_str()?;
                        let grade = obj["cj"].as_str()?;
                        let gpa_str = format_gpa_str(gpa);
                        let name_str = format_class_name(name, credit_num);
                        weight_sum_semester += gpa * credit_num;
                        credit_sum_semester += credit_num;
                        return Some(Grade {
                            name: name_str.to_string(),
                            grade: grade.to_string(),
                            credit: credit.to_string(),
                            gpa: gpa_str.to_string(),
                        });
                    }
                }
                None
            })
            .collect();
        let avg_gpa = weight_sum / credit_sum;
        let avg_gpa_semester = weight_sum_semester / credit_sum_semester;
        let avg_gpa_year = weight_sum_year / credit_sum_year;
        let table = create_table(&grade_list);
        println!("{}", table);
        println!(
            "学期均绩：{:.2}/{:.1}",
            avg_gpa_semester, credit_sum_semester
        );
        println!("学年均绩：{:.2}/{:.1}", avg_gpa_year, credit_sum_year);
        println!(" 总均绩 ：{:.2}/{:.1}", avg_gpa, credit_sum);

        Ok(())
    }
}

impl Deref for Session {
    type Target = Client;
    fn deref(&self) -> &Client {
        &self.client
    }
}

pub struct Course {
    pub id: u64,
    pub sid: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CourseData {
    pub id: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct CourseFull {
    pub id: u64,
    pub semester: String,
    pub name: String,
}

pub struct Homework {
    pub id: u64,
    pub name: String,
}

pub struct Grade {
    pub name: String,
    pub grade: String,
    pub credit: String,
    pub gpa: String,
}

/// 拆分 "2024-2025春夏" => ("2024-2025", "春夏") 的辅助函数
///
/// 这个函数非常脆弱，只有在 semester 的格式是 "xxxx-yyyy春夏" 的时候才能正常工作
fn split_semester(semester: &str) -> (&str, &str) {
    for (i, c) in semester.chars().enumerate() {
        if "春夏秋冬短".contains(c) {
            // i 是后缀开始位置
            return (&semester[..i], &semester[i..]);
        }
    }
    panic!("无法拆分学期：{}", semester);
}

/// 将「年-年前缀」解析为一个便于比较的整型，"2024-2025" => 2024
fn parse_year_prefix(prefix: &str) -> u32 {
    let parts: Vec<&str> = prefix.split('-').collect();
    return parts[0].parse().unwrap();
}

/// 给后缀定义自定义排序规则
/// 返回 (group, subpriority) 来进行排序
/// group 越大越靠前（同组内挨在一起），subpriority 越大越靠前
fn suffix_order(suffix: &str) -> (u8, u8) {
    match suffix {
        // 春夏组 => 夏 > 春夏 > 春
        "夏" => (2, 2),
        "春夏" => (2, 1),
        "春" => (2, 0),
        // 秋冬组 => 冬 > 秋冬 > 秋
        "冬" => (1, 2),
        "秋冬" => (1, 1),
        "秋" => (1, 0),
        // 短 => 最后
        "短" => (0, 0),
        // 其它任意后缀
        _ => (3, 0),
    }
}

/// 根据已有的 split_semester, parse_year_prefix, suffix_order
/// 返回：具备“最大年前缀”和“最大后缀group”的所有项，并按subpriority降序排列。
fn filter_latest_group(semesters: &[String]) -> Vec<String> {
    let mut parsed = Vec::new();
    for sem in semesters {
        let (prefix, suffix) = split_semester(sem);
        let year = parse_year_prefix(prefix); // 返回 u32
        let (group, sub) = suffix_order(suffix); // 返回 (u8, u8)
        parsed.push((sem.clone(), year, group, sub));
    }

    // 1) 找出最大的年前缀
    let max_year = parsed.iter().map(|(_, y, _, _)| *y).max().unwrap_or(0);
    // 2) 只保留年前缀= max_year 的项目
    let filtered: Vec<_> = parsed
        .into_iter()
        .filter(|(_, y, _, _)| *y == max_year)
        .collect();

    // 3) 在这些项目里，找出最大的 group
    let max_group = filtered.iter().map(|(_, _, g, _)| *g).max().unwrap_or(0);
    // 4) 只保留 group= max_group 的项目
    let mut final_items: Vec<_> = filtered
        .into_iter()
        .filter(|(_, _, g, _)| *g == max_group)
        .collect();

    // 5) 按 subpriority 降序排序
    final_items.sort_by(|a, b| b.3.cmp(&a.3));

    // 返回原学期字符串
    final_items.into_iter().map(|(s, _, _, _)| s).collect()
}

fn format_ddl(original_ddl: &str) -> String {
    use chrono::{DateTime, Utc};
    let time = DateTime::parse_from_rfc3339(original_ddl).unwrap();
    let time_utc: DateTime<Utc> = time.with_timezone(&Utc);
    let formatted_ddl = time_utc.format("ddl: %m-%d %H:%M %Y").to_string();
    formatted_ddl
}

fn strip_ansi_codes(s: &str) -> String {
    let mut stripped = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // 开始转义序列，跳过直到 'm'
            while let Some(c_inner) = chars.next() {
                if c_inner == 'm' {
                    break;
                }
            }
        } else {
            stripped.push(c);
        }
    }
    stripped
}

fn is_wide_char(c: char) -> bool {
    match c {
        // CJK Unified Ideographs
        '\u{4E00}'..='\u{9FFF}' | '（' | '）' => true,
        _ => false,
    }
}

fn wide_char_num(c: &str) -> usize {
    c.chars().map(|c| if is_wide_char(c) { 1 } else { 0 }).sum()
}

fn width_shift(c: &str) -> isize {
    let cjk_shift: isize = c
        .chars()
        .map(|c| if is_wide_char(c) { -1 } else { 0 })
        .sum();
    let color_shift: isize = if c.contains('\x1b') { 9 } else { 0 };
    cjk_shift + color_shift
}
fn display_width(s: &str) -> usize {
    strip_ansi_codes(s)
        .chars()
        .map(|c| if is_wide_char(c) { 2 } else { 1 })
        .sum()
}

fn calculate_column_widths(grades: &[Grade], headers: &[&str]) -> Vec<usize> {
    // 初始化列宽为标题的宽度
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h)).collect();

    // 更新列宽为内容的最大宽度
    for grade in grades {
        let columns = [&grade.name, &grade.grade, &grade.credit, &grade.gpa];
        for (col, width) in columns.iter().zip(widths.iter_mut()) {
            let len = display_width(col);
            if len > *width {
                *width = len;
            }
        }
    }
    widths
}

fn create_table(data: &[Grade]) -> String {
    const HEADERS: [&str; 4] = ["课程", "成绩", "绩点", "学分"];
    let column_widths = calculate_column_widths(data, &HEADERS);
    let mut table = String::new();

    // 构建分隔线
    let total_length: usize = column_widths.iter().map(|w| 1 + w + 2).sum::<usize>() + 2;
    let mut separator = String::with_capacity(total_length);
    for &w in &column_widths {
        separator.push('+');
        separator.push_str(&"-".repeat(w + 2));
    }
    separator.push_str("+\n");

    // 添加表头
    table.push_str(&separator);
    table.push('|');
    for (header, &width) in HEADERS.iter().zip(column_widths.iter()) {
        let total_width = width - wide_char_num(header);
        let padded = format!(" {:width$} |", header, width = total_width);
        table.push_str(&padded);
    }
    table.push('\n');
    table.push_str(&separator);

    // 添加数据行
    for grade in data {
        table.push('|');
        let columns = [&grade.name, &grade.grade, &grade.gpa, &grade.credit];
        for (col, &width) in columns.iter().zip(column_widths.iter()) {
            let total_width = (width.to_isize().unwrap() + width_shift(col))
                .to_usize()
                .unwrap();
            let padded = format!(" {:width$} |", col, width = total_width);
            table.push_str(&padded);
        }
        table.push('\n');
        table.push_str(&separator);
    }
    table
}

fn format_gpa_str(gpa: f64) -> ColoredString {
    let formatted_gpa = format!("{:.1}", gpa);
    match gpa {
        4.5..=5.0 => formatted_gpa.green(),
        3.5..4.5 => formatted_gpa.cyan(),
        2.4..3.5 => formatted_gpa.yellow(),
        0.0..2.4 => formatted_gpa.red(),
        _ => formatted_gpa.white(),
    }
}

fn format_class_name(name: &str, credit_num: f64) -> ColoredString {
    match credit_num {
        4.0..=5.0 => name.purple(),
        2.0..=3.0 => name.blue(),
        _ => name.white(),
    }
}
