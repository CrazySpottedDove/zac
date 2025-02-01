// filepath: /home/dove/CrazySpottedDove/zac/src/update.rs
// use crate::success;
// use anyhow::{Context, Result};
// use reqwest::blocking::Client;
// use semver::Version;
// use serde::Deserialize;
// use std::env;
// use std::fs;
// use std::fs::File;
// use std::io::copy;
// use std::io::Write;
// use std::path::{Path, PathBuf};
// use std::process::Command;


// #[derive(Deserialize)]
// struct Release {
//     tag_name: String,
//     assets: Vec<Asset>,
// }

// #[derive(Deserialize)]
// struct Asset {
//     name: String,
//     browser_download_url: String,
// }

// pub fn update() -> Result<()> {
//     let client = Client::builder()
//         .timeout(std::time::Duration::from_secs(1200))
//         .build()
//         .context("构建HTTP客户端失败")?;
//     let latest_release = fetch_latest_release(&client)?;
//     let current_version = env!("CARGO_PKG_VERSION");
//     let current_version = Version::parse(current_version).context("解析当前版本失败")?;
//     let latest_version = Version::parse(&latest_release.tag_name.trim_start_matches('v'))
//         .context("解析最新版本失败")?;

//     if latest_version > current_version {
//         println!("发现新版本: {}", latest_version);
//         let asset = select_asset(&latest_release.assets)?;
//         println!("下载资产: {}", asset.name);
//         let temp_path = download_asset(&client, &asset.browser_download_url, &asset.name)?;

//         replace_executable(&temp_path)?;
//         println!("更新成功到版本: {}", latest_version);
//     } else {
//         println!("当前已经是最新版本: {}", current_version);
//     }

//     Ok(())
// }

// fn fetch_latest_release(client: &Client) -> Result<Release> {
//     let url = "https://api.github.com/repos/CrazySpottedDove/zac/releases/latest";
//     let release = client
//         .get(url)
//         .header("User-Agent", "rust-self-update")
//         .send()
//         .context("请求最新发布版本失败")?
//         .json::<Release>()
//         .context("解析发布信息失败")?;
//     Ok(release)
// }

// fn select_asset(assets: &[Asset]) -> Result<&Asset> {
//     assets
//         .iter()
//         .find(|a| a.name == ASSET_NAME)
//         .context(format!("未找到匹配的资产: {}", ASSET_NAME))
// }

// fn download_asset(client: &Client, url: &str, filename: &str) -> Result<PathBuf> {
//     // 获取当前可执行文件的路径
//     let current_exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
//     let exe_dir = current_exe.parent().context("获取可执行文件父目录失败")?;

//     // 在可执行文件目录下创建临时下载路径，例如 "zac_new"
//     let temp_path = exe_dir.join(format!("{}.new", filename));

//     println!("开始下载资产: {}", url);
//     let response = client
//         .get(url)
//         .header("User-Agent", "rust-self-update")
//         .send()
//         .context("下载资产请求失败")?
//         .error_for_status()
//         .context("下载资产时发生HTTP错误")?;

//     println!("响应状态: {}", response.status());

//     // 创建目标文件
//     let mut file = File::create(&temp_path).context("创建临时文件失败")?;

//     // 读取响应体并写入文件
//     let content = response.bytes().context("读取下载内容失败")?;
//     let mut content = std::io::Cursor::new(content);
//     copy(&mut content, &mut file).context("写入临时文件失败")?;

//     println!("下载完成，保存在: {}", temp_path.display());
//     Ok(temp_path)
// }

// fn replace_executable(new_exe: &Path) -> Result<()> {
//     let current_exe = env::current_exe().context("获取当前可执行文件路径失败")?;
//     let exe_path_str = current_exe.to_str().ok_or_else(|| anyhow::anyhow!("无法将可执行文件路径转换为字符串"))?;
//     let new_exe_str = new_exe.to_str().ok_or_else(|| anyhow::anyhow!("无法将新可执行文件路径转换为字符串"))?;

//     #[cfg(target_os = "windows")]
//     {
//         // Windows 替换逻辑保持不变
//         let temp_exe = current_exe.with_extension("exe.new");
//         fs::rename(new_exe, &temp_exe).context("重命名新可执行文件失败")?;

//         // 创建一个批处理脚本来替换旧的可执行文件
//         let script = format!(
//             "cmd /C timeout /t 1 && move /Y \"{}\" \"{}\" && start \"\" \"{}\"",
//             temp_exe.display(),
//             current_exe.display(),
//             current_exe.display()
//         );
//         Command::new("cmd")
//             .args(&["/C", &script])
//             .spawn()
//             .context("执行替换命令失败")?;
//     }

//     #[cfg(not(target_os = "windows"))]
//     {
//         use std::os::unix::fs::PermissionsExt;
//         // 创建辅助更新脚本
//         let script_path = env::temp_dir().join("update.sh");
//         let mut script = fs::File::create(&script_path).context("创建辅助更新脚本失败")?;

//         let script_content = format!(
//             "#!/bin/sh
// sleep 1
// echo '开始替换可执行文件...'
// if [ \"$(id -u)\" -ne 0 ]; then
//     echo '需要管理员权限，请输入密码以继续...'
//     sudo cp \"{}\" \"{}\"
//     sudo chmod +x \"{}\"
// else
//     cp \"{}\" \"{}\"
//     chmod +x \"{}\"
// fi
// echo '替换完成，启动新版本...'
// \"{}\" &
// echo '新版本已启动。'
// rm -- \"$0\"
// ",
//             new_exe_str,
//             exe_path_str,
//             exe_path_str,
//             new_exe_str,
//             exe_path_str,
//             exe_path_str,
//             exe_path_str
//         );

//         script.write_all(script_content.as_bytes()).context("写入辅助更新脚本失败")?;
//         fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).context("设置辅助脚本可执行权限失败")?;

//         // 启动辅助脚本
//         Command::new(&script_path)
//             .spawn()
//             .context("启动辅助更新脚本失败")?;

//         // 退出主程序以释放可执行文件锁
//         std::process::exit(0);
//     }

//     // 对于 Windows，提示用户最新版本正在启动
//     #[cfg(target_os = "windows")]
//     {
//         println!("更新完成，正在启动新版本。");
//     }

//     Ok(())
// }


use self_update::cargo_crate_version;
use anyhow::Result;
pub fn update()->Result<()>{
    let status = self_update::backends::github::Update::configure()
        .repo_owner("CrazySpottedDove")
        .repo_name("zac")
        .bin_name("zac")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}