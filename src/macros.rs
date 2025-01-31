/// 成功信息打印
#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{}  {}","✓".green() ,format!($($arg)*));
    })
}

/// 错误信息打印
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        use colored::*;
        eprintln!("{}  {}","✗".red() ,format!($($arg)*));
    })
}

/// 警告信息打印
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{}  {}","!".yellow() ,format!($($arg)*));
    })
}

/// 进程信息打印
#[macro_export]
macro_rules! process {
    ($($arg:tt)*) => ({
        use colored::*;
        println!("{}  {}","⚙".blue() ,format!($($arg)*));
    })
}

/// 需等待进程提示
#[macro_export]
macro_rules! waiting {
    ($($arg:tt)*) => ({
        println!("{} {}……","⌛" ,format!($($arg)*));
    })
}

#[macro_export]
macro_rules! begin {
    ($($arg:tt)*) => ({
        use std::io::Write;
        print!("{} {}","⌛" ,format!($($arg)*));
        std::io::stdout().flush().unwrap();
    })
}

#[macro_export]
macro_rules! end {
    ($($arg:tt)*) => ({
        use colored::*;
        use std::io::Write;
        #[cfg(not(debug_assertions))]
        print!("\r{}  {}\n","✓".green() ,format!($($arg)*));
        #[cfg(debug_assertions)]
        print!("{}  {}\n","✓".green() ,format!($($arg)*));
        std::io::stdout().flush().unwrap();
    })
}

/// 成功返回值，失败报 error
#[macro_export]
macro_rules! try_or_log {
    ($expr:expr, $msg:expr) => {{
        #[cfg(debug_assertions)]
        use std::time::Instant;

        #[cfg(debug_assertions)]
        let start = Instant::now();
        match $expr {
            Ok(val) => {
                #[cfg(debug_assertions)]
                {
                    let duration = start.elapsed();
                    println!("{}: {:?}", $msg, duration);
                }
                val
            }
            Err(e) => {
                error!("{}：{}", $msg, e);
                return;
            }
        }
    }};
}

/// 成功返回值，失败报 error
#[macro_export]
macro_rules! try_or_throw {
    ($expr:expr, $msg:expr) => {{
        #[cfg(debug_assertions)]
        use std::time::Instant;

        #[cfg(debug_assertions)]
        let start = Instant::now();

        match $expr {
            Ok(val) => {
                #[cfg(debug_assertions)]
                {
                    let duration = start.elapsed();
                    println!("{}: {:?}", $msg, duration);
                }
                val
            }
            Err(e) => {
                return Err(anyhow::anyhow!("{}：{}", $msg, e));
            }
        }
    }};
}

/// 成功返回值，失败崩溃
#[macro_export]
macro_rules! try_or_exit {
    ($expr:expr, $msg:expr) => {{
        #[cfg(debug_assertions)]
        use std::time::Instant;

        #[cfg(debug_assertions)]
        let start = Instant::now();
        match $expr {
            Ok(val) => {
                #[cfg(debug_assertions)]
                {
                    let duration = start.elapsed();
                    println!("{}: {:?}", $msg, duration);
                }
                val
            }
            Err(e) => {
                error!("{}：{}", $msg, e);
                std::process::exit(1);
            }
        }
    }};
}