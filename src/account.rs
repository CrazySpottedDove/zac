use crate::{success, try_or_throw, utils, warning};
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AccountData {
    pub stuid: String,
    pub password: String,
}
pub type Accounts = HashMap<String, AccountData>;
pub struct Account {
    pub accounts: Accounts,
    pub path_accounts: PathBuf,
    pub default: AccountData,
}

impl Account {
    pub fn init(path_accounts: PathBuf, settings: &mut utils::Settings) -> Result<Self> {
        let mut accounts = Self::load_accounts(&path_accounts)?;

        // 处理没有已知账号的情况
        let default = if accounts.is_empty() {
            warning!("未发现已知的账号 => 创建账号");
            let Ok((new_account, user)) = Self::read_in_account() else {
                return Err(anyhow!("输入账号"));
            };
            accounts.insert(user.clone(), new_account.clone());
            let json = try_or_throw!(serde_json::to_string(&accounts), "序列化账号");
            try_or_throw!(fs::write(&path_accounts, json), "写入账号");
            try_or_throw!(settings.set_default_user(&user), "设置默认用户");
            new_account
        } else {
            let Some(default) = accounts.get(&settings.user) else {
                return Err(anyhow!("找不到账号 {}", settings.user));
            };
            default.clone()
        };

        Ok(Account {
            accounts,
            path_accounts,
            default,
        })
    }

    /// 获取所有的已有账号
    fn load_accounts(path_accounts: &PathBuf) -> Result<Accounts> {
        let data = try_or_throw!(fs::read_to_string(path_accounts), "读取账号");
        let accounts: Accounts = try_or_throw!(serde_json::from_str(&data), "解析账号");

        #[cfg(debug_assertions)]
        success!("已读取账号：{:?}", accounts);
        Ok(accounts)
    }

    /// 通过输入获得一个 AccountData 和对应用户名 String
    fn read_in_account() -> Result<(AccountData, String)> {
        let mut stuid = String::new();
        let mut password = String::new();
        let mut user = String::new();
        println!("账号：(用户名，学号，密码)");
        print!("请输入用户名：");
        try_or_throw!(io::stdout().flush(), "刷新 stdout");
        try_or_throw!(io::stdin().read_line(&mut user), "读取用户名");
        let user = user.trim().to_string();

        print!("请输入学号：");
        try_or_throw!(io::stdout().flush(), "刷新 stdout");
        try_or_throw!(io::stdin().read_line(&mut stuid), "读取学号");
        let stuid = stuid.trim().to_string();

        print!("请输入密码：");
        try_or_throw!(io::stdout().flush(), "刷新 stdout");
        try_or_throw!(io::stdin().read_line(&mut password), "读取密码");
        let password = password.trim().to_string();

        Ok((AccountData { stuid, password }, user))
    }

    /// 添加一个账号，并将此用户修改为默认用户
    pub fn add_account(&mut self, settings: &mut utils::Settings) -> Result<()> {
        let Ok((new_account, user)) = Self::read_in_account() else {
            return Err(anyhow!("输入账号"));
        };
        self.accounts.insert(user.clone(), new_account);

        let json = try_or_throw!(serde_json::to_string(&self.accounts), "序列化账号");
        try_or_throw!(fs::write(&self.path_accounts, json), "写入账号");
        try_or_throw!(settings.set_default_user(&user), "设置默认用户");

        Ok(())
    }

    /// 删除一个账号并(如果需要的话)修改默认用户！
    /// 该函数保证至少有一个默认账号
    pub fn remove_account(&mut self, settings: &mut utils::Settings, user: &str) -> Result<bool> {
        self.accounts.remove(user);
        let json = try_or_throw!(serde_json::to_string(&self.accounts), "序列化账号");
        try_or_throw!(fs::write(&self.path_accounts, json), "写入账号");
        success!("删除用户 {} -> {}", user, self.path_accounts.display());

        if settings.user != user {
            return Ok(false);
        }

        match self.accounts.keys().next() {
            Some(default_user) => {
                try_or_throw!(settings.set_default_user(default_user), "设置默认用户");
                self.default = self.accounts.get(default_user).unwrap().clone();
            }
            None => {
                warning!("没有已知账号 => 添加账号");
                try_or_throw!(self.add_account(settings), "添加账号");
            }
        }

        Ok(true)
    }
}
