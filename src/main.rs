use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::num::ParseIntError;
use std::error::Error;
use std::fmt;
use once_cell::sync::Lazy;
use regex::Regex;

const ASS_TO_QRC_CHOICE: &str = "1";
const QRC_TO_ASS_CHOICE: &str = "2";
const ASS_EXTENSION: &str = ".ass";
const QRC_EXTENSION: &str = ".qrc";
const INVALID_CHOICE_MESSAGE: &str = "无效选择";
const INPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: ";
const OUTPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: ";
const EMPTY_FILE_PATH_ERROR: &str = "输入的 {} 文件路径不能为空";
const FILE_NOT_FOUND_ERROR: &str = "错误: 输入文件不存在";
const ASS_TO_QRC_COMPLETE: &str = "ASS -> QRC 转换完成！";
const QRC_TO_ASS_COMPLETE: &str = "QRC -> ASS 转换完成！";
const CONVERSION_ERROR_MSG: &str = "转换过程中发生错误: {}";
const READ_INPUT_ERROR: &str = "读取用户输入失败";

const MILLISECONDS_PER_SECOND: usize = 1000;
const MILLISECONDS_PER_MINUTE: usize = 60 * MILLISECONDS_PER_SECOND;
const MILLISECONDS_PER_HOUR: usize = 60 * MILLISECONDS_PER_MINUTE;
const CENTISECONDS_TO_MILLISECONDS: usize = 10; 
const K_TAG_MULTIPLIER: usize = 10; 
const QRC_GAP_THRESHOLD_MS: usize = 200; 

const PROGRESS_BAR_LENGTH: usize = 20;

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";

macro_rules! log_info {
    ($($arg:tt)*) => {
        println!("{}[提示]{} {}", CYAN, RESET, format!($($arg)*));
    }
}
macro_rules! log_success {
    ($($arg:tt)*) => {
        println!("{}[成功]{} {}", GREEN, RESET, format!($($arg)*));
    }
}
macro_rules! log_warn {
    ($($arg:tt)*) => {
        eprintln!("{}[警告]{} {}", YELLOW, RESET, format!($($arg)*));
    }
}
macro_rules! log_error {
    ($($arg:tt)*) => {
        eprintln!("{}[错误]{} {}", RED, RESET, format!($($arg)*));
    }
}


#[derive(Debug)]
enum ConversionError {
    Io(io::Error),
    Regex(regex::Error),
    ParseInt(ParseIntError),
    InvalidFormat(String), 
    UserInputError(String),
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::Io(e) => write!(f, "I/O 错误: {}", e),
            ConversionError::Regex(e) => write!(f, "正则表达式错误: {}", e),
            ConversionError::ParseInt(e) => write!(f, "数字解析错误: {}", e),
            ConversionError::InvalidFormat(msg) => write!(f, "格式无效: {}", msg),
            ConversionError::UserInputError(msg) => write!(f, "用户输入错误: {}", msg),
        }
    }
}

impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConversionError::Io(e) => Some(e),
            ConversionError::Regex(e) => Some(e),
            ConversionError::ParseInt(e) => Some(e),
            _ => None,
        }
    }
}

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

static DIALOGUE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Dialogue:\s*\d+,(\d+:\d+:\d+\.\d+),(\d+:\d+:\d+\.\d+),").expect("")
});
static K_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\{\\k(\d+)\}([^\\{]*)").expect("")
});
static QRC_TIMESTAMP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[(\d+),(\d+)\]").expect("")
});
static WORD_TIME_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([^\(\)]*)(?:\((\d+),(\d+)\))?").expect("")
});

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        interactive_mode();
    } else if args.len() == 1 {
        let input_path = PathBuf::from(&args[0]);
        if !input_path.exists() {
            log_error!("{}", FILE_NOT_FOUND_ERROR); 
            return;
        }
        let extension = input_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let (conversion_action, _input_ext, output_ext) = if extension == &ASS_EXTENSION[1..] {
            (convert_ass_to_qrc as fn(&Path, &Path) -> Result<(), ConversionError>, ASS_EXTENSION, QRC_EXTENSION)
        } else if extension == &QRC_EXTENSION[1..] {
            (convert_qrc_to_ass as fn(&Path, &Path) -> Result<(), ConversionError>, QRC_EXTENSION, ASS_EXTENSION)
        } else {
            log_error!("无法根据文件后缀判断转换方向，请使用正确的 .ass 或 .qrc 文件");
            eprintln!("");
            log_info!("按下任意键退出...");
                let mut dummy = String::new();
                let _ = io::stdin().read_line(&mut dummy);
                return;
        };
        let output_path = auto_output_path(&input_path, output_ext);
        if let Err(e) = conversion_action(&input_path, &output_path) {
            log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
        }
    } else if args.len() == 3 {
        let direction = args[0].to_lowercase();
        let input_path = PathBuf::from(&args[1]);
        let output_path = PathBuf::from(&args[2]);
        if !input_path.exists() {
            log_error!("{}", FILE_NOT_FOUND_ERROR);
            return;
        }
        let conversion_action = match direction.as_str() {
            "ass2qrc" | "2q" => convert_ass_to_qrc as fn(&Path, &Path) -> Result<(), ConversionError>,
            "qrc2ass" | "2a" => convert_qrc_to_ass as fn(&Path, &Path) -> Result<(), ConversionError>,
            _ => {
                log_error!("转换方向参数无效，请使用 'ass2qrc'(或2q) 或 'qrc2ass'(或2a)");
                eprintln!("");
                log_info!("按下任意键退出...");
                let mut dummy = String::new();
                let _ = io::stdin().read_line(&mut dummy);
                return;
            }
        };
        if let Err(e) = conversion_action(&input_path, &output_path) {
            eprintln!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
        }
    } else {
        eprintln!("用法：QRCandASSConverter-rust [convert_direction] [input_file] [output_file]");
        eprintln!("  convert_direction: ass2qrc (或 2q), qrc2ass (或 2a)");
        eprintln!("  只有单个文件参数时自动判断转换方向并在该文件目录下生成转换文件");
    }
}

fn auto_output_path(input_path: &Path, output_ext: &str) -> PathBuf {
    let mut file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_owned();
    file_stem.push_str("_converted");
    let mut output_path = input_path.with_file_name(file_stem);
    output_path.set_extension(&output_ext[1..]); 
    output_path
}

fn interactive_mode() {
    log_info!("直接将文件拖到程序上可自动转换");
    loop {
        println!("");
        println!("请选择操作：");
        println!("{}. ASS -> QRC",ASS_TO_QRC_CHOICE);
        println!("{}. QRC -> ASS",QRC_TO_ASS_CHOICE);
        let mut choice = String::new();
        if io::stdin().read_line(&mut choice).is_err() {
             log_error!("{}", READ_INPUT_ERROR); 
             continue;
        }
        let choice = choice.trim();

        if choice.is_empty() {
            log_error!("{}", INVALID_CHOICE_MESSAGE); 
            continue;
        }

        let result = match choice {
            ASS_TO_QRC_CHOICE => process_conversion(convert_ass_to_qrc, ASS_EXTENSION, QRC_EXTENSION),
            QRC_TO_ASS_CHOICE => process_conversion(convert_qrc_to_ass, QRC_EXTENSION, ASS_EXTENSION),
            _ => {
                log_error!("{}", INVALID_CHOICE_MESSAGE); 
                continue; 
            }
        };

        if let Err(e) = result {
             log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
        }
    }
}

fn read_file_path(prompt_template: &str, extension: &str) -> Result<PathBuf, ConversionError> {
    loop {
        print!("{}", prompt_template.replace("{}", extension));
        io::stdout().flush().map_err(|e| ConversionError::UserInputError(format!("刷新标准输出失败: {}", e)))?;

        let mut path_str = String::new();
        io::stdin().read_line(&mut path_str)
                   .map_err(|e| ConversionError::UserInputError(format!("读取路径失败: {}", e)))?;

        let path_str = path_str.trim();

        if path_str.is_empty() {
            log_error!("{}", EMPTY_FILE_PATH_ERROR.replace("{}", extension)); 
            continue;
        }

        return Ok(PathBuf::from(path_str));
    }
}

fn process_conversion(
    conversion_action: fn(&Path, &Path) -> Result<(), ConversionError>,
    input_extension: &str,
    output_extension: &str,
) -> Result<(), ConversionError> {
    let input_path = read_file_path(INPUT_FILE_PATH_PROMPT, input_extension)?;
    let output_path = read_file_path(OUTPUT_FILE_PATH_PROMPT, output_extension)?;

    if !input_path.exists() {
        log_error!("{}", FILE_NOT_FOUND_ERROR);
         return Err(ConversionError::UserInputError("输入文件不存在".to_string()));
    }

    conversion_action(&input_path, &output_path) 
}

fn display_progress_bar(current: usize, total: usize) {
     if total == 0 { return; } 
    let percentage = (current as f64 / total as f64 * 100.0).min(100.0) as usize; 
    let filled_length = (PROGRESS_BAR_LENGTH as f64 * percentage as f64 / 100.0) as usize;
    let bar = "=".repeat(filled_length) + &" ".repeat(PROGRESS_BAR_LENGTH.saturating_sub(filled_length)); 
    print!("\r[{}] {}% ({}/{})", bar, percentage, current, total);
    let _ = io::stdout().flush();
}


fn convert_ass_to_qrc(ass_path: &Path, qrc_path: &Path) -> Result<(), ConversionError> {
    let file = File::open(ass_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes = 0;
    let mut warning = false;

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(qrc_path)?);

    let mut after_format = false;
    let mut dialogue_count = 0;

    for line_result in reader.lines() {
        let line = line_result?;

        let line_bytes = line.as_bytes().len() + if cfg!(windows) { 2 } else { 1 };
        processed_bytes += line_bytes;

        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text") {
                after_format = true;
            }
            continue;
        }

        if line.starts_with("Dialogue:") {
            dialogue_count += 1;

            if let Some(caps) = DIALOGUE_REGEX.captures(&line) {
                let start_time_str = caps.get(1).unwrap().as_str();
                let end_time_str   = caps.get(2).unwrap().as_str();

                let start_ms = time_to_milliseconds(start_time_str)
                    .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 条对话 开始时间格式错误: {}", dialogue_count, e)))?;
                let end_ms = time_to_milliseconds(end_time_str)
                    .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 条对话 结束时间格式错误: {}", dialogue_count, e)))?;
                let duration_ms = end_ms.saturating_sub(start_ms);

                let text_part_start = caps.get(0).unwrap().end();
                let ass_text = &line[text_part_start..];
                let mut segments = Vec::new();
                let mut sum_k_ms = 0;
                for k_cap in K_TAG_REGEX.captures_iter(ass_text) {
                    let k_cs: usize = k_cap.get(1).unwrap().as_str().parse()?;
                    let seg_text = k_cap.get(2).unwrap().as_str().to_string();
                    let seg_ms = k_cs * K_TAG_MULTIPLIER;
                    sum_k_ms += seg_ms;
                    segments.push((seg_text, seg_ms));
                }

                if sum_k_ms != duration_ms {
                    log_warn!(
                        "第 {} 行 k 值总和 {} ms 与持续时间 {} ms 不匹配",
                        dialogue_count,
                        sum_k_ms,
                        duration_ms
                    );
                    warning = true;
                }

                write!(writer, "[{},{}]", start_ms, duration_ms)?;
                let mut current_ms = start_ms;
                for (seg_text, seg_ms) in segments {
                    write!(writer, "{}({},{})", seg_text, current_ms, seg_ms)?;
                    current_ms += seg_ms;
                }
                writeln!(writer)?;
            }
        }
        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    }

    display_progress_bar(total_bytes, total_bytes);

    writer.flush().map_err(ConversionError::Io)?;
    println!("");
    log_success!("{}", ASS_TO_QRC_COMPLETE);

    if warning {
        eprintln!("");
        log_warn!("一行或多行文字总时间与行持续时间不匹配，建议修改原文件后再次转换");
        eprintln!("");
        log_info!("按下任意键退出...");
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy).map_err(ConversionError::Io)?;
    }

    Ok(())
}

fn convert_qrc_to_ass(qrc_path: &Path, ass_path: &Path) -> Result<(), ConversionError> {
    let file = File::open(qrc_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes = 0;

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(ass_path)?);

    writeln!(writer, "[Events]")?;
    writeln!(writer, "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text")?;

    for (_line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        processed_bytes += line.len() + 1; 

        if !line.starts_with('[') {
            display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
            continue;
        }

        if let Some(ts_caps) = QRC_TIMESTAMP_REGEX.captures(&line) {
            let header_start_ms: usize = ts_caps[1].parse()?;
            let header_duration_ms: usize = ts_caps[2].parse()?;
            let header_end_ms = header_start_ms + header_duration_ms;

            let start_time_ass = milliseconds_to_time(header_start_ms);
            let end_time_ass = milliseconds_to_time(header_end_ms);

            let mut ass_text = String::new();
            let mut last_word_end_ms = header_start_ms;

            let content_part = &line[ts_caps.get(0).unwrap().end()..];

            for word_cap in WORD_TIME_TAG_REGEX.captures_iter(content_part) {
                let word = word_cap.get(1).map(|m| m.as_str()).unwrap_or("");
                if word.is_empty() { continue; }

                if let (Some(ts_match), Some(dur_match)) = (word_cap.get(2), word_cap.get(3)) {
                    if ts_match.as_str() == "0" && dur_match.as_str() == "0" {
                        ass_text.push_str(word);
                        continue;
                    }

                    let current_word_start_ms: usize = ts_match.as_str().parse()?;
                    let current_word_duration_ms: usize = dur_match.as_str().parse()?;

                    if current_word_start_ms > last_word_end_ms {
                        let gap_ms = current_word_start_ms - last_word_end_ms;
                        let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                        if gap_k_value > 0 {
                            ass_text.push_str(&format!("{{\\k{}}}", gap_k_value));
                        }
                    }

                    let word_k_value = (current_word_duration_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER; 
                    ass_text.push_str(&format!("{{\\k{}}}{}", word_k_value, word));

                    last_word_end_ms = current_word_start_ms + current_word_duration_ms;
                    
                } else {
                    ass_text.push_str(word);
                }
            }

            if last_word_end_ms < header_end_ms && (header_end_ms - last_word_end_ms) > QRC_GAP_THRESHOLD_MS {
                let final_gap_ms = header_end_ms - last_word_end_ms;
                let final_gap_k_value = (final_gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                if final_gap_k_value > 0 {
                    ass_text.push_str(&format!("{{\\k{}}}", final_gap_k_value));
                }
            }

            let ass_text = ass_text.replace("{\\k0}", "");
            if !ass_text.is_empty() {
                writeln!(
                    writer,
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}", 
                    start_time_ass,
                    end_time_ass,
                    ass_text 
                )?;
            }
        }

        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    }

    display_progress_bar(total_bytes, total_bytes);
    println!("");
    log_success!("{}", QRC_TO_ASS_COMPLETE);
    Ok(())
}

fn milliseconds_to_time(ms: usize) -> String {
    let hours = ms / MILLISECONDS_PER_HOUR;
    let remaining = ms % MILLISECONDS_PER_HOUR;
    let minutes = remaining / MILLISECONDS_PER_MINUTE;
    let remaining = remaining % MILLISECONDS_PER_MINUTE;
    let seconds = remaining / MILLISECONDS_PER_SECOND;
    let centiseconds = (remaining % MILLISECONDS_PER_SECOND) / CENTISECONDS_TO_MILLISECONDS;

    format!("{:01}:{:02}:{:02}.{:02}", hours, minutes, seconds, centiseconds)
}

fn time_to_milliseconds(time_str: &str) -> Result<usize, ConversionError> {
    let parts: Vec<&str> = time_str.split(&[':', '.'][..]).collect();
    if parts.len() != 4 {
        return Err(ConversionError::InvalidFormat(format!("时间格式错误: {}", time_str)));
    }

    let h = parts[0].parse::<usize>()? * MILLISECONDS_PER_HOUR;
    let m = parts[1].parse::<usize>()? * MILLISECONDS_PER_MINUTE;
    let s = parts[2].parse::<usize>()? * MILLISECONDS_PER_SECOND;
    let cs = parts[3].parse::<usize>()? * CENTISECONDS_TO_MILLISECONDS;

    Ok(h + m + s + cs)
}
