use crate::{
    account, begin, end, error, success, try_or_exit, try_or_throw, utils, waiting, warning,
};
use crate::{blue, gray, purple};

use ::serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
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
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{stdout, Read};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const LOGIN_URL: &str = "https://zjuam.zju.edu.cn/cas/login";
const PUBKEY_URL: &str = "https://zjuam.zju.edu.cn/cas/v2/getPubKey";
const HOME_URL: &str = "https://courses.zju.edu.cn";
const GRADE_SERVICE_URL: &str = "http://appservice.zju.edu.cn/zdjw/cjcx/cjcxjg";
const GRADE_URL: &str = "http://appservice.zju.edu.cn/zju-smartcampus/zdydjw/api/kkqk_cxXscjxx";
const POST_URL: &str = "https://courses.zju.edu.cn/api/uploads";

use {
    indicatif::{MultiProgress, ProgressBar, ProgressStyle},
    std::io::Write,
};

#[cfg(debug_assertions)]
use crate::process;

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

/// 会话状态
struct State {
    /// 共享 cookie 状态
    cookie_store: Arc<CookieStoreMutex>,
    /// 本地 cookie 文件路径
    path_cookies: PathBuf,
}

impl State {
    /// 建立新的 state
    ///
    /// 使用可能已经存在的本地 cookie 文件。若没有，会自动创建
    pub fn try_new(path_cookies: PathBuf) -> Result<State> {
        #[allow(deprecated)]
        let cookie_store = match File::open(&path_cookies) {
            Ok(file) => CookieStore::load_json(std::io::BufReader::new(file)).unwrap_or_default(),
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

    /// 清除当前 cookie 和本地 cookie
    pub fn clear_cookie(&self) -> Result<()> {
        self.cookie_store.lock().unwrap().clear();
        if self.path_cookies.exists() {
            fs::remove_file(&self.path_cookies)?;
        }
        Ok(())
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
/// 网络会话
///
/// 自动管理会话 cookie 并在销毁时保存 cookie 到本地
pub struct Session {
    #[allow(dead_code)] // just make clippy happy
    state: Arc<State>,
    client: Client,
    path_courses: PathBuf,
    path_active_courses: PathBuf,
    path_selected_courses: PathBuf,
    path_activity_upload_record: PathBuf,
    path_active_semesters: PathBuf,
}

impl Session {
    /// 建立新的会话
    pub fn try_new(
        path_cookies: PathBuf,
        path_courses: PathBuf,
        path_active_courses: PathBuf,
        path_selected_courses: PathBuf,
        path_activity_upload_record: PathBuf,
        path_active_semesters: PathBuf,
    ) -> Result<Session> {
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
            .no_proxy()
            .build()?;

        #[cfg(debug_assertions)]
        success!("建立会话");

        Ok(Session {
            state,
            client,
            path_courses,
            path_active_courses,
            path_selected_courses,
            path_activity_upload_record,
            path_active_semesters,
        })
    }

    fn login_core(&self, account: &account::AccountData) -> Result<()> {
        for retry in 1..=utils::MAX_RETRIES {
            let (execution, (modulus, exponent)) = rayon::join(
                || {
                    let res = try_or_exit!(self.client.get(LOGIN_URL).send(), "连接登录页");
                    let text = res.text().unwrap();
                    let re = regex::Regex::new(
                        r#"<input type="hidden" name="execution" value="(.*?)" />"#,
                    )
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

            if res
                .url()
                .to_string()
                .contains("https://zjuam.zju.edu.cn/cas/login")
            {
                if retry == utils::MAX_RETRIES {
                    return Err(anyhow!("请检查学号-密码正确性及你的网络连接状态"));
                }
                #[cfg(debug_assertions)]
                warning!("retry {}/{}: 登录失败", retry, utils::MAX_RETRIES);
                continue;
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

            return Ok(());
        }
        Ok(())
    }

    /// 登录，使用本地 cookie
    pub fn login(&self, account: &account::AccountData) -> Result<()> {
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

    /// 清除本地 cookie 并重新登录
    pub fn relogin(&self, account: &account::AccountData) -> Result<()> {
        try_or_throw!(self.state.clear_cookie(), "清除 cookie");
        self.login_core(account)
    }

    /// 获取学期映射表(id -> name)
    pub fn get_semester_map_and_active_semester(&self) -> Result<(HashMap<u64, String>, String)> {
        let res = self
            .client
            .get("https://courses.zju.edu.cn/api/my-semesters?")
            .send()?;

        let json: Value = res.json()?;
        let mut active_semester = String::new();
        let semester_map: HashMap<u64, String> = json["semesters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let sid = c["id"].as_u64().unwrap();
                let name = c["name"].as_str().unwrap_or_default().to_string();
                if c["is_active"].as_bool().unwrap() {
                    active_semester = name.clone();
                }
                (sid, name)
            })
            .collect();

        #[cfg(debug_assertions)]
        success!("获取学期映射表");
        Ok((semester_map, active_semester))
    }

    /// 获取课程列表
    pub fn get_course_list(&self) -> Result<Vec<Course>> {
        let res = self.client.get("https://courses.zju.edu.cn/api/my-courses?conditions=%7B%22status%22:%5B%22ongoing%22,%22notStarted%22%5D,%22keyword%22:%22%22,%22classify_type%22:%22recently_started%22,%22display_studio_list%22:false%7D&fields=id,name,semester_id&page=1&page_size=1000").send()?;

        let json: Value = res.json()?;
        let Some(courses_json) = json["courses"].as_array() else {
            return Err(anyhow!("返回 json 无 courses 字段"));
        };
        let course_list: Vec<Course> = courses_json
            .iter()
            .map(|c| Course {
                id: c["id"].as_u64().unwrap(),
                sid: c["semester_id"].as_u64().unwrap(),
                name: c["name"].as_str().unwrap().to_string(),
            })
            .collect();

        #[cfg(debug_assertions)]
        success!("获取课程列表");
        Ok(course_list)
    }

    /// 根据课程数组和学期映射表，构建 学期->课程 映射表
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

    /// 存储学期-课程映射表
    pub fn store_semester_course_map(
        &self,
        semester_course_map: &HashMap<String, Vec<CourseData>>,
    ) -> Result<()> {
        std::fs::write(
            &self.path_courses,
            serde_json::to_string(&semester_course_map).unwrap(),
        )?;

        #[cfg(debug_assertions)]
        success!("存储 学期->课程 映射表");
        Ok(())
    }

    /// 加载学期-课程映射表!
    pub fn load_semester_course_map(&self) -> Result<HashMap<String, Vec<CourseData>>> {
        let data = fs::read_to_string(&self.path_courses)?;
        let semester_course_map: HashMap<String, Vec<CourseData>> = serde_json::from_str(&data)?;

        Ok(semester_course_map)
    }

    /// 存储已选课程!
    pub fn store_selected_courses(&self, selected_courses: &Vec<CourseFull>) -> Result<()> {
        std::fs::write(
            &self.path_selected_courses,
            serde_json::to_string(selected_courses)?,
        )?;

        #[cfg(debug_assertions)]
        success!("存储已选课程");
        Ok(())
    }

    /// 加载已选课程!
    pub fn load_selected_courses(&self) -> Result<Vec<CourseFull>> {
        let data = fs::read_to_string(&self.path_selected_courses)?;
        let selected_courses: Vec<CourseFull> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载已选课程");

        Ok(selected_courses)
    }

    /// 存储已下载课件记录!
    pub fn store_activity_upload_record(&self, activity_upload_record: &Vec<u64>) -> Result<()> {
        std::fs::write(
            &self.path_activity_upload_record,
            serde_json::to_string(activity_upload_record)?,
        )?;

        success!(
            "存储已下载课件记录 -> {}",
            &self.path_activity_upload_record.display()
        );
        Ok(())
    }

    /// 加载已下载课件记录!
    pub fn load_activity_upload_record(&self) -> Result<Vec<u64>> {
        let data = fs::read_to_string(&self.path_activity_upload_record)?;
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
                                "retry {attempt}/{}: {course_name} 的返回 json 无 activities 字段",
                                utils::MAX_RETRIES,
                            );
                        }
                    }
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        warning!(
                            "retry {attempt}/{}: {course_name} 的返回无法解析为 json: {e}",
                            utils::MAX_RETRIES,
                        );
                    }
                },
                Err(e) => {
                    warning!(
                        "retry {attempt}/{}: {course_name} 的请求失败: {e}",
                        utils::MAX_RETRIES,
                    );
                }
            }
        }
        Err(anyhow!("{course_name} 的请求失败"))
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
    pub fn fetch_activity_uploads(
        &self,
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
                    pb.set_message(format!("\x1b[34m⚙\x1b[0m {}", file_name));
                    if let Err(e) = self.download_upload(
                        &settings.storage_dir.join(semester).join(course_name),
                        *upload_id,
                        file_name,
                        settings.is_pdf,
                        pb,
                    ) {
                        error!("下载 {file_name} ：{e}");
                        return None;
                    }
                    Some(*upload_id)
                })
                .collect();
            if !successful_uploads.is_empty() {
                activity_upload_record.extend(successful_uploads);
                if let Err(e) = self.store_activity_upload_record(&activity_upload_record) {
                    error!("存储下载课件记录：{e}");
                }
            }
        });

        Ok(())
    }

    /// 下载一个upload文件！
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
                let json:Value = self.get(format!("https://courses.zju.edu.cn/api/uploads/reference/document/{id}/url?preview=true")).send()?.json().or_else(|e| {
                        error!("json失败：{e}");
                        Err(e)
                    })?;

                let Some(status) = json["status"].as_str() else {
                    return Err(anyhow!("json 不含 status 字段"));
                };
                if status == "ready" {
                    let Some(url) = json["url"].as_str() else {
                        return Err(anyhow!("json 不含 url 字段"));
                    };
                    break url.to_string();
                }

                retries += 1;
                if retries == utils::MAX_RETRIES {
                    error!("雪灾浙大一直准备不好 {name}");
                    return Ok(());
                }
            }
        } else {
            format!("https://courses.zju.edu.cn/api/uploads/reference/{id}/blob")
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

        pb.finish_with_message(format!("\x1b[32m✓\x1b[0m {file_name}"));
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
                        let err = &json_unjudged["errors"];
                        return Err(anyhow!("上传文件出错：{err}"));
                    }
                    json = Some(json_unjudged);
                    break;
                }
                Err(_) => {
                    #[cfg(debug_assertions)]
                    warning!("POST attempt {attempt}/{} Failed", utils::MAX_RETRIES);
                }
            }
            #[cfg(debug_assertions)]
            warning!("retry {attempt}/{}: 上传请求失败", utils::MAX_RETRIES);
        }

        #[cfg(debug_assertions)]
        process!("上传请求已被接受");

        let Some(json) = json else {
            return Err(anyhow!("上传请求失败"));
        };
        let Some(upload_url) = json["upload_url"].as_str() else {
            return Err(anyhow!("上传请求返回无 upload_url 字段"));
        };
        let Some(id) = json["id"].as_u64() else {
            return Err(anyhow!("上传请求返回无 id 字段"));
        };
        let Some(file_name) = json["name"].as_str() else {
            return Err(anyhow!("上传请求返回无 name 字段"));
        };

        // 转化文件内容为字节流
        let mut file = File::open(file_path)?;
        let mut file_content = Vec::new();
        file.read_to_end(&mut file_content)?;
        let file_part = multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;
        let form = multipart::Form::new().part("file", file_part);

        let res = self.client.put(upload_url).multipart(form).send()?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            return Err(anyhow!("上传状态码：{status}，响应内容：{text}"));
        }

        #[cfg(debug_assertions)]
        success!("上传文件：{file_name}");

        Ok(id)
    }

    /// 将 学期 -> 课程 映射表转换为活跃学期列表
    pub fn filter_active_semesters(
        semester_course_map: &HashMap<String, Vec<CourseData>>,
        active_semester: &str,
    ) -> Vec<String> {
        let semester_list: Vec<String> = semester_course_map.keys().cloned().collect();
        filter_latest_group(&semester_list, active_semester)
    }

    /// 将 学期 -> 课程 映射表转换为活跃课程列表
    pub fn filter_active_courses(
        semester_course_map: &HashMap<String, Vec<CourseData>>,
        filtered_semester_list: &Vec<String>,
    ) -> Vec<CourseData> {
        let courses: Vec<CourseData> = filtered_semester_list
            .iter()
            .map(|semester| semester_course_map.get(semester).unwrap().clone())
            .flatten()
            .collect();

        courses
    }

    /// 加载活跃课程
    pub fn load_active_courses(&self) -> Result<Vec<CourseData>> {
        let data = fs::read_to_string(&self.path_active_courses)?;
        let active_courses: Vec<CourseData> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载活跃课程");

        Ok(active_courses)
    }

    /// 存储活跃课程
    pub fn store_active_courses(&self, active_courses: &Vec<CourseData>) -> Result<()> {
        fs::write(
            &self.path_active_courses,
            serde_json::to_string(active_courses)?,
        )?;

        #[cfg(debug_assertions)]
        success!("存储活跃课程");

        Ok(())
    }

    /// 加载活跃学期
    pub fn load_active_semesters(&self) -> Result<Vec<String>> {
        let data = fs::read_to_string(&self.path_active_semesters)?;
        let active_semesters: Vec<String> = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("加载活跃学期");

        Ok(active_semesters)
    }

    /// 存储活跃学期
    pub fn store_active_semesters(&self, active_semesters: &Vec<String>) -> Result<()> {
        fs::write(
            &self.path_active_semesters,
            serde_json::to_string(active_semesters)?,
        )?;

        #[cfg(debug_assertions)]
        success!("存储活跃学期");

        Ok(())
    }

    /// 获取作业列表
    ///
    /// homework: id, name, ddl, description
    pub fn get_homework_list(&self) -> Result<Vec<Homework>> {
        let courses = try_or_throw!(self.load_active_courses(), "加载活跃课程");
        let num = courses.len();
        let pool = ThreadPoolBuilder::new().num_threads(num).build()?;
        let all_homeworks :Vec<Homework> = pool.install(||{
            courses.par_iter().filter_map(|course|{
                let url = format!("https://courses.zju.edu.cn/api/courses/{}/homework-activities?page=1&page_size=100&reloadPage=false",course.id);
                let mut homeworks:Vec<Homework> =Vec::new();
                for attempt in 1..=utils::MAX_RETRIES{
                    #[cfg(debug_assertions)]
                    let start = std::time::Instant::now();
                    let session = self.clone();
                    match session.client.get(&url).send(){
                        Err(e) => {
                            warning!(
                                "retry {attempt}/{}: {} 的请求失败: {e}",
                                utils::MAX_RETRIES,
                                course.name
                            );
                        },
                        Ok(res)=> {
                            match res.json::<Value>(){
                                Err(e)=>{
                                    #[cfg(debug_assertions)]
                                    warning!(
                                        "retry {attempt}/{}: {} 的返回无法解析为 json: {e}",
                                        utils::MAX_RETRIES,
                                        course.name
                                    );
                                },
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
                                                let (status_signal, ddl) = if status {
                                                    ("\x1b[32m✓\x1b[0m", format!("\x1b[32m{ddl}\x1b[0m"))
                                                } else {
                                                    ("\x1b[33m!\x1b[0m", format!("\x1b[33m{ddl}\x1b[0m"))
                                                };
                                                let name = format!(
                                                    "{status_signal} {}::{}\n\t{ddl}\n\t{description}",
                                                    course.name,
                                                    hw["title"].as_str().unwrap()
                                                );
                                                Homework { id, name }
                                            })
                                            .collect::<Vec<Homework>>());
                                        #[cfg(debug_assertions)]
                                        println!("{}::homeworks: {:?}", course.name, start.elapsed());
                                        break;
                                    }
                                },
                            }
                        },
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
        let handin_url =
            format!("https://courses.zju.edu.cn/api/course/activities/{homework_id}/submissions");

        if !comment.is_empty() {
            comment = format!("<p>{comment}<br></p>");
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
            if let Ok(json_unjudged) = serde_json::from_str::<Value>(&content) {
                #[cfg(debug_assertions)]
                println!("SUBMIT POST response as JSON: {:#?}", json_unjudged);
                if json_unjudged["errors"].is_array() {
                    return Err(anyhow!("上交作业失败"));
                }
                json = Some(json_unjudged);
                break;
            }
            #[cfg(debug_assertions)]
            warning!("retry {attempt}/{}: 上传请求失败", utils::MAX_RETRIES);
        }

        if json.is_none() {
            return Err(anyhow!("上传作业失败"));
        }
        #[cfg(debug_assertions)]
        process!("上交作业请求已被接受");
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
    pub fn get_grade(&self, account: &account::AccountData) -> Result<()> {
        let form = json!({
            "xh":account.stuid
        });
        begin!("查询成绩");
        let grade_json = self.query_grades(form)?;
        end!("查询成绩");

        let (xn_set, xq_set) =
            try_or_throw!(self.get_active_year_and_semester(), "获取活跃学年学期");
        let mut weight_sum = 0.0;
        let mut credit_sum = 0.0;
        let mut weight_sum_semester = 0.0;
        let mut credit_sum_semester = 0.0;
        let mut weight_sum_year = 0.0;
        let mut credit_sum_year = 0.0;
        let mut big_class_weight_sum = 0.0;
        let mut big_class_credit_sum = 0.0;
        let mut middle_class_weight_sum = 0.0;
        let mut middle_class_credit_sum = 0.0;
        let mut small_class_weight_sum = 0.0;
        let mut small_class_credit_sum = 0.0;
        let all_grade_list: Vec<Grade> = grade_json
            .iter()
            .filter_map(|grade_json| {
                let obj = grade_json.as_object()?;
                let grade = obj["cj"].as_str()?;
                if grade == "弃修" {
                    return None;
                }
                let name = obj["kcmc"].as_str()?;
                let xq = obj["xq"].as_str()?;
                let xn = obj["xn"].as_str()?;
                let credit = obj["xf"].as_str()?;
                let gpa = obj["jd"].as_f64()?;
                let gpa_str = format_gpa_str(gpa, 1);
                let credit_num: f64 = credit.parse().unwrap();
                let class_type = decide_class_type(credit_num);
                let name_str;
                match class_type {
                    Class::Big => {
                        name_str = format!("\x1b[35m{name}\x1b[0m");
                        big_class_weight_sum += gpa * credit_num;
                        big_class_credit_sum += credit_num;
                    }
                    Class::Middle => {
                        name_str = format!("\x1b[34m{name}\x1b[0m");
                        middle_class_weight_sum += gpa * credit_num;
                        middle_class_credit_sum += credit_num;
                    }
                    Class::Small => {
                        name_str = name.to_string();
                        small_class_weight_sum += gpa * credit_num;
                        small_class_credit_sum += credit_num;
                    }
                }
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
                Some(Grade {
                    name: name_str.to_string(),
                    grade: grade.to_string(),
                    credit: credit.to_string(),
                    gpa: gpa_str.to_string(),
                })
            })
            .collect();

        let avg_gpa = format_gpa_str(weight_sum / credit_sum, 2);
        let avg_gpa_semester = format_gpa_str(weight_sum_semester / credit_sum_semester, 2);
        let avg_gpa_year = format_gpa_str(weight_sum_year / credit_sum_year, 2);
        let avg_gpa_big_class = format_gpa_str(big_class_weight_sum / big_class_credit_sum, 2);
        let avg_gpa_middle_class =
            format_gpa_str(middle_class_weight_sum / middle_class_credit_sum, 2);
        let avg_gpa_small_class =
            format_gpa_str(small_class_weight_sum / small_class_credit_sum, 2);
        let table = create_table(&all_grade_list);
        println!("{table}");
        println!("学期均绩 | {avg_gpa_semester}/{credit_sum_semester:.1}");
        println!("学年均绩 | {avg_gpa_year}/{credit_sum_year:.1}");
        println!("总均绩   | {avg_gpa}/{credit_sum:.1}");
        println!("\x1b[35m大课均绩\x1b[0m | {avg_gpa_big_class}/{big_class_credit_sum:.1}");
        println!("\x1b[34m中课均绩\x1b[0m | {avg_gpa_middle_class}/{middle_class_credit_sum:.1}");
        println!("小课均绩 | {avg_gpa_small_class}/{small_class_credit_sum:.1}");

        Ok(())
    }

    /// 获取成绩 并打印本学期
    pub fn get_g(&self, account: &account::AccountData) -> Result<()> {
        let form = json!({
            "xh":account.stuid
        });
        begin!("查询成绩");
        let grade_json = self.query_grades(form)?;
        end!("查询成绩");

        let (xn_set, xq_set) =
            try_or_throw!(self.get_active_year_and_semester(), "获取活跃学年学期");
        let mut weight_sum = 0.0;
        let mut credit_sum = 0.0;
        let mut weight_sum_semester = 0.0;
        let mut credit_sum_semester = 0.0;
        let mut weight_sum_year = 0.0;
        let mut credit_sum_year = 0.0;
        let mut big_class_weight_sum = 0.0;
        let mut big_class_credit_sum = 0.0;
        let mut middle_class_weight_sum = 0.0;
        let mut middle_class_credit_sum = 0.0;
        let mut small_class_weight_sum = 0.0;
        let mut small_class_credit_sum = 0.0;

        let grade_list: Vec<Grade> = grade_json
            .iter()
            .filter_map(|grade_json| {
                let obj = grade_json.as_object()?;
                let grade = obj["cj"].as_str()?;
                if grade == "弃修" {
                    return None;
                }
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
                        let gpa_str = format_gpa_str(gpa, 1);
                        let class_type = decide_class_type(credit_num);
                        let name_str;
                        match class_type {
                            Class::Big => {
                                name_str = format!("\x1b[35m{name}\x1b[0m");
                                big_class_weight_sum += gpa * credit_num;
                                big_class_credit_sum += credit_num;
                            }
                            Class::Middle => {
                                name_str = format!("\x1b[34m{name}\x1b[0m");
                                middle_class_weight_sum += gpa * credit_num;
                                middle_class_credit_sum += credit_num;
                            }
                            Class::Small => {
                                name_str = name.to_string();
                                small_class_weight_sum += gpa * credit_num;
                                small_class_credit_sum += credit_num;
                            }
                        }
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
        let avg_gpa = format_gpa_str(weight_sum / credit_sum, 2);
        let avg_gpa_semester = format_gpa_str(weight_sum_semester / credit_sum_semester, 2);
        let avg_gpa_year = format_gpa_str(weight_sum_year / credit_sum_year, 2);
        let avg_gpa_big_class = format_gpa_str(big_class_weight_sum / big_class_credit_sum, 2);
        let avg_gpa_middle_class =
            format_gpa_str(middle_class_weight_sum / middle_class_credit_sum, 2);
        let avg_gpa_small_class =
            format_gpa_str(small_class_weight_sum / small_class_credit_sum, 2);
        let table = create_table(&grade_list);
        println!("{table}");
        println!("学期均绩     | {avg_gpa_semester}/{credit_sum_semester:.1}");
        println!("学年均绩     | {avg_gpa_year}/{credit_sum_year:.1}");
        println!("总均绩       | {avg_gpa}/{credit_sum:.1}");
        println!(
            "{} | {avg_gpa_big_class}/{big_class_credit_sum:.1}",
            purple!("学期大课均绩")
        );
        println!(
            "{} | {avg_gpa_middle_class}/{middle_class_credit_sum:.1}",
            blue!("学期中课均绩")
        );
        println!("学期小课均绩 | {avg_gpa_small_class}/{small_class_credit_sum:.1}");

        Ok(())
    }
    fn get_active_year_and_semester(&self) -> Result<(HashSet<String>, HashSet<String>)> {
        let active_semester_list = self.load_active_semesters()?;
        let (xn_set, xq_set): (HashSet<String>, HashSet<String>) = active_semester_list
            .iter()
            .map(|semester| split_semester(semester))
            .fold(
                (HashSet::new(), HashSet::new()),
                |(mut xn, mut xq), (a, b)| {
                    xn.insert(a.to_owned());
                    xq.insert(b.to_owned());
                    (xn, xq)
                },
            );
        Ok((xn_set, xq_set))
    }
    pub fn polling(&self, account: &account::AccountData) -> Result<()> {
        use crossterm::{
            event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
            terminal::{disable_raw_mode, enable_raw_mode},
        };

        enable_raw_mode().expect("failed to enable raw mode");
        let mut stdout = stdout();
        fn raw_println(str: &str, stdout: &mut impl Write) {
            print!("{str}\r\n");
            stdout.flush().unwrap();
        }
        fn alert(stdout: &mut impl Write) {
            for _ in 1..=3 {
                print!("\x07");
                stdout.flush().unwrap();
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        // 首次查询本学期已出成绩
        let form = json!({ "xh": account.stuid });
        let grade_json = self.query_grades(form.clone())?;
        let (xn_set, xq_set) =
            try_or_throw!(self.get_active_year_and_semester(), "获取活跃学年学期");
        let mut known_courses: HashSet<String> = HashSet::new();
        // 显示提示信息，让用户了解可通过 Ctrl+C 或 q 键退出
        raw_println("按 Ctrl + C / q / Esc 退出持续查询...", &mut stdout);
        raw_println(&gray!("( 课程 | 成绩 | 绩点 | 学分 )"), &mut stdout);
        for grade_value in grade_json.iter() {
            let Some(obj) = grade_value.as_object() else {
                continue;
            };
            let (xq, xn) = (obj["xq"].as_str().unwrap(), obj["xn"].as_str().unwrap());
            if !xn_set.contains(xn) || !xq_set.contains(xq) {
                continue;
            }
            // 忽略“弃修”成绩
            let grade = match obj.get("cj").and_then(|v| v.as_str()) {
                Some(s) if s != "弃修" => s,
                _ => continue,
            };

            let name = obj.get("kcmc").and_then(|v| v.as_str()).unwrap();
            let gpa = obj.get("jd").and_then(|v| v.as_f64()).unwrap();
            let credit = obj.get("xf").and_then(|v| v.as_str()).unwrap();
            let credit_num: f64 = credit.parse().unwrap();
            let class_type = decide_class_type(credit_num);
            let name_str;
            match class_type {
                Class::Big => {
                    name_str = purple!("{name}");
                }
                Class::Middle => {
                    name_str = blue!("{name}");
                }
                Class::Small => {
                    name_str = name.to_string();
                }
            }
            let gpa_str = format_gpa_str(gpa, 1);
            // 直接打印格式：课程名称 | 成绩 | 绩点 | 学分
            let width = (30 + width_shift(&name_str)) as usize;
            raw_println(
                &format!("{name_str:width$} | {grade} | {gpa_str} | {credit}"),
                &mut stdout,
            );
            known_courses.insert(name.to_string());
        }

        const TOTAL_SLEEP_TIME: Duration = if cfg!(debug_assertions) {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(600)
        };
        const SLEEP_INTERVAL: Duration = Duration::from_millis(200);

        // 循环查询，每 10 分钟检查一次
        'outer: loop {
            let mut elapsed = Duration::new(0, 0);
            while elapsed < TOTAL_SLEEP_TIME {
                // 使用 poll 检查按键事件，超时时间 SLEEP_INTERVAL
                if event::poll(SLEEP_INTERVAL).unwrap() {
                    if let Event::Key(KeyEvent { code, .. }) = event::read().unwrap() {
                        match code {
                            KeyCode::Char('q') => break 'outer,
                            KeyCode::Char('c')
                                if KeyModifiers::CONTROL == KeyModifiers::CONTROL =>
                            {
                                break 'outer
                            }
                            KeyCode::Esc => break 'outer,
                            _ => {}
                        }
                    }
                }
                elapsed += SLEEP_INTERVAL;
            }
            let new_grade_json = self.query_grades(form.clone())?;
            let mut found_new = false;
            for grade_value in new_grade_json.iter() {
                let Some(obj) = grade_value.as_object() else {
                    continue;
                };
                let (xq, xn) = (obj["xq"].as_str().unwrap(), obj["xn"].as_str().unwrap());
                if !xn_set.contains(xn) || !xq_set.contains(xq) {
                    continue;
                }
                // 忽略“弃修”
                let grade = match obj.get("cj").and_then(|v| v.as_str()) {
                    Some(s) if s != "弃修" => s,
                    _ => continue,
                };
                let name = obj.get("kcmc").and_then(|v| v.as_str()).unwrap_or("");
                if known_courses.contains(name) {
                    continue;
                }
                found_new = true;
                let gpa = obj.get("jd").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let credit = obj.get("xf").and_then(|v| v.as_str()).unwrap_or("");
                let credit_num: f64 = credit.parse().unwrap();
                let class_type = decide_class_type(credit_num);
                let name_str;
                match class_type {
                    Class::Big => {
                        name_str = purple!("!{name}");
                    }
                    Class::Middle => {
                        name_str = blue!("!{name}");
                    }
                    Class::Small => {
                        name_str = format!("!{name}");
                    }
                }
                let gpa_str = format_gpa_str(gpa, 1);
                // 直接打印格式：课程名称 | 成绩 | 绩点 | 学分
                let width = (30 + width_shift(&name_str)) as usize;

                raw_println(
                    &format!("{name_str:width$} | {grade} | {gpa_str} | {credit}"),
                    &mut stdout,
                );

                known_courses.insert(name.to_string());
                alert(&mut stdout);
            }
            #[cfg(debug_assertions)]
            if !found_new {
                alert(&mut stdout);
            }
        }
        disable_raw_mode().expect("failed to disable raw mode");
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
fn filter_latest_group(semesters: &[String], active_semester: &str) -> Vec<String> {
    let splitted = split_semester(active_semester);
    let max_year = parse_year_prefix(splitted.0);
    let max_group = suffix_order(splitted.1).0;
    let mut parsed = Vec::new();
    for sem in semesters {
        let (prefix, suffix) = split_semester(sem);
        let year = parse_year_prefix(prefix); // 返回 u32
        let (group, sub) = suffix_order(suffix); // 返回 (u8, u8)
        parsed.push((sem.clone(), year, group, sub));
    }

    let filtered: Vec<_> = parsed
        .into_iter()
        .filter(|(_, y, _, _)| *y == max_year)
        .collect();

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
        let padded = format!(" {header:total_width$} |");
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
            let padded = format!(" {col:total_width$} |");
            table.push_str(&padded);
        }
        table.push('\n');
        table.push_str(&separator);
    }
    table
}

fn format_gpa_str(gpa: f64, precision: usize) -> String {
    let formatted_gpa = format!("{gpa:.precision$}");
    match gpa {
        4.5..=5.0 => format!("\x1b[32m{formatted_gpa}\x1b[0m"), // 绿色
        3.5..4.5 => format!("\x1b[36m{formatted_gpa}\x1b[0m"),  // 青色
        2.4..3.5 => format!("\x1b[33m{formatted_gpa}\x1b[0m"),  // 黄色
        0.0..2.4 => format!("\x1b[31m{formatted_gpa}\x1b[0m"),  // 红色
        _ => formatted_gpa,                                     // 白色
    }
}

enum Class {
    Big,
    Middle,
    Small,
}
fn decide_class_type(credit_num: f64) -> Class {
    match credit_num {
        3.5..=7.0 => Class::Big,
        2.0..3.5 => Class::Middle,
        _ => Class::Small,
    }
}
