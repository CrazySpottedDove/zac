use self_update::cargo_crate_version;
use anyhow::Result;

use crate::success;
pub fn update()->Result<()>{
    let status = self_update::backends::github::Update::configure()
        .repo_owner("CrazySpottedDove")
        .repo_name("zac")
        .bin_name("zac")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    success!("已更新至 {}",status.version());
    success!("请重新启动 zac 以体验新版 O(∩_∩)O~~");
    Ok(())
}