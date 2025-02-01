#[cfg(debug_assertions)]
use crate::error;
use crate::success;
use anyhow::Result;
use self_update::backends::github::Update;
use self_update::cargo_crate_version;
use std::thread::{self, JoinHandle};
/// 更新 zac
pub fn update() -> Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("CrazySpottedDove")
        .repo_name("zac")
        .bin_name("zac")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    if status.version() == cargo_crate_version!() {
        success!("已是最新版本");
    } else {
        success!("已更新至 {}", status.version());
        success!("请重新启动 zac 以体验新版 O(∩_∩)O~~");
    }
    Ok(())
}

/// 检查是否有新版本
pub fn check_update() -> JoinHandle<bool> {
    thread::spawn(move || {
        let updater = match Update::configure()
            .repo_owner("CrazySpottedDove")
            .repo_name("zac")
            .bin_name("zac")
            .current_version(cargo_crate_version!())
            .build()
        {
            Ok(updater) => updater,
            Err(e) => {
                #[cfg(debug_assertions)]
                error!("检查更新失败: {:?}", e);
                return false;
            }
        };

        let latest_release = match updater.get_latest_release() {
            Ok(release) => release,
            Err(e) => {
                #[cfg(debug_assertions)]
                error!("检查更新失败: {:?}", e);
                return false;
            }
        };

        latest_release.version != cargo_crate_version!()
    })
}
