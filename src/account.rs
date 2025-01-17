use crate::{success, utils};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use anyhow::Result;
use anyhow::anyhow;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Account {
    pub stuid: String,
    pub password: String,
}

type Accounts = HashMap<String, Account>;

impl Account {
    /// 获取所有的已有账号!
    pub fn get_accounts(
        path_accounts: &PathBuf,
    ) -> Result<Accounts> {
        let data = fs::read_to_string(path_accounts)?;
        let accounts: Accounts = serde_json::from_str(&data)?;

        #[cfg(debug_assertions)]
        success!("已读取账号：{:?}", accounts);
        Ok(accounts)
    }

    /// 将账号写入配置文件
    fn write_accounts(path_accounts: &PathBuf, accounts: &Accounts) -> Result<()> {
        let json = serde_json::to_string(&accounts)?;
        fs::write(path_accounts, json)?;
        Ok(())
    }

    /// 添加一个账号并修改默认用户！
    pub fn add_account(
        path_accounts: &PathBuf,
        path_settings: &PathBuf,
        accounts: &mut Accounts,
        settings: &mut utils::Settings,
    ) -> Result<()> {
        let mut stuid = String::new();
        let mut password = String::new();
        let mut user = String::new();

        print!("请输入用户名：");
        io::stdout().flush()?;
        io::stdin().read_line(&mut user)?;
        let user = user.trim().to_string();

        print!("请输入学号：");
        io::stdout().flush()?;
        io::stdin().read_line(&mut stuid)?;
        let stuid = stuid.trim().to_string();

        print!("请输入密码：");
        io::stdout().flush()?;
        io::stdin().read_line(&mut password)?;
        let password = password.trim().to_string();

        let new_account = Account { stuid, password };
        accounts.insert(user.clone(), new_account);

        Account::write_accounts(path_accounts, accounts)?;

        success!("添加用户 {} -> {}", user, path_accounts.display());

        utils::Settings::set_default_user(settings, path_settings, &user)?;
        Ok(())
    }

    /// 获取默认账号!
    pub fn get_default_account(accounts: &Accounts, user: &str) -> Result<Account> {
        let account = accounts.get(user).ok_or(anyhow!("未找到用户"))?;

        success!("当前用户：{}", user);
        Ok(account.clone())
    }

    /// 删除一个账号并修改默认用户！
    pub fn remove_account(
        path_accounts: &PathBuf,
        path_settings: &PathBuf,
        accounts: &mut Accounts,
        settings: &mut utils::Settings,
        user: &str,
    ) -> Result<()> {
        accounts.remove(user).ok_or(anyhow!("未找到用户"))?;
        Account::write_accounts(path_accounts, accounts)?;

        success!("删除用户 {} -> {}", user, path_accounts.display());

        if settings.user == user {
            if accounts.is_empty(){
                utils::Settings::set_default_user(settings, path_settings, "")?;
            }
            else{
                let default_user = accounts.keys().next().unwrap().clone();
                utils::Settings::set_default_user(settings, path_settings, &default_user)?;
            }
        }
        Ok(())
    }
}
