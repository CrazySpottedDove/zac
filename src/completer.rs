use crate::{error, warning};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::config::CompletionType;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::{DefaultHistory, FileHistory};
use rustyline::validate::Validator;
use rustyline::Result as RResult;
use rustyline::{Context, Editor, Helper};
use std::borrow::Cow;
use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;

const COMMANDS: &[&str] = &[
    "help", "fetch", "submit", "upgrade", "config", "which", "grade", "task", "h", "f", "s", "u",
    "c", "w", "g", "t",
];
const CONFIG_COMMANDS: &[&str] = &[
    "help",
    "add-account",
    "remove-account",
    "user-default",
    "storage-dir",
    "mp4-trashed",
    "pdf-or-ppt",
    "list-config",
    "a",
    "r",
    "u",
    "s",
    "m",
    "p",
    "l",
    "h",
];
/// 用于文件名（路径）补全的 Helper 结构
struct FilePathHelper {
    completer: FilenameCompleter,
    last_completions: RefCell<Vec<String>>,
}

impl Completer for FilePathHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        // 直接使用 FilenameCompleter 的补全结果
        let (start, candidates) = self.completer.complete(line, pos, ctx)?;

        // 更新 last_completions，用于高亮逻辑
        let mut last = self.last_completions.borrow_mut();
        last.clear();
        for pair in &candidates {
            last.push(pair.replacement.clone());
        }

        Ok((start, candidates))
    }
}

impl Hinter for FilePathHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // 提取第一个候选项的剩余部分作为“Hint”
        if let Ok((start, candidates)) = self.completer.complete(line, pos, ctx) {
            if !candidates.is_empty() {
                let first = &candidates[0].replacement;
                let hint = &first[pos - start..];
                Some(hint.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Highlighter for FilePathHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // 使用 ANSI 转义序列设置提示的颜色
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint)) // 90 是灰色
    }

    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // 检查输入是否与最近的补全项中的任何一个匹配
        let last_comps = self.last_completions.borrow();
        if last_comps.contains(&line.to_string()) {
            Cow::Owned(format!("\x1b[1;32m{}\x1b[0m", line)) // 加粗并设置为绿色
        } else {
            Cow::Borrowed(line)
        }
    }
}
impl Validator for FilePathHelper {}
impl Helper for FilePathHelper {}

/// 解析并验证文件路径，确保路径存在且是文件
fn resolve_path_file(input: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = PathBuf::from(input);

    let canonical_path = if path.is_absolute() {
        path.canonicalize()?
    } else {
        let current_dir = std::env::current_dir()?;
        current_dir.join(path).canonicalize()?
    };

    if canonical_path.is_file() {
        Ok(canonical_path)
    } else {
        Err(format!("{} 也许不是文件？", canonical_path.display()).into())
    }
}

/// 解析并验证文件路径，确保路径存在且是文件夹
fn resolve_path_folder(input: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = PathBuf::from(input);

    let canonical_path = if path.is_absolute() {
        path.canonicalize()?
    } else {
        let current_dir = std::env::current_dir()?;
        current_dir.join(path).canonicalize()?
    };

    if canonical_path.is_file() {
        Err(format!("{} 是文件，不是路径", canonical_path.display()).into())
    } else {
        Ok(canonical_path)
    }
}

pub fn readin_storage_dir() -> String {
    // 使用 “列表” 补全类型，让用户可预览到所有可能的补全项
    let config = rustyline::Config::builder()
        .completion_type(CompletionType::List)
        .check_cursor_position(false)
        .build();
    // 创建 Editor，使用自定义的 FilePathHelper
    let mut rl = Editor::<FilePathHelper, DefaultHistory>::with_config(config)
        .expect("Fail to create editor");
    rl.set_helper(Some(FilePathHelper {
        completer: FilenameCompleter::new(),
        last_completions: RefCell::new(Vec::new()),
    }));
    loop {
        let readline = rl.readline("请输入存储路径：\n");
        match readline {
            Ok(line) => match resolve_path_folder(&line) {
                Ok(path) => {
                    return path.to_str().unwrap().to_string();
                }
                Err(e) => {
                    warning!("{}", e);
                }
            },
            Err(rustyline::error::ReadlineError::Interrupted) => {
                warning!("强制中断");
                std::process::exit(0);
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                warning!("强制中断");
                std::process::exit(0);
            }
            Err(e) => {
                error!("读取路径：{}", e);
            }
        }
    }
}

pub fn readin_path() -> PathBuf {
    // 使用 “列表” 补全类型，让用户可预览到所有可能的补全项
    let config = rustyline::Config::builder()
        .completion_type(CompletionType::List)
        .check_cursor_position(false)
        .build();
    const EXPECTED_FILE_TYPES: &[&str] = &[
        "avi", "flv", "m4v", "mov", "mp4", "3gp", "3gpp", "mpg", "rm", "rmvb", "swf", "webm",
        "wmv", "mp3", "m4a", "wav", "wma", "jpeg", "jpg", "png", "gif", "bmp", "heic", "webp",
        "txt", "pdf", "csv", "xls", "xlsx", "doc", "ppt", "pptx", "docx", "odp", "ods", "odt",
        "rtf", "zip", "rar", "tar", "mat", "dwg", "m", "mlapp", "slx", "mlx",
    ];
    // 创建 Editor，使用自定义的 FilePathHelper
    let mut rl = Editor::<FilePathHelper, DefaultHistory>::with_config(config)
        .expect("Fail to create editor");
    rl.set_helper(Some(FilePathHelper {
        completer: FilenameCompleter::new(),
        last_completions: RefCell::new(Vec::new()),
    }));
    loop {
        let readline = rl.readline("请输入文件(3GB 以内)路径：\n");
        match readline {
            Ok(line) => match resolve_path_file(&line) {
                Ok(path) => {
                    // 检查文件扩展名是否在允许的文件类型列表中
                    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                        if EXPECTED_FILE_TYPES.contains(&extension) {
                            return path;
                        } else {
                            warning!("不支持的文件类型: {}", extension);
                        }
                    } else {
                        warning!("无法解析文件扩展名");
                    }
                }
                Err(e) => {
                    warning!("{}", e);
                }
            },
            Err(rustyline::error::ReadlineError::Interrupted) => {
                warning!("中断 submit");
                return PathBuf::new();
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                warning!("中断 submit");
                return PathBuf::new();
            }
            Err(e) => {
                error!("读取路径：{}", e);
            }
        }
    }
}

#[derive(Default)]
struct CmdCompleter;
impl Completer for CmdCompleter {
    type Candidate = String;
    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> RResult<(usize, Vec<String>)> {
        let candidates: Vec<String> = COMMANDS
            .iter()
            .filter_map(|cmd| {
                if cmd.starts_with(line) {
                    Some(cmd.to_string())
                } else {
                    None
                }
            })
            .collect();
        // 从行首开始替换
        Ok((0, candidates))
    }
}

pub struct CommandHelper {
    completer: CmdCompleter,
}

impl Helper for CommandHelper {}
impl Validator for CommandHelper {}
impl Highlighter for CommandHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // 使用 ANSI 转义序列设置提示的颜色
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint)) // 90 是灰色
    }

    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // 检查输入是否完全匹配任何命令
        if COMMANDS.contains(&line) {
            // 使用绿色高亮完全匹配的输入
            Cow::Owned(format!("\x1b[1;32m{}\x1b[0m", line)) // 加粗并设置为绿色
        } else {
            Cow::Borrowed(line)
        }
    }
}
impl Completer for CommandHelper {
    type Candidate = String;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> RResult<(usize, Vec<String>)> {
        self.completer.complete(line, pos, ctx)
    }
}
impl Hinter for CommandHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // 提取第一个候选项的剩余部分作为“Hint”
        if let Ok((start, candidates)) = self.completer.complete(line, pos, ctx) {
            if !candidates.is_empty() {
                let first = &candidates[0];
                let hint = &first[pos - start..];
                Some(hint.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct CommandEditor;

impl CommandEditor {
    pub fn build() -> rustyline::Editor<CommandHelper, FileHistory> {
        let config = rustyline::Config::builder()
            .completion_type(rustyline::CompletionType::List)
            .check_cursor_position(false)
            .build();
        let mut rl = Editor::with_config(config).expect("创建 rustyline Editor 失败");
        rl.set_helper(Some(CommandHelper {
            completer: CmdCompleter::default(),
        }));
        rl
    }
}

#[derive(Default)]
struct CfgCompleter;
impl Completer for CfgCompleter {
    type Candidate = String;
    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> RResult<(usize, Vec<String>)> {
        let candidates: Vec<String> = CONFIG_COMMANDS
            .iter()
            .filter_map(|cfg| {
                if cfg.starts_with(line) {
                    Some(cfg.to_string())
                } else {
                    None
                }
            })
            .collect();
        // 从行首开始替换
        Ok((0, candidates))
    }
}

pub struct ConfigHelper {
    completer: CfgCompleter,
}

impl Helper for ConfigHelper {}
impl Validator for ConfigHelper {}
impl Highlighter for ConfigHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // 使用 ANSI 转义序列设置提示的颜色
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint)) // 90 是灰色
    }
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // 检查输入是否完全匹配任何命令
        if CONFIG_COMMANDS.contains(&line) {
            // 使用绿色高亮完全匹配的输入
            Cow::Owned(format!("\x1b[1;32m{}\x1b[0m", line)) // 加粗并设置为绿色
        } else {
            Cow::Borrowed(line)
        }
    }
}
impl Completer for ConfigHelper {
    type Candidate = String;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> RResult<(usize, Vec<String>)> {
        self.completer.complete(line, pos, ctx)
    }
}
impl Hinter for ConfigHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // 提取第一个候选项的剩余部分作为“Hint”
        if let Ok((start, candidates)) = self.completer.complete(line, pos, ctx) {
            if !candidates.is_empty() {
                let first = &candidates[0];
                let hint = &first[pos - start..];
                Some(hint.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct ConfigEditor;

impl ConfigEditor {
    pub fn build() -> rustyline::Editor<ConfigHelper, FileHistory> {
        let config = rustyline::Config::builder()
            .completion_type(rustyline::CompletionType::List)
            .check_cursor_position(false)
            .build();
        let mut rl = Editor::with_config(config).expect("创建 rustyline Editor 失败");
        rl.set_helper(Some(ConfigHelper {
            completer: CfgCompleter::default(),
        }));
        rl
    }
}
