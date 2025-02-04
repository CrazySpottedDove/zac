use crate::{error, try_or_exit, utils, warning};
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

const MAIN_COMMANDS: &[&str] = &[
    "help", "fetch", "submit", "upgrade", "config", "which", "grade", "task", "version", "h", "f",
    "s", "u", "c", "w", "g", "t", "v", "update",
];
const CONFIG_MAIN_COMMANDS: &[&str] = &[
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
        Cow::Owned(format!("\x1b[90m{hint}\x1b[0m")) // 90 是灰色
    }

    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // 检查输入是否与最近的补全项中的任何一个匹配
        let last_comps = self.last_completions.borrow();
        if last_comps.contains(&line.to_string()) {
            Cow::Owned(format!("\x1b[1;32m{line}\x1b[0m")) // 加粗并设置为绿色
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
    use std::fs;
    let path = PathBuf::from(input);

    // 如果是相对路径，则拼上当前工作目录
    let resolved_path = if path.is_absolute() {
        path
    } else {
        let current_dir = std::env::current_dir()?;
        current_dir.join(path)
    };

    // 如果路径已存在，且是文件则报错，否则正常返回其规范化路径
    if resolved_path.exists() {
        if resolved_path.is_file() {
            return Err(format!("{} 是文件，不是目录", resolved_path.display()).into());
        }
        Ok(resolved_path)
    } else {
        // 若路径不存在，则创建对应文件夹
        warning!("路径不存在，为您创建：{}", resolved_path.display());
        fs::create_dir_all(&resolved_path)?;
        Ok(resolved_path)
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
    let example = try_or_exit!(utils::get_config_path(), "获取配置文件路径");
    println!("示例路径：{}", example.display());
    loop {
        let readline = rl.readline("请输入存储路径：\n");
        match readline {
            Ok(line) => match resolve_path_folder(&line) {
                Ok(path) => {
                    return path.to_str().unwrap().to_string();
                }
                Err(e) => {
                    warning!("{e}");
                }
            },
            Err(rustyline::error::ReadlineError::Interrupted) => {
                warning!("强制中断");
                return "EXIT".to_string();
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                warning!("强制中断");
                return "EXIT".to_string();
            }
            Err(e) => {
                error!("读取路径：{e}");
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
        let readline = rl.readline("请输入文件(3GB 以内)路径(绝对或相对均可)：\n");
        match readline {
            Ok(line) => match resolve_path_file(&line) {
                Ok(path) => {
                    // 检查文件扩展名是否在允许的文件类型列表中
                    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                        if EXPECTED_FILE_TYPES.contains(&extension) {
                            return path;
                        } else {
                            warning!("不支持的文件类型: {extension}");
                        }
                    } else {
                        warning!("无法解析文件扩展名");
                    }
                }
                Err(e) => {
                    warning!("{e}");
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
                error!("读取路径：{e}");
            }
        }
    }
}

pub fn build_generic_editor(
    commands: CommandType,
) -> rustyline::Editor<GenericHelper, FileHistory> {
    let config = rustyline::Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .check_cursor_position(false)
        .build();
    let mut rl = Editor::with_config(config).expect("创建 rustyline Editor 失败");
    rl.set_helper(Some(GenericHelper {
        completer: GenericCompleter::new(commands),
    }));
    rl
}

pub enum CommandType {
    MainCommand,
    ConfigCommand,
}

pub struct GenericCompleter {
    commands: CommandType,
}

impl GenericCompleter {
    pub fn new(commands: CommandType) -> Self {
        Self { commands }
    }
}

impl Completer for GenericCompleter {
    type Candidate = String;
    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> RResult<(usize, Vec<Self::Candidate>)> {
        let commands = match self.commands {
            CommandType::MainCommand => MAIN_COMMANDS,
            CommandType::ConfigCommand => CONFIG_MAIN_COMMANDS,
        };
        let candidates: Vec<String> = commands
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

pub struct GenericHelper {
    completer: GenericCompleter,
}
impl Validator for GenericHelper {}
impl Highlighter for GenericHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // 使用 ANSI 转义序列设置提示的颜色
        Cow::Owned(format!("\x1b[90m{hint}\x1b[0m")) // 90 是灰色
    }
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let commands = match self.completer.commands {
            CommandType::MainCommand => MAIN_COMMANDS,
            CommandType::ConfigCommand => CONFIG_MAIN_COMMANDS,
        };
        // 检查输入是否完全匹配任何命令
        if commands.contains(&line) {
            // 使用绿色高亮完全匹配的输入
            Cow::Owned(format!("\x1b[1;32m{line}\x1b[0m")) // 加粗并设置为绿色
        } else {
            Cow::Borrowed(line)
        }
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'b str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Owned(format!("\x1b[1;34m{prompt}\x1b[0m")) // 加粗并设置为蓝色
    }
}
impl Hinter for GenericHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // 提取第一个候选项的剩余部分作为“Hint”
        if let Ok((start, candidates)) = self.completer.complete(line, pos, ctx) {
            if !candidates.is_empty() {
                let first = &candidates[0];
                let hint = &first[pos - start..];
                return Some(hint.to_string());
            }
        }
        None
    }
}

impl Completer for GenericHelper {
    type Candidate = String;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> RResult<(usize, Vec<String>)> {
        self.completer.complete(line, pos, ctx)
    }
}
impl Helper for GenericHelper {}
