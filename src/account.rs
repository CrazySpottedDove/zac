use crate::{error, success, try_or_exit, try_or_throw, utils, warning};
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Account {
    pub stuid: String,
    pub password: String,
}

type Accounts = HashMap<String, Account>;

impl Account {
    /// 获取所有的已有账号!
    pub fn get_accounts(path_accounts: &PathBuf) -> Result<Accounts> {
        let data = try_or_throw!(fs::read_to_string(path_accounts), "读取账号");
        let accounts: Accounts = try_or_throw!(serde_json::from_str(&data), "解析账号");

        #[cfg(debug_assertions)]
        success!("已读取账号：{:?}", accounts);
        Ok(accounts)
    }

    /// 将账号写入配置文件
    fn write_accounts(path_accounts: &PathBuf, accounts: &Accounts) -> Result<()> {
        let json = try_or_throw!(serde_json::to_string(&accounts), "序列化账号");
        try_or_throw!(fs::write(path_accounts, json), "写入账号");
        Ok(())
    }

    /// 添加一个账号并修改默认用户！
    pub fn add_account(
        path_accounts: &PathBuf,
        path_settings: &PathBuf,
        accounts: &mut Accounts,
        settings: &mut utils::Settings,
    ) -> Result<Account> {
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

        let new_account = Account { stuid, password };
        accounts.insert(user.clone(), new_account.clone());

        try_or_throw!(Account::write_accounts(path_accounts, accounts), "写入账号");

        success!("添加用户 {} -> {}", user, path_accounts.display());

        try_or_throw!(
            utils::Settings::set_default_user(settings, path_settings, &user),
            "设置默认用户"
        );
        Ok(new_account)
    }

    /// 获取默认账号!
    pub fn get_default_account(accounts: &Accounts, user: &str) -> Result<Account> {
        let account = accounts.get(user).ok_or(anyhow!("未找到用户"))?;

        success!("当前用户：{}", user);
        Ok(account.clone())
    }

    /// 删除一个账号并(如果需要的话)修改默认用户！
    /// 该函数保证至少有一个默认账号
    pub fn remove_account(
        path_accounts: &PathBuf,
        path_settings: &PathBuf,
        accounts: &mut Accounts,
        settings: &mut utils::Settings,
        user: &str,
    ) -> Option<Account> {
        accounts.remove(user);
        try_or_exit!(Account::write_accounts(path_accounts, accounts), "写入账号");

        success!("删除用户 {} -> {}", user, path_accounts.display());

        if settings.user != user {
            return None;
        }

        match accounts.keys().next() {
            Some(default_user) => {
                try_or_exit!(
                    utils::Settings::set_default_user(settings, path_settings, default_user),
                    "设置默认用户"
                );
                let default_account = accounts.get(default_user).unwrap().clone();
                Some(default_account)
            }
            None => {
                warning!("没有已知账号 => 添加账号");
                let default_account = try_or_exit!(
                    Account::add_account(path_accounts, path_settings, accounts, settings),
                    "添加账号"
                );
                Some(default_account)
            }
        }
    }
}
