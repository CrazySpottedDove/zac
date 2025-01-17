use crate::{account, error, process, success, warning};
use ::serde::{Deserialize, Serialize};
use anyhow::anyhow;
use anyhow::Result;
use cookie_store::CookieStore;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

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

#[derive(Debug)]
struct State {
    // path_cookies: PathBuf,
    cookie_store: Arc<CookieStoreMutex>,
}

impl State {
    // pub fn try_new(path_cookies: PathBuf) -> anyhow::Result<State> {
    //     let cookie_store = match File::open(&path_cookies) {
    //         Ok(f) => CookieStore::load_json(BufReader::new(f)).map_err(|e| {
    //             let context = format!("error when read cookies from {}", path_cookies.display());
    //             anyhow::anyhow!("{}", e).context(context)
    //         })?,
    //         Err(e) => {
    //             warning!(
    //                 "open {} failed. error: {}, use default empty cookie store",
    //                 path_cookies.display(),
    //                 e
    //             );
    //             CookieStore::default()
    //         }
    //     };
    //     // // 打印所有加载的 Cookie
    //     // {
    //     //     println!("加载的 Cookie：{:?}", cookie_store);
    //     // }
    //     let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));
    //     Ok(State {
    //         path_cookies,
    //         cookie_store,
    //     })
    // }

    /// 建立新的 cookie_store
    pub fn try_new() -> anyhow::Result<State> {
        let cookie_store = Arc::new(CookieStoreMutex::new(CookieStore::default()));
        Ok(State { cookie_store })
    }
}

// impl Drop for State {
//     fn drop(&mut self) {
//         let mut file = match fs::OpenOptions::new()
//             .write(true)
//             .create(true)
//             .truncate(true)
//             .open(&self.path_cookies)
//         {
//             Ok(f) => f,
//             Err(e) => {
//                 error!(
//                     "open {} for write failed. error: {}",
//                     self.path_cookies.display(),
//                     e
//                 );
//                 return;
//             }
//         };
//         let store = self.cookie_store.lock().unwrap();
//         if let Err(e) = store.save_json(&mut file) {
//             error!(
//                 "save cookies to path {} failed. error: {}",
//                 self.path_cookies.display(),
//                 e
//             );
//         }
//     }
// }

#[derive(Debug, Clone)]
pub struct Session {
    #[allow(dead_code)] // just make clippy happy
    state: Arc<State>,
    client: Client,
}

impl Session {
    //  pub fn try_new(path_cookies: PathBuf) -> anyhow::Result<Session> {
    //     let state = State::try_new()?;
    //     let state = Arc::new(state);
    //     let mut headers = HeaderMap::new();
    //     headers.insert(
    //         USER_AGENT,
    //         "Mozilla/5.0 (X11; Linux x86_64; rv:88.0) Gecko/20100101 Firefox/88.0"
    //             .parse()
    //             .unwrap(),
    //     );
    //     let client = Client::builder()
    //         .cookie_provider(state.cookie_store.clone())
    //         .default_headers(headers)
    //         .build()?;
    //     Ok(Session { state, client })
    // }

    /// 建立新的会话!
    pub fn try_new() -> Result<Session> {
        let state = State::try_new()?;
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
            .build()?;

        success!("建立会话");
        Ok(Session { state, client })
    }

    /// 登录!
    pub fn login(&self, account: &account::Account) -> Result<()> {
        process!("登录……");
        let login_url = "https://zjuam.zju.edu.cn/cas/login";
        let res = self.client.get(login_url).send()?;
        let text = res.text()?;
        let re =
            regex::Regex::new(r#"<input type="hidden" name="execution" value="(.*?)" />"#).unwrap();
        let execution = re
            .captures(&text)
            .and_then(|cap| cap.get(1).map(|m| m.as_str()))
            .ok_or(anyhow!("Execution value not found"))?;
        let res = self
            .get("https://zjuam.zju.edu.cn/cas/v2/getPubKey")
            .send()?;

        let json: Value = res.json()?;
        let modulus = json["modulus"]
            .as_str()
            .ok_or(anyhow!("Modulus not found"))?;
        let exponent = json["exponent"]
            .as_str()
            .ok_or(anyhow!("Exponent not found"))?;

        let rsapwd = rsa_no_padding(&account.password, modulus, exponent);

        let params = [
            ("username", account.stuid.as_str()),
            ("password", &rsapwd),
            ("execution", execution),
            ("_eventId", "submit"),
            ("authcode", ""),
        ];
        let res = self.client.post(login_url).form(&params).send()?;
        if res.status().is_success() {
            success!("登录");
            Ok(())
        } else {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            error!("登录状态码：{}，响应内容：{}", status, text);
            Err(anyhow!("登录失败"))
        }
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
        let res = self.client.get("https://courses.zju.edu.cn/api/my-courses?conditions=%7B%22status%22:%5B%22ongoing%22,%22notStarted%22%5D,%22keyword%22:%22%22,%22classify_type%22:%22recently_started%22,%22display_studio_list%22:false%7D&fields=id,name,semester_id&page=1&page_size=1000&showScorePassedStatus=false").send()?;

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

    /// 存储学期-课程映射表!
    pub fn store_semester_course_map(
        path_courses: &PathBuf,
        course_list: Vec<Course>,
        semester_map: HashMap<u64, String>,
    ) -> Result<()> {
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
        std::fs::write(
            path_courses,
            serde_json::to_string(&semester_course_map).unwrap(),
        )?;

        success!("存储 学期->课程 映射表");
        Ok(())
    }

    /// 加载学期-课程映射表!
    pub fn load_semester_course_map(
        path_courses: &PathBuf,
    ) -> Result<HashMap<String, Vec<CourseData>>> {
        let data = fs::read_to_string(path_courses)?;
        let semester_course_map: HashMap<String, Vec<CourseData>> = serde_json::from_str(&data)?;

        success!("加载 学期->课程 映射表");
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

        success!("存储已选课程");
        Ok(())
    }

    /// 加载已选课程!
    pub fn load_selected_courses(path_selected_courses: &PathBuf) -> Result<Vec<CourseFull>> {
        let data = fs::read_to_string(path_selected_courses)?;
        let selected_courses: Vec<CourseFull> = serde_json::from_str(&data)?;

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

        success!("加载已下载课件记录");
        Ok(activity_upload_record)
    }

    /// 拉取活动！
    fn fetch_activities(&self, selected_course: &CourseFull) -> Result<Vec<Value>> {
        const MAX_RETRIES: usize = 3;
        let url = format!(
            "https://courses.zju.edu.cn/api/courses/{}/activities",
            selected_course.id
        );
        for attempt in 1..=MAX_RETRIES {
            match self.client.get(&url).send() {
                Ok(res) => match res.json::<Value>() {
                    Ok(json) => {
                        if let Some(activities) = json["activities"].as_array() {
                            #[cfg(debug_assertions)]
                            success!("{} :: activities",selected_course.name);

                            return Ok(activities.clone());
                        } else {
                            warning!(
                                "retry {}/{}: {} 的返回 json 无 activities 字段",
                                attempt,
                                MAX_RETRIES,
                                selected_course.name
                            );
                        }
                    }
                    Err(e) => {
                        warning!(
                            "retry {}/{}: {} 的返回无法解析为 json: {}",
                            attempt,
                            MAX_RETRIES,
                            selected_course.name,
                            e
                        );
                    }
                },
                Err(e) => {
                    warning!(
                        "retry {}/{}: {} 的请求失败: {}",
                        attempt,
                        MAX_RETRIES,
                        selected_course.name,
                        e
                    );
                }
            }
        }
        Err(anyhow!("{} 的请求失败", selected_course.name))
    }

    /// 拉取新课件！
    pub fn fetch_activity_uploads(
        &self,
        path_download: &PathBuf,
        path_activity_upload_record: &PathBuf,
        selected_courses: Vec<CourseFull>,
        mut activity_upload_record: Vec<u64>,
        is_pdf: bool,
    ) -> Result<()> {
        process!("拉取新课件……");
        let mut tasks = Vec::new();

        for selected_course in selected_courses {
            let activities = self.fetch_activities(&selected_course)?;

            for activity in activities {
                let uploads = activity["uploads"].as_array().unwrap();
                for upload in uploads {
                    if let Some(id) = upload["reference_id"].as_u64() {
                        if activity_upload_record.contains(&id) {
                            continue;
                        }
                        let name = upload["name"].as_str().unwrap_or("unnamed").to_string();
                        tasks.push((
                            selected_course.semester.clone(),
                            selected_course.name.clone(),
                            id,
                            name,
                        ));
                    }
                }
            }
        }

        // 用自定义线程池将并发限制为 4
        let pool = ThreadPoolBuilder::new().num_threads(4).build()?;
        pool.install(|| {
            let successful_uploads: Vec<u64> = tasks
                .par_iter()
                .filter_map(|(semester, course_name, upload_id, file_name)| {
                    #[cfg(debug_assertions)]
                    process!("{} :: {}",course_name,file_name);

                    match Session::download_upload(
                        self,
                        &path_download.join(semester).join(course_name),
                        *upload_id,
                        file_name,
                        is_pdf,
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

        Ok(())
    }

    /// 下载一个upload文件！
    pub fn download_upload(
        &self,
        path_download: &PathBuf,
        id: u64,
        name: &str,
        is_pdf: bool,
    ) -> Result<()> {
        let download_url = match is_pdf {
            true => {
                let mut retries = 0;
                let url;
                loop {
                    let json:Value=self.get(format!("https://courses.zju.edu.cn/api/uploads/reference/document/{}/url?preview=true",id)).send()?.json().or_else(|e| {
                        error!("json失败：{}", e);
                        Err(e)
                    })?;

                    if json["status"].as_str().unwrap() == "ready" {
                        url = json["url"].as_str().unwrap().to_string();
                        break;
                    }

                    retries += 1;
                    if retries == 3 {
                        error!("雪灾浙大一直准备不好 {}", name);
                        return Ok(());
                    }
                }
                url
            }
            false => {
                let url = format!(
                    "https://courses.zju.edu.cn/api/uploads/reference/{}/blob",
                    id
                );
                url
            }
        };

        let mut res = self.get(&download_url).send()?;

        fs::create_dir_all(std::path::Path::new(path_download))?;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
