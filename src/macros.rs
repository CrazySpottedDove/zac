/// 成功信息打印
#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => ({
        println!("\x1b[32m✓\x1b[0m  {}" ,format!($($arg)*));
    })
}

/// 错误信息打印
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        eprintln!("\x1b[31m✗\x1b[0m  {}",format!($($arg)*));
    })
}

/// 警告信息打印
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => ({
        println!("\x1b[33m!\x1b[0m  {}" ,format!($($arg)*));
    })
}

/// 进程信息打印
#[macro_export]
macro_rules! process {
    ($($arg:tt)*) => ({
        println!("\x1b[34m⚙\x1b[0m  {}" ,format!($($arg)*));
    })
}

/// 需等待进程提示
#[macro_export]
macro_rules! waiting {
    ($($arg:tt)*) => ({
        println!("⌛ {}……" ,format!($($arg)*));
    })
}

#[macro_export]
macro_rules! begin {
    ($($arg:tt)*) => ({
        use std::io::Write;
        print!("⌛ {}" ,format!($($arg)*));
        std::io::stdout().flush().unwrap();
    })
}

#[macro_export]
macro_rules! end {
    ($($arg:tt)*) => ({
        use std::io::Write;
        #[cfg(not(debug_assertions))]
        print!("\r\x1b[32m✓\x1b[0m  {}\n",format!($($arg)*));
        #[cfg(debug_assertions)]
        print!("\x1b[32m✓\x1b[0m  {}\n",format!($($arg)*));
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

#[macro_export]
macro_rules! red {
    ($($arg:tt)*) => {{
        format!("\x1B[31m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! green {
    ($($arg:tt)*) => {{
        format!("\x1B[32m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! yellow {
    ($($arg:tt)*) => {{
        format!("\x1B[33m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! blue {
    ($($arg:tt)*) => {{
        format!("\x1B[34m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! purple {
    ($($arg:tt)*) => {{
        format!("\x1B[35m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! cyan {
    ($($arg:tt)*) => {{
        format!("\x1B[36m{}\x1B[0m", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! gray {
    ($($arg:tt)*) => {{
        format!("\x1B[90m{}\x1B[0m", format!($($arg)*))
    }};
}

