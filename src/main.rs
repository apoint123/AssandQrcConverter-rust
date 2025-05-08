//
//
//  注释使用 AI 生成
//
//


// --- 依赖引入 ---
use clap::{CommandFactory, Parser}; // 用于命令行参数解析
use once_cell::sync::Lazy; // 用于惰性初始化静态变量 (如 Regex)
use regex::Regex; // 用于正则表达式操作
use std::error::Error; // 标准库错误处理 Trait
use std::fmt; // 标准库格式化 Trait
use std::fs::File; // 文件操作
use std::io::{self, BufRead, BufReader, BufWriter, Write}; // 输入输出流相关
use std::num::ParseIntError; // 整数解析错误类型
use std::path::{Path, PathBuf}; // 文件路径处理
use std::collections::HashMap;

// --- 常量定义 ---

// 文件扩展名常量
const ASS_EXTENSION: &str = ".ass";
const QRC_EXTENSION: &str = ".qrc";
const LYRICIFY_EXTENSION: &str = ".lys";

// 用户交互信息常量
const INVALID_CHOICE_MESSAGE: &str = "无效选择";
const INPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: "; // {} 会被替换为具体格式
const OUTPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: ";
const EMPTY_FILE_PATH_ERROR: &str = "输入的 {} 文件路径不能为空";
const FILE_NOT_FOUND_ERROR: &str = "错误: 输入文件不存在";

// 转换完成提示信息
const ASS_TO_QRC_COMPLETE: &str = "ASS -> QRC 转换完成！\n";
const QRC_TO_ASS_COMPLETE: &str = "QRC -> ASS 转换完成！\n";
const ASS_TO_LYS_COMPLETE: &str = "ASS -> Lyricify Syllable 转换完成！\n";
const LYS_TO_ASS_COMPLETE: &str = "Lyricify Syllable -> ASS 转换完成！\n";

// 错误信息模板
const CONVERSION_ERROR_MSG: &str = "转换过程中发生错误: {}"; // {} 会被具体错误信息替换

// 交互模式选项常量
const ASS_FORMAT_CHOICE: &str = "1";
const QRC_FORMAT_CHOICE: &str = "2";
const LYS_FORMAT_CHOICE: &str = "3";

// 时间相关计算常量 (基础单位: 毫秒)
const MILLISECONDS_PER_SECOND: usize = 1000;
const MILLISECONDS_PER_MINUTE: usize = 60 * MILLISECONDS_PER_SECOND;
const MILLISECONDS_PER_HOUR: usize = 60 * MILLISECONDS_PER_MINUTE;
/// ASS 时间码中的厘秒 (cs) 转换为毫秒的系数
const CENTISECONDS_TO_MILLISECONDS: usize = 10;
/// ASS 卡拉OK标签 {\kX} 中的 X (厘秒) 转换为毫秒的乘数
const K_TAG_MULTIPLIER: usize = 10;

// 进度条显示相关常量
const PROGRESS_BAR_LENGTH: usize = 20; // 进度条的字符显示长度
const PROGRESS_BAR_THRESHOLD: usize = 64 * 1024 * 1024; // 64MB, 文件小于此大小时不显示进度条

// 终端输出颜色 ANSI 转义码
const RESET: &str = "\x1b[0m"; // 重置颜色
const RED: &str = "\x1b[31m";   // 红色 (通常用于错误)
const GREEN: &str = "\x1b[32m"; // 绿色 (通常用于成功)
const YELLOW: &str = "\x1b[33m";// 黄色 (通常用于警告)
const CYAN: &str = "\x1b[36m";  // 青色 (通常用于提示信息)

// Lyricify Syllable (.lys) 属性常量定义
const LYS_PROPERTY_UNSET: usize = 0; // 默认对齐
const LYS_PROPERTY_LEFT: usize = 1; // 仅左对齐
const LYS_PROPERTY_RIGHT: usize = 2; // 仅右对齐
// const LYS_PROPERTY_NO_BACK_UNSET: usize = 3; // (未使用)
const LYS_PROPERTY_NO_BACK_LEFT: usize = 4; // 无背景，左对齐 (对应 ASS Name="左")
const LYS_PROPERTY_NO_BACK_RIGHT: usize = 5; // 无背景，右对齐 (对应 ASS Name="右")
const LYS_PROPERTY_BACK_UNSET: usize = 6; // 有背景，对齐方式待定 (对应 ASS Name="背"，需看前一行)
const LYS_PROPERTY_BACK_LEFT: usize = 7; // 有背景，左对齐 (对应 ASS Name="背" 且前一行是 "左")
const LYS_PROPERTY_BACK_RIGHT: usize = 8; // 有背景，右对齐 (对应 ASS Name="背" 且前一行是 "右")

// --- 日志宏定义 ---
// 简化带颜色和前缀的日志输出

macro_rules! log_info {
    // 接受任意格式化参数
    ($($arg:tt)*) => {
        // 使用青色输出提示信息
        println!("\n{}[提示]{} {}", CYAN, RESET, format!($($arg)*))
    }
}
macro_rules! log_success {
    ($($arg:tt)*) => {
        // 使用绿色输出成功信息
        println!("\n{}[成功]{} {}", GREEN, RESET, format!($($arg)*))
    }
}
macro_rules! log_warn {
    ($($arg:tt)*) => {
        // 使用黄色将警告信息输出到标准错误流 (stderr)
        eprintln!("\n{}[警告]{} {}", YELLOW, RESET, format!($($arg)*))
    }
}
macro_rules! log_error {
    ($($arg:tt)*) => {
        // 使用红色将错误信息输出到标准错误流 (stderr)
        eprintln!("\n{}[错误]{} {}", RED, RESET, format!($($arg)*))
    }
}

// --- 自定义错误类型 ---

/// 定义程序中可能发生的各种转换错误。
#[derive(Debug)] // 允许 Debug 打印
enum ConversionError {
    Io(io::Error),           // 包装标准库的 IO 错误
    Regex(regex::Error),     // 包装 Regex 库的错误
    ParseInt(ParseIntError), // 包装整数解析错误
    InvalidFormat(String),   // 自定义错误，表示文件格式或内容不符合预期
}

// 实现 Display Trait，用于向用户显示错误信息。
impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::Io(e) => write!(f, "文件读写错误: {}", e),
            ConversionError::Regex(e) => write!(f, "正则表达式处理错误: {}", e),
            ConversionError::ParseInt(e) => write!(f, "数字解析错误: {}", e),
            ConversionError::InvalidFormat(msg) => write!(f, "格式无效或内容错误: {}", msg),
        }
    }
}

// 实现 Error Trait，用于错误链和获取底层错误源。
impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConversionError::Io(e) => Some(e),
            ConversionError::Regex(e) => Some(e),
            ConversionError::ParseInt(e) => Some(e),
            _ => None, // InvalidFormat 没有底层错误源
        }
    }
}

// 实现 From Trait，使得可以使用 `?` 操作符方便地将其他错误类型转换为 ConversionError。
impl From<io::Error> for ConversionError {
    fn from(err: io::Error) -> Self {
        ConversionError::Io(err)
    }
}
impl From<regex::Error> for ConversionError {
    fn from(err: regex::Error) -> Self {
        ConversionError::Regex(err)
    }
}
impl From<ParseIntError> for ConversionError {
    fn from(err: ParseIntError) -> Self {
        ConversionError::ParseInt(err)
    }
}

// --- 数据结构定义 ---

/// 存储从 ASS Dialogue 行解析出的关键信息。
/// 主要用于 ASS -> LYS 的两遍扫描，避免重复解析。
#[derive(Clone)] // 需要 Clone trait 因为 LYS 转换逻辑需要访问前一个元素
struct ParsedDialogue {
    line_number: usize,             // 该 Dialogue 在原始 ASS 文件中的行号 (用于报错)
    start_ms: usize,                // Dialogue 开始时间 (毫秒)
    name: Option<String>,           // Dialogue 的 Name 字段内容 (例如 "左", "右", "背")
    segments: Vec<(String, usize)>, // 从 {\k} 标签解析出的文本段及其持续时间 (毫秒) 列表
    duration_ms: usize,             // Dialogue 行的总持续时间 (End - Start) (毫秒)
    sum_k_ms: usize,                // 行内所有 {\k} 标签解析出的时长总和 (毫秒)
    style: String,
}

/// 定义 ASS Name 字段的逻辑分类，用于简化 LYS 属性计算。
#[derive(PartialEq, Eq, Debug, Clone, Copy)] // 派生必要的 Trait
enum AssNameCategory {
    LeftV1,     // 代表 "" (空), "v1", "左", 以及 None (无 Name 字段)
    RightV2,    // 代表 "右", "v2", "x-duet", "x-anti"
    Background, // 代表 "背", "x-bg"
    Other,      // 代表任何其他非空的 Name 字段
}

/// 定义转换函数的类型别名，提高可读性
type ConversionFnSig = fn(&Path, &Path) -> Result<bool, ConversionError>;

// --- 静态正则表达式定义 ---
// 使用 once_cell::sync::Lazy 确保正则表达式只在首次使用时编译一次，提高性能。

/// 匹配 ASS 卡拉OK (Karaoke) 时间标签 {\kX} 或 {\kfX}，捕获时长 X (厘秒) 和紧随其后的文本。
static K_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Captures: (Group 1: Duration in cs) (Group 2: Text segment)
    Regex::new(r"\{\\k[f]?(\d+)\}([^\\{]*)").expect("未能编译 K_TAG_REGEX") // [f]? 匹配可选的 'f'
});
/// 匹配 QRC 行时间戳 `[start_ms,duration_ms]`。
static QRC_TIMESTAMP_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Captures: (Group 1: Start ms) (Group 2: Duration ms)
    Regex::new(r"\[(\d+),(\d+)\]").expect("未能编译 QRC_TIMESTAMP_REGEX")
});
/// 匹配 QRC 或 LYS 中的逐字/逐段时间戳 `(start_ms,duration_ms)`。
static WORD_TIME_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Captures: (Group 'start': Start ms) (Group 'duration': Duration ms) - 使用命名捕获组
    Regex::new(r"\((?P<start>\d+),(?P<duration>\d+)\)").expect("未能编译 WORD_TIME_TAG_REGEX")
});
/// 匹配 LYS 行的属性标签 `[property_value]` 并捕获属性值和后面的内容。
static LYS_PROPERTY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Captures: (Group 1: Property value) (Group 2: Remaining content)
    Regex::new(r"\[(\d+)\](.*)").expect("未能编译 LYS_PROPERTY_REGEX")
});
/// 匹配特定格式的 ASS Comment 行，用于提取元数据。
static META_COMMENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    // 要求: Comment 行, 起始结束时间为0, Style 为 "meta", 最后捕获元数据键值对文本
    // Captures: (Group 1: Metadata Key:Value text)
    Regex::new(r"^Comment:\s*\d+,0:00:00\.00,0:00:00\.00,meta,,0,0,0,,(.*)")
        .expect("未能编译 META_COMMENT_REGEX")
});
static ASS_DIALOGUE_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Format: Dialogue: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text
    Regex::new(
        //  ^Dialogue: Layer,   Start Time        ,    End Time         , Style    , Name     , ML , MR , MV , Effect, Text
        r"^Dialogue:\s*[^,]+,(?P<start_time>\d+:\d+:\d+\.\d+),(?P<end_time>\d+:\d+:\d+\.\d+),(?P<style>[^,]*),(?P<name>[^,]*),[^,]*,[^,]*,[^,]*,[^,]*,(?P<text>.*)"
    ).expect("未能编译 ASS_DIALOGUE_REGEX")
});
/// 匹配 ASS Name 字段中的语言标签 "x-lang:<code>" 并捕获语言代码。
static LANG_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Captures: (Group 'lang_code': the language code part)
    Regex::new(r"^x-lang:(?P<lang_code>.+)$").expect("未能编译 LANG_TAG_REGEX")
});

/// 匹配常见的 ASS 标签（如 {\...}）以方便移除。
static ASS_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    // 匹配花括号及其内部所有非花括号的内容
    Regex::new(r"\{[^}]*\}").expect("未能编译 ASS_TAG_REGEX")
});


// --- Clap 命令行接口定义 ---

/// 定义程序的命令行参数和选项。
/// 使用 `clap` 的 `derive` 宏自动生成解析器和帮助信息。
#[derive(Parser, Debug)]
#[command(
    author = "apoint123",
    version = "1.0.0",
    about = "ASS/QRC/LYS 字幕/歌词格式转换工具",
    long_about = "一个简单的工具，用于在 ASS, QRC 和 Lyricify Syllable (.lys) 格式之间转换文件。"
)]
struct CliArgs {
    /// 运行交互模式，此选项会忽略其他所有位置参数。
    #[arg(short, long)] // -i, --interactive 标志
    interactive: bool,

    /// 【必需】输入文件路径。
    /// 如果只提供此参数（没有 direction 和 output_file），则进入自动模式。
    #[arg(index = 1)] // 第一个位置参数
    input_file: Option<PathBuf>, // 定义为 Option 以便在仅使用 --interactive 时不报错

    /// 【手动模式可选】转换方向 (例如: ass2qrc, qrc2ass, 2q, 2a, 2l, l2a)。
    /// 如果提供此参数，则必须同时提供 OUTPUT_FILE。
    #[arg(index = 2)] // 第二个可选位置参数
    direction: Option<String>,

    /// 【手动模式可选】输出文件路径。
    /// 仅在提供了 DIRECTION 时使用。
    #[arg(index = 3)] // 第三个可选位置参数
    output_file: Option<PathBuf>,

    /// 如果输入是 ASS 文件，则额外提取翻译行到 LRC 文件。
    #[arg(long)] // 定义 --extract-lrc 标志
    extract_lrc: bool,
}

// --- 程序主入口 ---

/// 程序的主函数。
fn main() {
    // 使用 clap 解析命令行参数。
    let args = CliArgs::parse();

    // 检查原始命令行参数的数量
    // std::env::args() 返回一个迭代器，第一个元素通常是程序自身的路径
    // 如果参数数量小于等于 1，说明用户没有提供任何额外的参数（例如双击运行）
    if std::env::args().len() <= 1 {
        interactive_mode(); // 直接进入交互模式
        return; // 退出程序
    }

    // 提取 extract_lrc 标志的值，以便传递给后续函数
    let should_extract_lrc = args.extract_lrc;

    // 检查是否提供了必要的 input_file (在非交互模式下)
    let input_path = match args.input_file {
        Some(path) => path,
        None => {
            // 如果 input_file 为 None 且不是交互模式，说明用户未提供输入文件
            log_error!("错误：需要指定输入文件或使用 --interactive 选项。");
            CliArgs::command().print_help().unwrap_or_else(|e| log_error!("无法打印帮助信息: {}", e));
            // wait_for_exit(); // 可以选择在这里等待或直接退出
            return;
        }
    };

    // 优先级 2: 根据 'direction' 和 'output_file' 是否存在来判断模式
    match (args.direction, args.output_file) {
        // 组合 1: 自动模式 (direction 和 output_file 都没有提供)
        (None, None) => {
            // 调用自动模式处理函数，并传入 extract_lrc 标志的值
            run_automatic_mode_clap(&input_path);
        }

        // 组合 2: 手动模式 (direction 和 output_file 都提供了)
        (Some(dir), Some(output)) => {
             // 调用手动模式处理函数，并传入 extract_lrc 标志的值
             run_manual_mode_clap(&dir, &input_path, &output, should_extract_lrc);
        }

        // 组合 3: 无效或不完整的参数组合 (手动模式参数不匹配)

        // 提供了 direction 但缺少 output_file
        (Some(_), None) => {
            log_error!("错误：手动模式需要同时提供转换方向和输出文件。");
            CliArgs::command().print_help().unwrap_or_else(|e| log_error!("无法打印帮助信息: {}", e));
            wait_for_exit();
        }
        // 提供了 output_file 但缺少 direction (这通常暗示用户想用自动模式但误提供了输出)
        (None, Some(_)) => {
            log_error!("错误：提供了输出文件但未指定转换方向（自动模式请勿指定输出文件）。");
            CliArgs::command().print_help().unwrap_or_else(|e| log_error!("无法打印帮助信息: {}", e));
            wait_for_exit();
        }
    }
}


// --- 模式处理函数 (由 main 调用) ---

/// 执行手动转换模式。
///
/// # Arguments
/// * `direction` - 用户指定的转换方向字符串。
/// * `input_path` - 输入文件的路径。
/// * `output_path` - 输出文件的路径。
fn run_manual_mode_clap(direction: &str, input_path: &Path, output_path: &Path, extract_lrc: bool) {
    if !input_path.exists() {
        log_error!("{}", FILE_NOT_FOUND_ERROR);
        wait_for_exit();
        return;
    }

    // 定义转换函数的类型签名别名，方便使用。
    let lower_dir = direction.to_lowercase(); // 将方向字符串转为小写，进行不区分大小写的匹配。
    // 定义转换函数的包装器，以统一返回类型 (Result<bool, ConversionError>)
    // 这里的 bool 代表 warning_occurred
    let qrc2ass_wrapper = |i: &Path, o: &Path| convert_qrc_to_ass(i, o); // convert_qrc_to_ass 已经是 Result<bool, ...>
    let lys2ass_wrapper = |i: &Path, o: &Path| convert_lys_to_ass(i, o); // convert_lys_to_ass 已经是 Result<bool, ...>
    // convert_ass_to_qrc 和 convert_ass_to_lys 已经是 Result<bool, ...>

    // 根据方向字符串选择对应的转换函数。
    let conversion_function_to_execute: Option<ConversionFnSig> = match lower_dir.as_str() {
        "ass2qrc" | "2q" => Some(convert_ass_to_qrc),
        "qrc2ass" | "2a" => Some(qrc2ass_wrapper),
        "ass2lys" | "2l" => Some(convert_ass_to_lys),
        "lys2ass" | "l2a" => Some(lys2ass_wrapper),
        _ => None,
    };

    // 初始化一个变量来跟踪操作是否需要暂停
    let mut operation_requires_pause = false;

    match conversion_function_to_execute {
        Some(selected_action) => {
            // 首先执行主转换
            // execute_conversion 返回 true 如果主转换出错或有需要暂停的警告
            operation_requires_pause = execute_conversion(selected_action, input_path, output_path);

            // 检查是否需要提取翻译 (仅当输入是 ASS 时)
            let input_is_ass = input_path.extension()
                .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("ass"));

            if input_is_ass && extract_lrc { // extract_lrc 是命令行参数
                match extract_translations_to_lrc(input_path) {
                    Ok(warned) => {
                        if warned { operation_requires_pause = true; } // 如果提取操作本身有警告，也需要暂停
                    }
                    Err(e) => {
                        log_error!("提取 LRC 翻译时出错: {}", e);
                        operation_requires_pause = true; // 提取出错需要暂停
                    }
                }
                match extract_roma_to_lrc(input_path) {
                    Ok(warned) => {
                        if warned { operation_requires_pause = true; } // 如果提取操作本身有警告，也需要暂停
                    }
                    Err(e) => {
                        log_error!("提取 LRC 罗马音时出错: {}", e);
                        operation_requires_pause = true; // 提取出错需要等待
                    }
                }
            }
        }
        None => {
            log_error!("无效的转换方向: {}", direction);
            CliArgs::command().print_help().unwrap_or_else(|e| log_error!("无法打印帮助信息: {}",e));
            operation_requires_pause = true; // 无效方向，也需要暂停以显示帮助
        }
    }

    // 如果主转换或任何提取步骤出错/有警告，则等待用户确认
    if operation_requires_pause {
        wait_for_exit();
    }
}

/// 执行自动转换模式。
///
/// # Arguments
/// * `input_path` - 输入文件的路径。
fn run_automatic_mode_clap(input_path: &Path) {
    // 检查输入文件是否存在。
    if !input_path.exists() {
        log_error!("{}", FILE_NOT_FOUND_ERROR);
        wait_for_exit();
        return;
    }

    // 获取输入文件的扩展名（小写）。
    let extension = input_path
        .extension()
        .and_then(|s| s.to_str()) // Option<OsStr> -> Option<&str>
        .unwrap_or("")           // 如果没有扩展名，则为空字符串
        .to_lowercase();         // 转为小写

    let mut needs_wait = false; // 标记本次操作后是否需要等待退出

    let qrc2ass_wrapper = |i: &Path, o: &Path| -> Result<bool, ConversionError> {
    convert_qrc_to_ass(i, o).map(|_| false)};

    let lys2ass_wrapper = |i: &Path, o: &Path| -> Result<bool, ConversionError> {
    convert_lys_to_ass(i, o).map(|_| false)};



    // 根据文件扩展名决定执行哪个转换。
    match extension.as_str() {
        "lys" => {
            let output_path = auto_output_path(input_path, ASS_EXTENSION);
            if execute_conversion(lys2ass_wrapper, input_path, &output_path) {
                 needs_wait = true;
            }

        }
        "ass" => {
            // --- 主转换逻辑 ---
            let main_conversion_result = match check_ass_has_special_names(input_path) {
                Ok(true) => {
                    let output_path = auto_output_path(input_path, LYRICIFY_EXTENSION);
                    execute_conversion(convert_ass_to_lys, input_path, &output_path)
                }
                Ok(false) => {
                    let output_path = auto_output_path(input_path, QRC_EXTENSION);
                    execute_conversion(convert_ass_to_qrc, input_path, &output_path)
                }
                Err(e) => {
                    log_error!("检查 ASS Name 字段时出错: {}", e);
                    true // 出错，标记需要等待
                }
            };
            if main_conversion_result { needs_wait = true; } // 如果主转换出错/警告，标记等待

            // --- 自动模式下，无条件尝试提取翻译 ---
            match extract_translations_to_lrc(input_path) {
                Ok(warned) => {
                    if warned { needs_wait = true; }
                }
                Err(e) => {
                    log_error!("提取 LRC 翻译时出错: {}", e);
                    needs_wait = true;
                }
            }

            // --- 自动模式下，无条件尝试提取罗马音 ---
            match extract_roma_to_lrc(input_path) {
                Ok(warned) => {
                    if warned { needs_wait = true; }
                }
                Err(e) => {
                    log_error!("提取 LRC 罗马音时出错: {}", e);
                    needs_wait = true;
                }
            }

        }
        "qrc" => {
            let output_path = auto_output_path(input_path, ASS_EXTENSION);
            if execute_conversion(qrc2ass_wrapper, input_path, &output_path) {
                 needs_wait = true;
            }

        }
        _ => {
            log_error!("无法根据文件后缀 .{} 判断转换方向", extension);
            needs_wait = true;
        }
    }

    // 如果任何步骤出错/警告，则等待
    if needs_wait {
        wait_for_exit();
    }
}

/// 运行交互式命令行界面，引导用户进行转换。
fn interactive_mode() {
    log_info!("直接将文件拖到程序图标上可自动转换");
    // 无限循环，提供持续的转换服务，直到用户手动关闭窗口。
    loop {
        println!("请选择源文件格式：");
        println!("{}. ASS 文件 (.ass)", ASS_FORMAT_CHOICE);
        println!("{}. QRC 文件 (.qrc)", QRC_FORMAT_CHOICE);
        println!("{}. Lyricify Syllable 文件 (.lys)", LYS_FORMAT_CHOICE);

        // 1. 读取用户输入的源文件格式选择
        let source_choice = match read_user_input("你的选择: ") {
            Ok(choice) if !choice.is_empty() => choice, // 获取非空输入
            Ok(_) => {                                  // 输入为空
                log_error!("{}", INVALID_CHOICE_MESSAGE); // 打印无效选择错误
                continue;                               // 跳过本次循环，重新开始
            }
            Err(e) => {                                 // 读取输入时发生 IO 错误
                log_error!("读取输入时出错: {}", e);   // 打印具体错误
                continue;                               // 重新开始循环
            }
        };

        // 2. 根据用户选择的源格式，确定允许的目标格式列表和相应的提示信息
        // 定义目标选项元组类型，方便存储: (用户输入选项字符串, 文件扩展名字符串)
        type TargetOption = (&'static str, &'static str);
        // 使用 match 语句根据 source_choice 分配变量
        let (source_extension, target_options, target_prompt): (
            &str,              // 源文件扩展名 (例如 ".ass")
            Vec<TargetOption>, // 允许的目标选项列表 (例如 vec![("2", ".qrc"), ("3", ".lys")])
            String,            // 提示用户选择目标格式的文本
        ) = match source_choice.as_str() {
            // 如果源是 ASS
            ASS_FORMAT_CHOICE => (
                ASS_EXTENSION,
                vec![(QRC_FORMAT_CHOICE, QRC_EXTENSION), (LYS_FORMAT_CHOICE, LYRICIFY_EXTENSION)],
                format!("请选择目标文件格式:\n{}. QRC 文件 (.qrc)\n{}. Lyricify Syllable 文件 (.lys)", QRC_FORMAT_CHOICE, LYS_FORMAT_CHOICE),
            ),
            // 如果源是 QRC
            QRC_FORMAT_CHOICE => (
                QRC_EXTENSION,
                vec![(ASS_FORMAT_CHOICE, ASS_EXTENSION)],
                format!("请选择目标文件格式:\n{}. ASS 文件 (.ass)", ASS_FORMAT_CHOICE),
            ),
            // 如果源是 LYS
            LYS_FORMAT_CHOICE => (
                LYRICIFY_EXTENSION,
                vec![(ASS_FORMAT_CHOICE, ASS_EXTENSION)],
                format!("请选择目标文件格式:\n{}. ASS 文件 (.ass)", ASS_FORMAT_CHOICE),
            ),
            // 如果 source_choice 不是 "1", "2", 或 "3"
            _ => {
                log_error!("{}", INVALID_CHOICE_MESSAGE); // 打印无效选择
                continue;                               // 重新开始循环
            }
        };

        println!("{}", target_prompt); // 向用户显示可选的目标格式

        // 3. 读取用户输入的目标文件格式选择
        let target_choice = match read_user_input("你的选择: ") {
            Ok(choice) if !choice.is_empty() => choice,
            Ok(_) => {
                log_error!("{}", INVALID_CHOICE_MESSAGE);
                continue;
            }
            Err(e) => {
                log_error!("读取输入时出错: {}", e);
                continue;
            }
        };

        // 4. 校验用户选择的目标格式是否在允许的选项中
        let target_info = target_options.iter().find(|(choice, _)| *choice == target_choice);
        if target_info.is_none() { // 如果没找到匹配的选项
            log_error!("{}", INVALID_CHOICE_MESSAGE);
            continue; // 重新开始循环
        }
        // 获取目标格式对应的文件扩展名，用于后续文件路径提示
        let target_extension_for_prompt = target_info.unwrap().1;

        // 5. 读取输入文件的路径
        let input_path = match read_file_path(INPUT_FILE_PATH_PROMPT, source_extension) {
            Ok(path) => path,
            Err(e) => {
                log_error!("读取输入路径时出错: {}", e);
                continue; // 重新开始循环
            }
        };
        // 检查输入文件是否存在
        if !input_path.exists() {
            log_error!("{}", FILE_NOT_FOUND_ERROR); // 打印文件不存在错误
            continue; // 重新开始循环
        }

        // 6. 读取输出文件的路径
        let output_path = match read_file_path(OUTPUT_FILE_PATH_PROMPT, target_extension_for_prompt) {
            Ok(path) => path,
            Err(e) => {
                log_error!("读取输出路径时出错: {}", e);
                continue; // 重新开始循环
            }
        };

        // 7. 根据用户的源格式和目标格式选择，调用相应的转换函数
        //    所有转换函数现在都返回 Result<bool, ConversionError>
        //    我们调用 execute_conversion 来封装实际的函数调用和日志打印
        let _needs_wait = match (source_choice.as_str(), target_choice.as_str()) {
            (ASS_FORMAT_CHOICE, QRC_FORMAT_CHOICE) =>
                // 调用 execute_conversion，传入转换函数 `convert_ass_to_qrc`
                execute_conversion(convert_ass_to_qrc, &input_path, &output_path),
            (ASS_FORMAT_CHOICE, LYS_FORMAT_CHOICE) =>
                // 调用 execute_conversion，传入转换函数 `convert_ass_to_lys`
                execute_conversion(convert_ass_to_lys, &input_path, &output_path),
            (QRC_FORMAT_CHOICE, ASS_FORMAT_CHOICE) =>
                // 调用 execute_conversion，传入转换函数 `convert_qrc_to_ass`
                execute_conversion(convert_qrc_to_ass, &input_path, &output_path),
            (LYS_FORMAT_CHOICE, ASS_FORMAT_CHOICE) =>
                // 调用 execute_conversion，传入转换函数 `convert_lys_to_ass`
                execute_conversion(convert_lys_to_ass, &input_path, &output_path),
            _ => { // 理论上不会执行到这里，因为前面已经校验过选项组合
                log_error!("内部错误：无效的转换组合");
                false // 标记不需要等待
            }
        };

        // 8. 交互模式下，一次转换结束后提示用户可以继续操作
        //    不需要根据 _needs_wait 来调用 wait_for_exit()，因为如果转换内部
        //    有警告并调用了 wait_for_exit()，用户已经看到了等待提示。
        log_info!("本次转换操作完成。您可以继续进行下一次转换，或关闭此窗口。");

    } // 交互模式的无限循环结束 (实际上只有用户关闭窗口才会结束)
}

// --- 核心转换函数 ---

/// 将 ASS 文件转换为 QRC 文件 (单遍扫描)。
fn convert_ass_to_qrc(ass_path: &Path, qrc_path: &Path) -> Result<bool, ConversionError> {
    // 打开输入文件并获取元数据 (用于进度条总大小)
    let file = File::open(ass_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes: usize = 0; // 跟踪已处理字节数
    let mut warning_occurred = false; // 标记是否有时间不一致警告

    // 创建带缓冲的读取器和写入器以提高效率
    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(qrc_path)?);

    let mut metadata_lines: Vec<String> = Vec::new(); // 存储解析出的元数据行 [ti:...]
    let mut output_qrc_lines: Vec<String> = Vec::new(); // 存储转换后的 QRC 内容行

    let mut after_format = false; // 标记是否已找到 Events 段的 Format 行
    let mut line_number = 0; // 文件行号计数器

    // 逐行读取输入文件
    for line_result in reader.lines() {
        line_number += 1;
        let line = line_result?; // 处理 IO 错误
        // 估算已处理字节数 (用于进度条)
        let line_bytes = line.len() + if cfg!(windows) { 2 } else { 1 }; // 加上换行符字节
        processed_bytes += line_bytes;

        // --- 核心处理逻辑 ---

        // 必须先找到 Format 行才能开始处理 Dialogue 和 Comment
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name,") {
                after_format = true; // 找到 Format 行
            }
            display_progress_bar(processed_bytes.min(total_bytes), total_bytes); // 更新进度条
            continue; // 跳过 Format 行之前的所有行 (包括 Format 行本身)
        }

        // --- 在 Format 行之后 ---

        // 检查是否是元数据 Comment 行
        if let Some(caps) = META_COMMENT_REGEX.captures(&line) {
            if let Some(text) = caps.get(1) { // 获取捕获的元数据文本
                if let Some(formatted_meta) = parse_ass_metadata_text(text.as_str()) {
                    metadata_lines.push(formatted_meta); // 收集元数据
                }
            }
            display_progress_bar(processed_bytes.min(total_bytes), total_bytes); // 更新进度条
            continue; // 跳到下一行
        }

        // 检查是否是 Dialogue 行
        if line.starts_with("Dialogue:") {
            // 调用辅助函数处理 Dialogue -> QRC 转换
             match process_dialogue_for_qrc(&line, line_number, &mut warning_occurred) {
                 Ok(Some(qrc_line)) => output_qrc_lines.push(qrc_line), // 收集 QRC 行
                 Ok(None) => {}, // 解析器认为无效或跳过，不处理
                 Err(e) => {
                     // 记录警告并继续处理下一行
                     log_warn!("处理第 {} 行 Dialogue 时出错: {}", line_number, e);
                     // 注意：这里不返回 Err，允许程序继续处理文件的其余部分
                 }
             }
        }
        // 忽略 Format 行之后的其他非 Dialogue、非元数据 Comment 行

        display_progress_bar(processed_bytes.min(total_bytes), total_bytes); // 更新进度条
    }

    // --- 读取完成，开始写入输出文件 ---

    // 1. 写入元数据 (如果存在)
    for meta_line in &metadata_lines {
        writeln!(writer, "{}", meta_line)?;
    }

    // 2. 写入 QRC 内容
    for qrc_line in &output_qrc_lines {
        writeln!(writer, "{}", qrc_line)?;
    }

    // --- 写入完成，收尾 ---
    display_progress_bar(total_bytes, total_bytes); // 确保进度条显示 100%
    println!(); // 进度条后换行

    writer.flush()?; // 确保所有缓冲内容写入文件
    log_success!("{}", ASS_TO_QRC_COMPLETE); // 打印成功信息

    Ok(warning_occurred) // 返回包含警告状态的 Ok
}


/// 将 QRC 文件转换为 ASS 文件 (单遍扫描)。
fn convert_qrc_to_ass(qrc_path: &Path, ass_path: &Path) -> Result<bool, ConversionError> {
    let file = File::open(qrc_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes: usize = 0; // 跟踪已处理字节

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(ass_path)?);

    // 写入 ASS 文件头和样式 (使用辅助函数)
    write_ass_header(&mut writer)?;

    // 逐行读取 QRC 文件
    for line_result in reader.lines() {
        let line = line_result?;
        let line_bytes = line.len() + if cfg!(windows) { 2 } else { 1 };
        processed_bytes += line_bytes;

        // 尝试匹配行时间戳 [开始ms,持续ms]
        // 跳过元数据行或非 QRC 时间戳开头的行
        if let Some(ts_caps) = QRC_TIMESTAMP_REGEX.captures(&line) {
             // 解析行开始和持续时间
            let header_start_ms: usize = ts_caps[1].parse()?;
            let header_duration_ms: usize = ts_caps[2].parse()?;
            let header_end_ms = header_start_ms.saturating_add(header_duration_ms); // 计算行结束时间

            // 转换为 ASS 时间格式
            let start_time_ass = milliseconds_to_time(header_start_ms);
            let end_time_ass = milliseconds_to_time(header_end_ms);

            // --- 重建 ASS 文本和 K 标签 ---
            let mut ass_text_builder = String::new(); // 用于构建带 K 标签的文本
            let mut last_word_end_ms = header_start_ms; // 跟踪上一个单词的结束时间, 初始化为行开始时间

            // 获取时间戳之后的内容部分
            let content_part = &line[ts_caps.get(0).unwrap().end()..];

            // 提取所有单词时间戳及其在字符串中的位置
            let mut time_tags: Vec<(usize, usize, usize, usize)> = Vec::new(); // (start_pos, end_pos, start_ms, duration_ms)
            for cap in WORD_TIME_TAG_REGEX.captures_iter(content_part) {
                let start_pos = cap.get(0).unwrap().start(); // 时间戳在字符串中的开始位置
                let end_pos = cap.get(0).unwrap().end();     // 时间戳在字符串中的结束位置
                // 使用命名捕获组解析单词的开始和持续时间
                let word_start_ms: usize = cap["start"].parse()?;
                let word_duration_ms: usize = cap["duration"].parse()?;

                // 过滤掉无效时间戳 (例如时长为0)
                 if word_duration_ms == 0 { continue; }

                time_tags.push((start_pos, end_pos, word_start_ms, word_duration_ms));
            }

            // 按时间戳在字符串中的位置排序，以正确提取文本片段
             time_tags.sort_by_key(|k| k.0);

            // 遍历时间戳，重建文本和 K 标签
            let mut current_char_index = 0; // 跟踪 content_part 的处理位置
            for (tag_start_pos, tag_end_pos, word_start_ms, word_duration_ms) in time_tags {
                 // 提取当前时间戳之前的文本片段
                 let text_segment = &content_part[current_char_index..tag_start_pos];

                 // 计算与上个词尾的时间差（用于插入停顿的 K 标签）
                 if word_start_ms > last_word_end_ms {
                     let gap_ms = word_start_ms - last_word_end_ms;
                     // 向上取整计算 K 值 (厘秒)
                     let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                     if gap_k_value > 0 {
                         ass_text_builder.push_str(&format!("{{\\k{}}}", gap_k_value)); // 插入停顿 K 标签
                     }
                 }

                 // 计算当前单词的 K 值 (向上取整)
                 let word_k_value = (word_duration_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;

                 // 添加 K 标签和文本段 (只有 K > 0 时才添加标签)
                 if word_k_value > 0 {
                     // 如果文本段为空，只添加 K 标签（例如 {\k50}）
                     // 如果文本段不为空，添加 K 标签和文本（例如 {\k50}歌词）
                     ass_text_builder.push_str(&format!("{{\\k{}}}{}", word_k_value, text_segment));
                 } else if !text_segment.is_empty() {
                     // 如果 K=0 但有文本，只添加文本 (避免 {\k0} )
                     ass_text_builder.push_str(text_segment);
                 }

                 // 更新下一个文本段的起始索引和上个词的结束时间
                 current_char_index = tag_end_pos;
                 last_word_end_ms = word_start_ms.saturating_add(word_duration_ms);
            }

             // 处理最后一个时间戳到行尾的内容
             let remaining_text = &content_part[current_char_index..];
             // 检查是否有剩余文本，或者是否有时间间隙
             if !remaining_text.is_empty() { // 有剩余文本
                 if header_end_ms > last_word_end_ms { // 且行未结束
                    let gap_ms = header_end_ms - last_word_end_ms;
                    let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                    if gap_k_value > 0 {
                        // 给剩余文本加上 K 标签
                        ass_text_builder.push_str(&format!("{{\\k{}}}{}", gap_k_value, remaining_text));
                    } else {
                        // K=0, 直接加文本
                        ass_text_builder.push_str(remaining_text);
                    }
                 } else {
                     // 行已结束，直接追加剩余文本
                     ass_text_builder.push_str(remaining_text);
                 }
             } else if header_end_ms > last_word_end_ms { // 没有剩余文本，但行未结束 (纯粹的时间间隙)
                let gap_ms = header_end_ms - last_word_end_ms;
                let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                if gap_k_value > 0 {
                    // 只插入 K 标签表示停顿
                    ass_text_builder.push_str(&format!("{{\\k{}}}", gap_k_value));
                }
             }

            // 移除可能产生的 {\k0} 标签 (理论上应该不会产生，但作为保险)
            let final_ass_text = ass_text_builder.replace("{\\k0}", "");

            // 如果最终文本不为空，则写入 ASS Dialogue 行
            if !final_ass_text.is_empty() {
                writeln!(
                    writer,
                    // 使用默认样式 "Default", Name 字段留空
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}",
                    start_time_ass,
                    end_time_ass,
                    final_ass_text
                )?;
            }
        } // end if let Some(ts_caps)

        // 更新进度条
        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    } // end for line_result

    // --- 收尾 ---
    display_progress_bar(total_bytes, total_bytes); // 确保进度条 100%
    println!(); // 换行

    writer.flush()?;
    log_success!("{}", QRC_TO_ASS_COMPLETE); // 打印成功信息

    Ok(false)
}


/// 将 ASS 文件转换为 Lyricify Syllable (.lys) 文件 (两遍扫描)。
/// 使用两遍扫描是为了正确处理 LYS 的 '背' (背景) 属性，该属性依赖于前一行的 Name 字段。
fn convert_ass_to_lys(ass_path: &Path, lys_path: &Path) -> Result<bool, ConversionError> {
    let mut metadata_lines: Vec<String> = Vec::new(); // 存储元数据行
    let mut parsed_dialogues: Vec<ParsedDialogue> = Vec::new(); // 存储第一遍解析的 Dialogue 数据
    let mut warning_occurred = false; // 时间不一致警告标志
    let mut line_counter: usize = 0; // 文件行号计数器
    let mut warning_occurred_overall = false; // 用于累积所有警告

    // --- 第一遍扫描: 读取文件, 收集元数据, 解析 Dialogue 行并检查时间一致性 ---
    { // 使用块作用域限制 reader_pass1 的生命周期
        let file_pass1 = File::open(ass_path)?;
        let reader_pass1 = BufReader::new(file_pass1);
        let mut after_format = false; // 标记是否已找到 Format 行

        for line_result in reader_pass1.lines() {
            line_counter += 1;
            let line = line_result?; // 处理 IO 错误

            // 跳过直到找到 Format 行
            if !after_format {
                if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name,") {
                    after_format = true;
                }
                continue;
            }

            // --- 在 Format 行之后处理 ---

            // 检查元数据 Comment 行
            if let Some(caps) = META_COMMENT_REGEX.captures(&line) {
                 if let Some(text) = caps.get(1) {
                     if let Some(formatted_meta) = parse_ass_metadata_text(text.as_str()) {
                         metadata_lines.push(formatted_meta); // 收集元数据
                     }
                 }
                 continue; // 处理完元数据，跳到下一行
            }

            // 尝试解析 Dialogue 行
            if line.starts_with("Dialogue:") {
                // 调用辅助函数解析
                match parse_ass_dialogue_line(&line, line_counter)? {
                    Some(dialogue_data) => {
                        // --- 如果不是 roma/trans/ts 才进行时间一致性检查 ---
                        if !(dialogue_data.style.eq_ignore_ascii_case("roma")
                             || dialogue_data.style.eq_ignore_ascii_case("trans")
                             || dialogue_data.style.eq_ignore_ascii_case("ts"))
                        {
                            // 只有在样式不是 roma/trans/ts 时才调用检查函数
                            if !check_time_consistency(dialogue_data.duration_ms, dialogue_data.sum_k_ms, dialogue_data.line_number) {
                                warning_occurred_overall = true; // 设置警告标志
                            }
                        }
                        parsed_dialogues.push(dialogue_data); // 存储解析结果
                    }
                    None => {
                        // 虽然以 "Dialogue:" 开头，但正则不匹配，可能格式错误
                        log_warn!("第 {} 行看起来像 Dialogue 但无法完整解析其结构。", line_counter);
                        warning_occurred_overall = true;
                    }
                }
            }
            // 忽略其他非元数据、非 Dialogue 的行
        }
    } // reader_pass1 和 file_pass1 在此释放

    // --- 第二遍扫描: 写入元数据, 计算 LYS 属性并写入 LYS 文件 ---
    let mut writer = BufWriter::new(File::create(lys_path)?);
    let total_dialogues = parsed_dialogues.len();
    // LYS '背' 属性计算需要跟踪上一次的计算结果 (因为 '背' 后面跟 '背' 需要继承)
    let mut last_calculated_property = LYS_PROPERTY_UNSET;

    // 1. 写入元数据 (如果存在)
    for meta_line in &metadata_lines {
        writeln!(writer, "{}", meta_line)?;
    }

    // 2. 遍历第一遍解析好的 Dialogue 数据
    for (i, current_dialogue) in parsed_dialogues.iter().enumerate() {

        // 如果当前行的 Style 是 "ts" 或 "trans"，则跳过，不生成 LYS 输出
        if current_dialogue.style == "ts" || current_dialogue.style == "trans" {
            // 更新进度条时考虑跳过的行，可以使用总行数 i+1 作为当前进度
            display_progress_bar(i + 1, total_dialogues);
            continue; // 进行下一次循环
        }
        // 如果不是翻译行，则继续处理

        // 获取上一个解析后的 Dialogue 数据（如果当前不是第一行）
        let previous_dialogue = if i > 0 { parsed_dialogues.get(i - 1) } else { None };

        // 调用辅助函数计算当前行的 LYS 属性
        let (property, calc_warned) = calculate_lys_property(
        current_dialogue,
        previous_dialogue,
        last_calculated_property // 传入上一次的计算结果用于继承
        );
        if calc_warned { // 如果 calculate_lys_property (主要来自 map_ass_name_to_category) 报告了警告
            warning_occurred_overall = true;
        }
        last_calculated_property = property;

        // 构建 LYS 输出行: [属性]文本1(开始ms,持续ms)文本2(开始ms,持续ms)...
        let mut lys_line_content = format!("[{}]", property); // 行首是属性标签
        let mut current_segment_start_ms = current_dialogue.start_ms; // LYS 时间戳是分段的绝对开始时间
        for (seg_text, seg_ms) in &current_dialogue.segments {
             let segment_duration_ms = *seg_ms; // 解引用得到时长值
             // 过滤掉无效分段 (无文本且无时长)
             if !seg_text.is_empty() || segment_duration_ms > 0 {
                 // 拼接文本和时间戳
                 lys_line_content.push_str(&format!("{}({},{})", seg_text, current_segment_start_ms, segment_duration_ms));
                 // 更新下一个分段的理论开始时间
                 current_segment_start_ms += segment_duration_ms;
             }
        }
        // 将构建好的完整 LYS 行写入文件
        writeln!(writer, "{}", lys_line_content)?;

        // 更新进度条 (基于已处理的 Dialogue 数量)
        display_progress_bar(i + 1, total_dialogues);
    }

    // --- 收尾工作 ---
    display_progress_bar(total_dialogues, total_dialogues); // 确保进度条达到 100%
    println!(); // 进度条后换行

    writer.flush()?; // 确保所有缓冲数据写入磁盘
    log_success!("{}", ASS_TO_LYS_COMPLETE); // 输出成功日志

    Ok(warning_occurred_overall) // 返回总的警告状态
}


/// 将 Lyricify Syllable (.lys) 文件转换为 ASS 文件 (单遍扫描)。
fn convert_lys_to_ass(lys_path: &Path, ass_path: &Path) -> Result<bool, ConversionError> {
    let file = File::open(lys_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes: usize = 0; // 跟踪已处理字节
    let mut line_number: usize = 0; // 文件行号
    let mut warning_occurred_overall = false; // 用于累积所有警告

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(ass_path)?);

    // 写入 ASS 文件头和样式 (使用辅助函数)
    write_ass_header(&mut writer)?;

    // 逐行读取 LYS 文件
    for line_result in reader.lines() {
        line_number += 1;
        let line = line_result?;
        let line_bytes = line.len() + if cfg!(windows) { 2 } else { 1 };
        processed_bytes += line_bytes;

        // 尝试匹配 LYS 行的属性标签 `[属性值]` 和内容部分
        if let Some(prop_caps) = LYS_PROPERTY_REGEX.captures(&line) {
            // 解析属性值 (如果解析失败则使用默认值 LYS_PROPERTY_UNSET)
            let property: usize = prop_caps[1].parse().unwrap_or(LYS_PROPERTY_UNSET);
            let content = &prop_caps[2]; // 获取属性标签之后的内容

            // --- 解析内容中的单词/分段时间戳 `(开始ms,持续ms)` ---
            let mut timestamps: Vec<(usize, usize, usize, usize)> = Vec::new(); // (start_pos, end_pos, start_ms, duration_ms)
            for ts_caps in WORD_TIME_TAG_REGEX.captures_iter(content) {
                let start_pos = ts_caps.get(0).unwrap().start(); // 时间戳在字符串中的开始位置
                let end_pos = ts_caps.get(0).unwrap().end();     // 时间戳在字符串中的结束位置
                // 使用命名捕获组解析时间戳数值
                let start_ms: usize = ts_caps["start"].parse()?;
                let duration_ms: usize = ts_caps["duration"].parse()?;
                timestamps.push((start_pos, end_pos, start_ms, duration_ms));
            }

            // 如果行内没有解析到有效的时间戳，则跳过此行
            if timestamps.is_empty() {
                display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
                continue;
            }

            // 按时间戳在字符串中的出现位置排序，以保证正确提取文本片段
            timestamps.sort_by_key(|k| k.0);

            // --- 重建 ASS 文本和对应的 {\k} 标签 ---
            let mut ass_text_parts: Vec<String> = Vec::new(); // 存储构建的 ASS 文本片段
            let mut last_char_pos = 0; // 跟踪已处理到的字符索引
            let mut min_start_ms = usize::MAX; // 记录该行所有时间戳中最早的开始时间
            let mut max_end_ms = 0; // 记录该行所有时间戳中最晚的结束时间

            // 第一次遍历时间戳：确定行的整体开始时间和结束时间
            for &(_, _, start_ms, duration_ms) in &timestamps {
                 if start_ms < min_start_ms {
                     min_start_ms = start_ms; // 更新最小开始时间
                 }
                 // 计算当前时间戳段的结束时间
                 let current_end_ms = start_ms.saturating_add(duration_ms);
                 if current_end_ms > max_end_ms {
                     max_end_ms = current_end_ms; // 更新最大结束时间
                 }
            }

            // 如果未能找到有效的最小开始时间 (例如所有时间戳都是 (0,0) 且被忽略)，跳过此行
            if min_start_ms == usize::MAX {
                 log_warn!("第 {} 行 LYS 数据缺少有效时间戳，已跳过。", line_number);
                 warning_occurred_overall = true; // 累积警告
                 display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
                 continue;
            }

            // 初始化上一个时间戳段的结束时间为行的最小开始时间，用于计算第一个 K 标签前的停顿
            let mut last_segment_end_ms = min_start_ms;

            // 第二次遍历时间戳：构建带 {\k} 标签的 ASS 文本
            for &(start_pos, end_pos, current_start_ms, current_duration_ms) in &timestamps {
                // 计算当前时间戳开始时间与上一个时间戳结束时间之间的间隙 (gap)
                if current_start_ms > last_segment_end_ms {
                    let gap_ms = current_start_ms - last_segment_end_ms;
                    // 计算间隙对应的 K 值 (向上取整)
                    let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                    if gap_k_value > 0 {
                        // 如果有间隙，插入一个只有 K 标签的停顿片段
                        ass_text_parts.push(format!("{{\\k{}}}", gap_k_value));
                    }
                }

                // 提取当前时间戳之前的文本片段
                let text_segment = &content[last_char_pos..start_pos];

                // 计算当前文本片段对应的 K 值 (向上取整)
                let k_value = (current_duration_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;

                // 组合 K 标签和文本片段
                if k_value > 0 {
                    // 如果 K > 0，则添加 {\kX}文本
                    ass_text_parts.push(format!("{{\\k{}}}{}", k_value, text_segment));
                } else if !text_segment.is_empty() {
                     // 如果 K = 0 但有文本，只添加文本 (避免产生 {\k0})
                    ass_text_parts.push(text_segment.to_string());
                }
                // 如果 K = 0 且文本为空，则忽略此段

                // 更新下一个片段的处理起始位置和上一个片段的结束时间
                last_segment_end_ms = current_start_ms.saturating_add(current_duration_ms);
                last_char_pos = end_pos;
            }

             // 处理最后一个时间戳之后可能存在的文本
             let remaining_text = &content[last_char_pos..];
             if !remaining_text.is_empty() {
                 // 检查这部分文本是否还有时间（即行结束时间晚于最后一个片段结束时间）
                 if max_end_ms > last_segment_end_ms {
                     let gap_ms = max_end_ms - last_segment_end_ms;
                     let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                     if gap_k_value > 0 {
                         // 如果有时间，给剩余文本加上 K 标签
                         ass_text_parts.push(format!("{{\\k{}}}{}", gap_k_value, remaining_text));
                     } else {
                         // 时间不足 K=1，直接添加文本
                         ass_text_parts.push(remaining_text.to_string());
                     }
                 } else {
                    // 没有剩余时间了，直接添加文本
                    ass_text_parts.push(remaining_text.to_string());
                 }
             } else if max_end_ms > last_segment_end_ms {
                // 没有剩余文本，但还有时间间隙
                 let gap_ms = max_end_ms - last_segment_end_ms;
                 let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                 if gap_k_value > 0 {
                     // 只添加一个 K 标签表示结束前的停顿
                     ass_text_parts.push(format!("{{\\k{}}}", gap_k_value));
                 }
             }


            // 将所有文本片段连接起来，形成最终的 ASS 文本内容
            let final_ass_text = ass_text_parts.join("");

            // 将 LYS 属性值映射回 ASS 的 Name 字段
            let ass_name = match property {
                // 左对齐相关的属性都映射为 "左"
                LYS_PROPERTY_LEFT | LYS_PROPERTY_NO_BACK_LEFT | LYS_PROPERTY_BACK_LEFT => "左",
                // 右对齐相关的属性都映射为 "右"
                LYS_PROPERTY_RIGHT | LYS_PROPERTY_NO_BACK_RIGHT | LYS_PROPERTY_BACK_RIGHT => "右",
                // 有背景但未定左右的属性映射为 "背"
                LYS_PROPERTY_BACK_UNSET => "背",
                // 其他属性 (如 LYS_PROPERTY_UNSET) 映射为空 Name
                _ => "",
            };

            // 将 LYS 行的整体开始/结束时间转换为 ASS 时间格式
            let start_time_ass = milliseconds_to_time(min_start_ms);
            let end_time_ass = milliseconds_to_time(max_end_ms);

            // 检查最终文本是否为空，避免写入空的 Dialogue 行
            if !final_ass_text.is_empty() {
                // 写入 ASS Dialogue 行到输出文件
                writeln!(
                    writer,
                    "Dialogue: 0,{},{},Default,{},0,0,0,,{}", // 使用 Default 样式
                    start_time_ass, end_time_ass, ass_name, final_ass_text
                )?;
            }
        } else if !line.trim().is_empty() && !line.starts_with('[') {
            // 如果行不匹配 LYS 格式 (不是 [数字] 开头)，但也不是空行或元数据行
            // 则记录一个警告，说明可能存在无法识别的数据
             log_warn!("第 {} 行 LYS 数据格式无法识别，已跳过: '{}'", line_number, line);
             warning_occurred_overall = true; // 累积警告
        } // 忽略空行和元数据行

        // 更新进度条
        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    } // 文件行处理循环结束

    // --- 收尾 ---
    display_progress_bar(total_bytes, total_bytes); // 确保进度条显示 100%
    println!(); // 换行

    writer.flush()?; // 确保所有缓冲写入文件
    log_success!("{}", LYS_TO_ASS_COMPLETE); // 打印成功信息
    Ok(warning_occurred_overall) // 返回总的警告状态
}


// --- 辅助函数 ---

/// 将 ASS 文件头和样式信息写入 Writer。
/// 用于 `convert_qrc_to_ass` 和 `convert_lys_to_ass`。
fn write_ass_header(writer: &mut BufWriter<File>) -> io::Result<()> {
    // 写入 [Script Info] 段，包含脚本元信息和播放器参数建议
    writeln!(writer, "[Script Info]")?;
    writeln!(writer, "PlayResX: 1920")?; // 建议播放器渲染分辨率宽度
    writeln!(writer, "PlayResY: 1440")?; // 建议播放器渲染分辨率高度
    writeln!(writer)?;

    // 写入 [V4+ Styles] 段，定义样式
    writeln!(writer, "[V4+ Styles]")?;
    // 定义样式的格式 (字段顺序)
    writeln!(writer, "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding")?;
    // 定义一个名为 "Default" 的样式，可以根据需要修改字体、颜色、边框等参数
    writeln!(writer, "Style: Default,微软雅黑,100,&H00FFFFFF,&H004E503F,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1.5,0.5,2,10,10,60,1")?;
    writeln!(writer)?; // 空行分隔段落

    // 写入 [Events] 段的头部，定义事件（即 Dialogue 行）的格式
    writeln!(writer, "[Events]")?;
    writeln!(writer, "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text")?;

    Ok(()) // 返回成功
}


/// 封装转换函数的执行过程，包括日志打印和错误处理。
/// 返回 `true` 如果发生了错误或（未来可能实现的）需要用户注意的警告，否则返回 `false`。
fn execute_conversion(
    action: fn(&Path, &Path) -> Result<bool, ConversionError>, // 接受一个转换函数作为参数
    input_path: &Path,
    output_path: &Path,
) -> bool { // 返回 bool 表示是否出错/需要等待

    // 执行传入的转换函数 `action`
    match action(input_path, output_path) {
        Ok(_) => {
            // 转换成功，成功日志已在 action 内部打印
            false // 返回 false 表示没有错误发生
        }
        Err(e) => {
            // 转换过程中发生错误
            let formatted_error_msg = format!("{} {}", CONVERSION_ERROR_MSG, e); // 格式化错误信息
            log_error!("{}", formatted_error_msg); // 打印错误日志
            true // 返回 true 表示发生了错误，可能需要等待
        }
    }
}


/// 在程序退出前暂停，等待用户按 Enter 键。
/// 主要用于在命令行模式下，发生错误或警告后给用户时间查看信息。
fn wait_for_exit() {
     log_info!("按下 Enter 键退出..."); // 提示用户操作
     let mut dummy = String::new(); // 用于存储读取的行（内容不重要）
     let _ = io::stdin().read_line(&mut dummy);
}


/// 根据输入路径和目标扩展名，自动生成输出文件的路径。
/// 输出路径与输入路径在同一目录下，文件名添加 "_converted" 后缀。
fn auto_output_path(input_path: &Path, output_ext_with_dot: &str) -> PathBuf {
    // 获取输入文件的文件名（不含扩展名），如果失败则使用 "output" 作为默认值
    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str()) // OsStr -> Option<&str>
        .unwrap_or("output");

    // 构建新的文件名：原始文件名 + "_converted" + 目标扩展名
    let new_file_name = format!("{}_converted{}", file_stem, output_ext_with_dot);

    // 返回与输入文件同目录，但使用新文件名的 PathBuf
    input_path.with_file_name(new_file_name)
}


/// 读取用户在命令行中的单行输入。
///
/// # Arguments
/// * `prompt` - 显示给用户的提示信息。
///
/// # Returns
/// * `Ok(String)` - 用户输入的文本（已去除首尾空白）。
/// * `Err(ConversionError)` - 读取过程中发生 IO 错误。
fn read_user_input(prompt: &str) -> Result<String, ConversionError> {
    print!("{}", prompt); // 显示提示信息
    io::stdout().flush()?; // 确保提示信息立即显示在控制台

    let mut input = String::new(); // 创建一个空字符串用于存储输入
    io::stdin().read_line(&mut input)?; // 从标准输入读取一行
    Ok(input.trim().to_string()) // 去除输入字符串首尾的空白字符并返回
}


/// 读取用户输入的文件路径，并处理路径中可能包含的引号。
///
/// # Arguments
/// * `prompt_template` - 提示信息模板，其中 "{}" 会被 `extension` 替换。
/// * `extension` - 期望的文件扩展名（用于显示在提示信息中）。
///
/// # Returns
/// * `Ok(PathBuf)` - 用户输入的有效文件路径。
/// * `Err(ConversionError)` - 读取或处理路径过程中发生错误。
fn read_file_path(prompt_template: &str, extension: &str) -> Result<PathBuf, ConversionError> {
    // 循环提示用户输入，直到获得非空路径
    loop {
        // 调用 read_user_input 获取用户输入的路径字符串
        let path_str = read_user_input(&prompt_template.replace("{}", extension))?;

        // 清理路径字符串：去除首尾可能存在的单引号或双引号（常见于拖放文件操作）
        let cleaned_path_str = path_str
            .strip_prefix('"').unwrap_or(&path_str) // 尝试移除前导双引号
            .strip_suffix('"').unwrap_or(&path_str) // 尝试移除后置双引号
            .strip_prefix('\'').unwrap_or(&path_str) // 尝试移除前导单引号
            .strip_suffix('\'').unwrap_or(&path_str); // 尝试移除后置单引号

        // 检查清理后的路径是否为空
        if cleaned_path_str.is_empty() {
            // 如果为空，打印错误信息并重新开始循环
            let formatted_msg = format!("{} {}", EMPTY_FILE_PATH_ERROR, extension);
            log_error!("{}", formatted_msg);
            continue;
        }

        // 如果路径非空，则将其转换为 PathBuf 并返回
        return Ok(PathBuf::from(cleaned_path_str));
    }
}


/// 在命令行中显示一个简单的文本进度条。
/// 仅当文件总大小超过阈值时显示。
///
/// # Arguments
/// * `current` - 当前已处理的大小（例如字节数）。
/// * `total` - 文件总大小。
fn display_progress_bar(current: usize, total: usize) {
    // 如果总大小为 0 或小于预设阈值，则不显示进度条
    if total == 0 || total < PROGRESS_BAR_THRESHOLD { return; }

    // 计算当前进度百分比 (限制在 0.0 到 100.0 之间)
    let percentage = (current as f64 / total as f64 * 100.0).min(100.0);
    // 根据百分比计算进度条中已填充部分的长度
    let filled_length = (PROGRESS_BAR_LENGTH as f64 * percentage / 100.0) as usize;
    // 构建进度条的可视化字符串 (例如 "[=======      ]")
    let bar = format!(
        "{}{}", // 由填充部分和空白部分组成
        "=".repeat(filled_length), // 填充部分用 "=" 表示
        " ".repeat(PROGRESS_BAR_LENGTH.saturating_sub(filled_length)) // 空白部分用 " " 表示 (使用 saturating_sub 防止负数)
    );
    // 使用 \r (回车符) 将光标移到行首，实现原地更新进度条
    // 打印格式: "转换进度: [========>     ]  XX% (current/total)"
    print!("\r转换进度: [{}] {:>3.0}% ({}/{})", bar, percentage, current, total);
    // 刷新标准输出流，确保进度条立即显示出来
    let _ = io::stdout().flush(); // 忽略 flush 可能产生的错误
}


/// 将毫秒数转换为 ASS 时间格式字符串 (H:MM:SS.cs)。
fn milliseconds_to_time(ms: usize) -> String {
    // 计算小时、分钟、秒和厘秒
    let hours = ms / MILLISECONDS_PER_HOUR;
    let minutes = (ms % MILLISECONDS_PER_HOUR) / MILLISECONDS_PER_MINUTE;
    let seconds = (ms % MILLISECONDS_PER_MINUTE) / MILLISECONDS_PER_SECOND;
    // 厘秒 = 毫秒部分 / 10
    let centiseconds = (ms % MILLISECONDS_PER_SECOND) / CENTISECONDS_TO_MILLISECONDS;

    // 格式化为 "H:MM:SS.cs" 字符串，注意使用 {:0X} 进行零填充
    format!("{:01}:{:02}:{:02}.{:02}", hours, minutes, seconds, centiseconds)
}

/// 将 ASS 时间格式字符串 (H:MM:SS.cs) 转换为毫秒数。
fn time_to_milliseconds(time_str: &str) -> Result<usize, ConversionError> {
    // 使用 ':' 和 '.' 作为分隔符分割时间字符串
    let parts: Vec<&str> = time_str.split(&[':', '.'][..]).collect();
    // ASS 时间格式必须有 4 个部分 (H, MM, SS, cs)
    if parts.len() != 4 {
        return Err(ConversionError::InvalidFormat(format!(
            "无效 ASS 时间格式: '{}', 期望 H:MM:SS.cs", time_str
        )));
    }

    // 解析每个部分为数字，并乘以相应的毫秒转换系数
    // 使用 `?` 操作符处理 ParseIntError，它会自动通过 From trait 转换为 ConversionError
    let h = parts[0].parse::<usize>()? * MILLISECONDS_PER_HOUR;
    let m = parts[1].parse::<usize>()? * MILLISECONDS_PER_MINUTE;
    let s = parts[2].parse::<usize>()? * MILLISECONDS_PER_SECOND;
    let cs = parts[3].parse::<usize>()? * CENTISECONDS_TO_MILLISECONDS;

    // 返回各部分毫秒数之和
    Ok(h + m + s + cs)
}

/// 检查 ASS 文件中是否存在被视为“特殊”的 Name 字段的 Dialogue 行。
/// “特殊”包括：空、"v1"、"左"、"右"、"v2"、"x-duet"、"x-anti"、"背"、"x-bg"。
/// 用于自动模式判断 ASS 文件应转为 LYS (如果包含特殊名) 还是 QRC。
fn check_ass_has_special_names(ass_path: &Path) -> Result<bool, ConversionError> {
    let file = File::open(ass_path)?;
    let reader = BufReader::new(file);
    let mut after_format = false; // 标记是否已找到 [Events] 段的 Format 行

    for line_result in reader.lines() {
        let line = line_result?;

        // 首先定位到 Format 行
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name,") {
                after_format = true;
            }
            continue;
        }

        // 在 Format 行之后，处理 Dialogue 行
        if line.starts_with("Dialogue:") { // 快速前缀检查
            // 解析行以获取 Name 字段
            if let Some(caps) = ASS_DIALOGUE_REGEX.captures(&line) {
                // 尝试获取 "name" 捕获组
                if let Some(name_match) = caps.name("name") {
                    let name = name_match.as_str(); // 获取 Name 字符串

                    // 使用 matches! 宏检查 name 是否是任何一个需要特殊处理的值
                    if matches!(name,
                        // LeftV1 组 (包含空字符串)
                        "" | "v1" | "左" |
                        // RightV2 组
                        "右" | "v2" | "x-duet" | "x-anti" |
                        // Background 组
                        "背" | "x-bg"
                    ) {
                        return Ok(true); // 只要找到一个符合条件的 Name，就认为文件“特殊”，返回 true
                    }
                }
                // 如果正则匹配但没找到 name 组（理论上不应发生），继续检查下一行
            }
            // 如果行不匹配正则，也继续检查下一行
        }
    } // 文件读取结束

    // 遍历完所有行都没有找到任何特殊 Name，则返回 false
    Ok(false)
}

/// 检查 ASS Dialogue 行定义的总时长与内部所有 {\k} 标签计算出的时长之和是否一致。
///
/// # Arguments
/// * `expected_duration` - Dialogue 行定义的结束时间减去开始时间 (毫秒)。
/// * `actual_duration` - 从行内所有 {\kX} 标签计算出的 X * 10 的总和 (毫秒)。
/// * `line_number` - 当前处理的 Dialogue 在原始文件中的行号 (用于日志)。
///
/// # Returns
/// * `true` - 如果时长一致。
/// * `false` - 如果时长不一致，并会打印警告信息。
fn check_time_consistency(expected_duration: usize, actual_duration: usize, line_number: usize) -> bool {
    // 比较期望时长和实际计算时长
    if expected_duration != actual_duration {
        // 如果不一致，使用 log_warn! 宏打印带颜色的警告信息
        log_warn!(
            "第 {} 行 K tags 时间总和 {}{}{} ms 与行定义持续时间 {}{}{} ms 不匹配",
            line_number,
            RED, actual_duration, RESET, // K 标签总和用红色显示
            GREEN, expected_duration, RESET // 行定义持续时间用绿色显示
        );
        return false; // 返回 false 表示时间不一致
    }
    true // 时间一致，返回 true
}


/// 解析 ASS 元数据 Comment 行的文本部分 (例如 "musicName:歌曲名")
/// 并将其转换为 LRC 风格的元数据标签 (例如 "[ti:歌曲名]")。
///
/// # Arguments
/// * `text` - 从 META_COMMENT_REGEX 捕获到的 Comment 文本内容。
///
/// # Returns
/// * `Some(String)` - 如果成功解析并映射，返回格式化后的标签字符串。
/// * `None` - 如果文本格式不符、键不认识或值为空。
fn parse_ass_metadata_text(text: &str) -> Option<String> {
    // 查找文本中第一个冒号 ":" 的位置
    if let Some(colon_pos) = text.find(':') {
        // 提取冒号前后的部分作为键 (key) 和值 (value)，并去除首尾空格
        let key = text[..colon_pos].trim();
        let value = text[colon_pos + 1..].trim();
        // 根据 ASS 中的键，映射到 LRC 标准或常用的标签
        let tag = match key { // 直接匹配 &str 类型的 key
            "musicName" => "ti", // Title / 歌曲名
            "artists" => "ar", // Artist / 艺术家
            "album" => "al", // Album / 专辑
            "ttmlAuthorGithubLogin" => "by", // Editor / 编辑者 (这里使用了特定的键)
            _ => return None, // 如果键无法识别，则忽略此元数据行
        };
        // 确保值不为空
        if !value.is_empty() {
            // 格式化为 "[标签:值]" 的字符串并返回
            return Some(format!("[{}:{}]", tag, value));
        }
    }
    // 如果没有找到冒号，或者值为空，则返回 None
    None
}


/// 核心辅助函数：解析单行 ASS Dialogue 字符串，提取所有关键信息存入 `ParsedDialogue` 结构体。
///
/// # Arguments
/// * `line` - 要解析的 ASS Dialogue 行字符串。
/// * `line_number` - 该行在原始文件中的行号 (用于错误报告)。
///
/// # Returns
/// * `Ok(Some(ParsedDialogue))` - 如果成功解析。
/// * `Ok(None)` - 如果该行不是有效的 Dialogue 行格式。
/// * `Err(ConversionError)` - 如果解析过程中发生错误 (例如时间格式错误、数字解析错误)。
fn parse_ass_dialogue_line(line: &str, line_number: usize) -> Result<Option<ParsedDialogue>, ConversionError> {
    // 1. 尝试匹配整行结构
    if let Some(caps) = ASS_DIALOGUE_REGEX.captures(line) {
        // 2. 从命名捕获组提取时间字符串
        // 使用 .get().map_or() 或 unwrap() - 这里假设匹配成功则必然存在这些组
        let start_time_str = caps.name("start_time").unwrap().as_str();
        let end_time_str   = caps.name("end_time").unwrap().as_str();

        // 3. 转换时间字符串为毫秒数
        let start_ms = time_to_milliseconds(start_time_str)
            .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 行对话开始时间解析失败: {}", line_number, e)))?;
        let end_ms = time_to_milliseconds(end_time_str)
             .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 行对话结束时间解析失败: {}", line_number, e)))?;
        let duration_ms = end_ms.saturating_sub(start_ms); // 计算行总持续时间

        let style = caps.name("style").unwrap().as_str().trim().to_string(); // 获取 Style 并 trim

        // 4. 提取 Name 字段内容
        let name_str = caps.name("name").unwrap().as_str();
        // 如果 Name 字段不为空，则存入 Some(String)，否则为 None
        let name: Option<String> = if name_str.is_empty() {
            None
        } else {
            Some(name_str.to_string())
        };

        // 5. 提取 Text 字段内容
        let ass_text = caps.name("text").unwrap().as_str();

        // 6. 解析 Text 字段中的 {\k} 标签和对应的文本段
        let mut segments = Vec::new();
        let mut sum_k_ms = 0;
        for k_cap in K_TAG_REGEX.captures_iter(ass_text) {
            let k_cs_str = k_cap.get(1).unwrap().as_str();
            let k_cs: usize = k_cs_str.parse()
                 .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 行 K 数值解析失败 ('{}'): {}", line_number, k_cs_str, e)))?;
            let seg_text = k_cap.get(2).unwrap().as_str().to_string();
            let seg_ms = k_cs * K_TAG_MULTIPLIER;

            sum_k_ms += seg_ms;
            segments.push((seg_text, seg_ms));
        }

        // 7. 构建并返回 ParsedDialogue 结构体
        return Ok(Some(ParsedDialogue {
            line_number,
            start_ms,
            name,
            segments,
            duration_ms,
            sum_k_ms,
            style,
        }));
    } // end if let Some(caps) = ASS_DIALOGUE_REGEX.captures(line)

    // 如果行不匹配 ASS_DIALOGUE_REGEX，则认为不是有效的 Dialogue 行
    Ok(None)
}


/// 辅助函数：处理单行 ASS Dialogue 到 QRC 格式行的转换逻辑。
///
/// # Arguments
/// * `line` - 要转换的 ASS Dialogue 行字符串。
/// * `line_number` - 该行在原始文件中的行号 (用于日志和错误报告)。
/// * `warning_occurred` - 一个可变引用，用于标记是否在此行发生了时间不一致的警告。
///
/// # Returns
/// * `Ok(Some(String))` - 如果成功转换，返回格式化后的 QRC 行字符串。
/// * `Ok(None)` - 如果输入行不是有效的 Dialogue 或解析/转换失败但选择忽略。
/// * `Err(ConversionError)` - 如果发生不可恢复的解析错误。
fn process_dialogue_for_qrc(line: &str, line_number: usize, warning_occurred: &mut bool) -> Result<Option<String>, ConversionError> {
    // 1. 调用统一的解析函数获取解析后的 Dialogue 数据
    match parse_ass_dialogue_line(line, line_number)? {
        Some(parsed_data) => { // 如果解析成功
            // 2. 如果不是 roma/trans/ts 才进行时间一致性检查
            if !(parsed_data.style.eq_ignore_ascii_case("roma")
                 || parsed_data.style.eq_ignore_ascii_case("trans")
                 || parsed_data.style.eq_ignore_ascii_case("ts"))
            {
                // 只有在样式不是 roma/trans/ts 时才调用检查函数
                if !check_time_consistency(parsed_data.duration_ms, parsed_data.sum_k_ms, parsed_data.line_number) {
                    *warning_occurred = true; // 设置警告标志
                }
            }

            // 3. 使用解析出的数据构建 QRC 格式的行字符串
            // QRC 格式: [行开始ms,行持续ms]文本1(绝对开始ms,分段持续ms)文本2(绝对开始ms,分段持续ms)...
            let mut qrc_line = format!("[{},{}]", parsed_data.start_ms, parsed_data.duration_ms);
            let mut current_segment_start_ms = parsed_data.start_ms; // QRC 分段时间戳使用绝对开始时间

            // 遍历解析出的文本段和时长
            for (seg_text, seg_ms) in &parsed_data.segments {
                let segment_duration_ms = *seg_ms; // 解引用得到时长值 (usize)
                // 过滤掉无效的分段 (例如，文本为空且时长为 0)
                if !seg_text.is_empty() || segment_duration_ms > 0 {
                    // 拼接文本和对应的 (开始时间, 持续时间) 标签
                    qrc_line.push_str(&format!("{}({},{})", seg_text, current_segment_start_ms, segment_duration_ms));
                    // 更新下一个分段的理论开始时间
                    current_segment_start_ms += segment_duration_ms;
                }
            }
            // 返回构建好的 QRC 行字符串
            Ok(Some(qrc_line))
        }
        None => { // ASS_DIALOGUE_REGEX 未匹配但行以 "Dialogue:" 开头
        log_warn!("第 {} 行看起来像 Dialogue 但未能通过 parse_ass_dialogue_line 解析。", line_number);
        *warning_occurred = true;
        Ok(None)
    }
    } // end match parse_ass_dialogue_line
}


/// 核心辅助函数：计算当前 Dialogue 行对应的 LYS 属性值。
///
/// # Arguments
/// * `current_dialogue` - 当前正在处理的、已解析的 Dialogue 数据。
/// * `previous_dialogue` - 上一个已处理的 Dialogue 数据 (如果是第一行则为 None)。
/// * `last_calculated_property` - 上一个 Dialogue 行最终计算得到的 LYS 属性值 (用于处理连续 '背' 的情况)。
///
/// # Returns
/// * `usize` - 计算得到的当前行应使用的 LYS 属性常量值。
fn calculate_lys_property(
    current_dialogue: &ParsedDialogue,
    previous_dialogue: Option<&ParsedDialogue>,
    last_calculated_property: usize,
) -> (usize, bool) {
    // 获取当前行的 Name 字段分类，并捕获是否发生警告
    let (current_category, map_warned) = map_ass_name_to_category(current_dialogue.name.as_deref());

    // 2. 根据当前行的分类决定 LYS 属性
    let property = match current_category {
        // LeftV1 分类 (包括空, v1, 左, None) -> 映射为无背景左对齐
        AssNameCategory::LeftV1 => LYS_PROPERTY_NO_BACK_LEFT,
        
        // RightV2 分类 (包括右, v2, x-duet, x-anti) -> 映射为无背景右对齐
        AssNameCategory::RightV2 => LYS_PROPERTY_NO_BACK_RIGHT,

        // Background 分类 (包括背, x-bg) -> 需要根据上一行决定具体属性
        AssNameCategory::Background => {
            // 获取上一行的 Name 字段分类 (如果不存在上一行，则视为 Other)
             let previous_category = previous_dialogue
                .map(|prev| map_ass_name_to_category(prev.name.as_deref()).0) // 只取类别，忽略内部警告（已由map_warned捕获）
                .unwrap_or(AssNameCategory::Other); // 没有前一行时，默认前一行为 Other

            match previous_category {
                AssNameCategory::LeftV1 => LYS_PROPERTY_BACK_LEFT, // 前一行是 LeftV1 -> 有背景左
                AssNameCategory::RightV2 => LYS_PROPERTY_BACK_RIGHT, // 前一行是 RightV2 -> 有背景右
                AssNameCategory::Background => last_calculated_property, // 前一行也是 Background -> 继承上次计算结果
                AssNameCategory::Other => LYS_PROPERTY_BACK_UNSET, // 前一行是 Other -> 有背景未定左右
            }
        }
        
        // Other 分类 -> 映射为未设置属性
        AssNameCategory::Other => LYS_PROPERTY_UNSET,
    };
    (property, map_warned) // 返回计算的属性和map_ass_name_to_category的警告状态
}

/// 将从 ASS 解析出的 Name 字段 (Option<&str>) 映射到对应的内部逻辑分类 `AssNameCategory`。
///
/// 此函数旨在处理 ASS `Dialogue` 行中的 `Name` 字段，即使该字段包含多个由空格分隔的标签
/// （例如 "左 itunes:song-part=..." 或 "v1 extra-tag"）。
/// 它会分析 `Name` 字段的第一个词（"word"，按空格分割）是否为已知的类别关键字。
///
/// # Arguments
/// * `name_opt` - 一个 `Option<&str>`，代表从 ASS 行解析出来的 `Name` 字段。
///   - `None` 表示 ASS 行中没有 `Name` 字段。
///   - `Some("")` 表示 `Name` 字段存在但为空。
///   - `Some("左 anothertag")` 表示 `Name` 字段包含内容。
///
/// # Returns
/// * `AssNameCategory` - 根据 `Name` 字段内容判断出的逻辑分类。
///
/// # 逻辑优先级:
/// 1. 如果 `name_opt` 是 `None` (无 Name 字段) 或 `Some("")` (Name 字段为空)，则归类为 `AssNameCategory::LeftV1`。
/// 2. 否则，获取 `Name` 字段字符串，去除首尾空格。
/// 3. 将处理后的字符串按空格分割，提取第一个词（`first_part`）。
/// 4. 检查 `first_part` 是否匹配以下任一关键字组合：
///    - "左" 或 "v1" -> `AssNameCategory::LeftV1`
///    - "右" 或 "v2" 或 "x-duet" 或 "x-anti" -> `AssNameCategory::RightV2`
///    - "背" 或 "x-bg" -> `AssNameCategory::Background`
/// 5. 如果 `first_part` 不匹配任何已知关键字，则记录一条警告日志，并将该 `Name` 字段归类为 `AssNameCategory::Other`。
///    这适用于如 "路人甲" 或其他非预定义 Actor 名称的情况。
fn map_ass_name_to_category(name_opt: Option<&str>) -> (AssNameCategory, bool) {
    match name_opt {
        // 情况 1: Name 字段不存在或为空
        None | Some("") => (AssNameCategory::LeftV1, false),
        Some(name_str) => {
            // 去除 Name 字段首尾的空格，以确保后续处理的准确性
            let trimmed_name = name_str.trim();
            
            // 如果去除空格后为空字符串，也视为 LeftV1 (例如 Name 字段只包含空格)
            if trimmed_name.is_empty() {
                return (AssNameCategory::LeftV1, false);
            }

            // 按空白字符分割 Name 字段，以分析其组成部分，特别是第一个词。
            // `.next()` 获取迭代器的第一个元素，即按空格分割后的第一个词。
            if let Some(first_part) = trimmed_name.split_whitespace().next() {
                // 情况 4: 检查第一个词是否为已定义的类别关键字
                if matches!(first_part, "左" | "v1" | "合" | "v1000") {
                    return (AssNameCategory::LeftV1, false);
                }
                if matches!(first_part, "右" | "v2" | "x-duet" | "x-anti") {
                    return (AssNameCategory::RightV2, false);
                }
                if matches!(first_part, "背" | "x-bg") {
                    return (AssNameCategory::Background, false);
                }
            }
            // 情况 5: 如果 Name 字段的第一个词不匹配任何已知关键字
            // (或者 Name 字段不包含任何非空白字符，这种情况已被 trimmed_name.is_empty() 捕获)
            // 则记录警告并归类为 Other。
            log_warn!(
                "遇到未定义的 ASS Name 字段值 '{}'，将按默认方式处理。",
                name_str // 记录原始的 name_str 以便调试
            );
            (AssNameCategory::Other, true) // 发生警告
        }
    }
}

/// 移除字符串中所有 ASS 标签 (形如 {\...} 的部分)。
fn strip_ass_tags(text: &str) -> String {
    // 使用 ASS_TAG_REGEX 替换所有匹配项为空字符串
    ASS_TAG_REGEX.replace_all(text, "").into_owned() // into_owned() 将 Cow<str> 转换为 String
}

/// 将毫秒数转换为 LRC 时间格式字符串 [mm:ss.xx] (注意 xx 是百分秒)。
fn milliseconds_to_lrc_time(ms: usize) -> String {
    let minutes = ms / MILLISECONDS_PER_MINUTE; // 计算分钟
    let seconds = (ms % MILLISECONDS_PER_MINUTE) / MILLISECONDS_PER_SECOND; // 计算秒
    // 计算百分秒 (毫秒除以 10)
    let hundredths = (ms % MILLISECONDS_PER_SECOND) / 10;
    // 格式化输出，MM:SS.xx，注意补零
    format!("[{:02}:{:02}.{:02}]", minutes, seconds, hundredths)
}

/// 从 ASS 文件中提取指定样式的翻译行，并按语言生成 LRC 文件。
///
/// # Arguments
/// * `ass_path` - 输入的 ASS 文件路径。
///
/// # Returns
/// * `Ok(())` - 如果提取和写入成功（即使没有找到翻译行）。
/// * `Err(ConversionError)` - 如果发生文件读取或写入错误。
fn extract_translations_to_lrc(ass_path: &Path) -> Result<bool, ConversionError> { // 新签名
    log_info!("开始从 {:?} 提取翻译...", ass_path.file_name().unwrap_or_default());
    let mut warning_occurred_during_extraction = false;

    // 使用 HashMap 存储不同语言的 LRC 行数据
    // Key: 语言代码 (String), Value: Vec<(开始时间ms, 纯文本)>
    let mut translations: HashMap<String, Vec<(usize, String)>> = HashMap::new();

    // --- 读取和解析 ASS 文件 ---
    let file = File::open(ass_path)?;
    let reader = BufReader::new(file);
    let mut after_format = false;
    let mut line_number = 0;

    for line_result in reader.lines() {
        line_number += 1;
        let line = line_result?;

        // 定位到 Format 行
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name,") {
                after_format = true;
            }
            continue;
        }

        // 只处理 Format 行之后的 Dialogue 行
        if line.starts_with("Dialogue:") {
            // 使用合并后的 Regex 解析行
            if let Some(caps) = ASS_DIALOGUE_REGEX.captures(&line) {
                // 安全地获取 Style 和 Name 字段
                let style = caps.name("style").map_or("", |m| m.as_str()).trim();
                let name = caps.name("name").map_or("", |m| m.as_str()).trim();

                // 检查样式是否是翻译样式 ("ts" 或 "trans")
                if style == "ts" || style == "trans" {
                    // 检查 Name 字段是否匹配语言标签格式 "x-lang:..."
                    if let Some(lang_caps) = LANG_TAG_REGEX.captures(name) {
                        // 提取语言代码
                        if let Some(lang_code_match) = lang_caps.name("lang_code") {
                            let lang_code = lang_code_match.as_str().to_lowercase(); // 统一转小写

                            // 提取开始时间和纯文本
                            let start_time_str = caps.name("start_time").unwrap().as_str();
                            let text_with_tags = caps.name("text").unwrap().as_str();

                            match time_to_milliseconds(start_time_str) {
                                Ok(start_ms) => {
                                    // 移除 ASS 标签获取纯文本
                                    let plain_text = strip_ass_tags(text_with_tags);
                                    // 如果纯文本不为空，则添加到对应语言的列表中
                                    if !plain_text.is_empty() {
                                        translations
                                            .entry(lang_code) // 获取或插入该语言的条目
                                            .or_default() // 如果不存在则创建空的 Vec
                                            .push((start_ms, plain_text)); // 添加 (时间, 文本)
                                    }
                                }
                                Err(e) => {
                                    // 报告时间解析错误，但继续处理
                                    log_warn!("第 {} 行翻译时间解析失败: {}", line_number, e);
                                    warning_occurred_during_extraction = true; // 设置警告标志
                                }
                            }
                        }
                    } // end if lang tag matches
                } // end if style is translation
            } // end if dialogue matches
        } // end if starts with Dialogue:
    } // end for line_result

    // --- 写入 LRC 文件 ---
    if translations.is_empty() {
        log_info!("在文件中未找到符合条件的翻译行。");
        return Ok(warning_occurred_during_extraction); // 即使没找到翻译，也可能之前有时间解析警告
    }

    let mut lrc_files_generated = 0;
    // 遍历 HashMap 中每个语言的数据
    for (lang_code, mut lines) in translations {
        // 检查该语言是否有内容行
        if lines.is_empty() { continue; }

        // 1. 按开始时间对行进行排序
        lines.sort_unstable_by_key(|k| k.0);

        // 2. 构建输出 LRC 文件名: 输入文件名(无扩展名).语言代码.lrc
        let lrc_filename = format!(
            "{}.{}.lrc",
            ass_path.file_stem().unwrap_or_default().to_string_lossy(), // 获取文件名（不含扩展名）
            lang_code
        );
        let lrc_output_path = ass_path.with_file_name(lrc_filename);

        log_info!("正在生成翻译文件: {:?}", lrc_output_path.file_name().unwrap_or_default());

        // 3. 创建并写入 LRC 文件
        match File::create(&lrc_output_path) {
            Ok(lrc_file) => {
                let mut lrc_writer = BufWriter::new(lrc_file);
                // 写入元数据（可选，可以考虑从 ASS 元数据传递）
                // writeln!(lrc_writer, "[by:ASS Extractor]")?;
                for (start_ms, text) in lines {
                    // 将毫秒转换为 LRC 时间格式 [mm:ss.xx]
                    let lrc_time = milliseconds_to_lrc_time(start_ms);
                    // 写入 LRC 行
                    if let Err(e) = writeln!(lrc_writer, "{}{}", lrc_time, text) {
                        log_error!("写入 LRC 文件 {:?} 时出错: {}", lrc_output_path, e);
                        // 选择继续尝试写入其他语言，或者直接返回错误
                        // return Err(e.into()); // 如果希望任何写入失败都中止
                        break; // 中止当前文件的写入，尝试下一个语言
                    }
                }
                // 确保写入缓冲区
                if let Err(e) = lrc_writer.flush() {
                     log_error!("刷新 LRC 文件 {:?} 缓冲区时出错: {}", lrc_output_path, e);
                } else {
                    lrc_files_generated += 1;
                }
            }
            Err(e) => {
                log_error!("无法创建 LRC 输出文件 {:?}: {}", lrc_output_path, e);
                // 继续尝试生成其他语言的文件
            }
        } // end match File::create
    } // end for (lang_code, lines)

    if lrc_files_generated > 0 {
        log_success!("成功生成 {} 个 LRC 翻译文件。", lrc_files_generated);
    } else {
        log_warn!("提取过程完成，但未能成功生成任何 LRC 文件（请检查错误信息）。");
        warning_occurred_during_extraction = true; // 标记需要等待
    }

    Ok(warning_occurred_during_extraction)
}

// --- 在 extract_translations_to_lrc 函数之后添加 ---

/// 从 ASS 文件中提取指定样式 ("roma") 的行，并生成罗马音 LRC 文件。
///
/// # Arguments
/// * `ass_path` - 输入的 ASS 文件路径。
///
/// # Returns
/// * `Ok(())` - 如果提取和写入成功（即使没有找到 "roma" 行）。
/// * `Err(ConversionError)` - 如果发生文件读取或写入错误。
fn extract_roma_to_lrc(ass_path: &Path) -> Result<bool, ConversionError> {
    log_info!("开始从 {:?} 提取罗马音 (Style: roma)...", ass_path.file_name().unwrap_or_default());
    let mut warning_occurred_during_extraction = false;

    // 使用 Vec 存储罗马音行的 (开始时间ms, 纯文本)
    let mut roma_lines: Vec<(usize, String)> = Vec::new();

    // --- 读取和解析 ASS 文件 ---
    let file = File::open(ass_path)?;
    let reader = BufReader::new(file);
    let mut after_format = false;
    let mut line_number = 0;

    for line_result in reader.lines() {
        line_number += 1;
        let line = line_result?;

        // 定位到 Format 行
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name,") {
                after_format = true;
            }
            continue;
        }

        // 只处理 Format 行之后的 Dialogue 行
        if line.starts_with("Dialogue:") {
            if let Some(caps) = ASS_DIALOGUE_REGEX.captures(&line) {
                // 获取 Style 字段
                let style = caps.name("style").map_or("", |m| m.as_str()).trim();

                // 检查样式是否为 "roma" (不区分大小写)
                if style.eq_ignore_ascii_case("roma") {
                    // 提取开始时间和纯文本
                    let start_time_str = caps.name("start_time").unwrap().as_str();
                    let text_with_tags = caps.name("text").unwrap().as_str();

                    match time_to_milliseconds(start_time_str) {
                        Ok(start_ms) => {
                            // 移除 ASS 标签获取纯文本
                            let plain_text = strip_ass_tags(text_with_tags);
                            // 如果纯文本不为空，则添加到列表中
                            if !plain_text.is_empty() {
                                roma_lines.push((start_ms, plain_text));
                            }
                        }
                        Err(e) => {
                            log_warn!("第 {} 行罗马音时间解析失败: {}", line_number, e);
                            warning_occurred_during_extraction = true;
                        }
                    }
                } // end if style is roma
            } // end if dialogue matches
        } // end if starts with Dialogue:
    } // end for line_result

    // --- 写入 LRC 文件 ---
    if roma_lines.is_empty() {
        log_info!("在文件中未找到 Style 为 'roma' 的行。");
        return Ok(warning_occurred_during_extraction);
    }

    // 1. 按开始时间对行进行排序
    roma_lines.sort_unstable_by_key(|k| k.0);

    // 2. 构建输出 LRC 文件名: 输入文件名(无扩展名).roma.lrc
    let lrc_filename = format!(
        "{}.roma.lrc", // 固定后缀
        ass_path.file_stem().unwrap_or_default().to_string_lossy(),
    );
    let lrc_output_path = ass_path.with_file_name(lrc_filename);

    log_info!("正在生成罗马音文件: {:?}", lrc_output_path.file_name().unwrap_or_default());

    // 3. 创建并写入 LRC 文件
    match File::create(&lrc_output_path) {
        Ok(lrc_file) => {
            let mut lrc_writer = BufWriter::new(lrc_file);
            // 写入元数据（可选）
            // writeln!(lrc_writer, "[by:ASS Roma Extractor]")?;
            for (start_ms, text) in roma_lines {
                let lrc_time = milliseconds_to_lrc_time(start_ms);
                if let Err(e) = writeln!(lrc_writer, "{}{}", lrc_time, text) {
                    log_error!("写入罗马音 LRC 文件 {:?} 时出错: {}", lrc_output_path, e);
                    return Err(e.into()); // 写入失败则直接返回错误
                }
            }
            // 确保写入缓冲区
            if let Err(e) = lrc_writer.flush() {
                 log_error!("刷新罗马音 LRC 文件 {:?} 缓冲区时出错: {}", lrc_output_path, e);
                 return Err(e.into());
            } else {
                log_success!("成功生成罗马音 LRC 文件。");
            }
        }
        Err(e) => {
            log_error!("无法创建罗马音 LRC 输出文件 {:?}: {}", lrc_output_path, e);
            return Err(e.into()); // 创建文件失败也返回错误
        }
    }

    Ok(warning_occurred_during_extraction)
}