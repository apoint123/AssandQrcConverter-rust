use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::num::ParseIntError;
use std::error::Error;
use std::fmt;
use once_cell::sync::Lazy;
use regex::Regex;

const ASS_EXTENSION: &str = ".ass";
const QRC_EXTENSION: &str = ".qrc";
const LYRICIFY_EXTENSION: &str = ".lys"; 
const INVALID_CHOICE_MESSAGE: &str = "无效选择";
const INPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: ";
const OUTPUT_FILE_PATH_PROMPT: &str = "请输入 {} 文件路径: ";
const EMPTY_FILE_PATH_ERROR: &str = "输入的 {} 文件路径不能为空";
const FILE_NOT_FOUND_ERROR: &str = "错误: 输入文件不存在";
const ASS_TO_QRC_COMPLETE: &str = "ASS -> QRC 转换完成！\n";
const QRC_TO_ASS_COMPLETE: &str = "QRC -> ASS 转换完成！\n";
const ASS_TO_LYS_COMPLETE: &str = "ASS -> Lyricify Syllable 转换完成！\n"; 
const LYS_TO_ASS_COMPLETE: &str = "Lyricify Syllable -> ASS 转换完成！\n";
const CONVERSION_ERROR_MSG: &str = "转换过程中发生错误: {}";
const READ_INPUT_ERROR: &str = "读取用户输入失败";
const ASS_FORMAT_CHOICE: &str = "1";
const QRC_FORMAT_CHOICE: &str = "2";
const LYS_FORMAT_CHOICE: &str = "3";

const MILLISECONDS_PER_SECOND: usize = 1000;
const MILLISECONDS_PER_MINUTE: usize = 60 * MILLISECONDS_PER_SECOND;
const MILLISECONDS_PER_HOUR: usize = 60 * MILLISECONDS_PER_MINUTE;
const CENTISECONDS_TO_MILLISECONDS: usize = 10; 
const K_TAG_MULTIPLIER: usize = 10; 
const QRC_GAP_THRESHOLD_MS: usize = 200; 

const PROGRESS_BAR_LENGTH: usize = 20;
const PROGRESS_BAR_THRESHOLD: usize = 64 * 1024 * 1024; // 64MB

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";

// Lyricify Syllable 属性
const LYS_PROPERTY_UNSET: usize = 0;
const LYS_PROPERTY_LEFT: usize = 1;
const LYS_PROPERTY_RIGHT: usize = 2;
//const LYS_PROPERTY_NO_BACK_UNSET: usize = 3;
//const LYS_PROPERTY_NO_BACK_LEFT: usize = 4;
//const LYS_PROPERTY_NO_BACK_RIGHT: usize = 5;
const LYS_PROPERTY_BACK_UNSET: usize = 6;
const LYS_PROPERTY_BACK_LEFT: usize = 7;
const LYS_PROPERTY_BACK_RIGHT: usize = 8;

macro_rules! log_info {
    ($($arg:tt)*) => {
        println!("\n{}[提示]{} {}", CYAN, RESET, format!($($arg)*));
    }
}
macro_rules! log_success {
    ($($arg:tt)*) => {
        println!("\n{}[成功]{} {}", GREEN, RESET, format!($($arg)*));
    }
}
macro_rules! log_warn {
    ($($arg:tt)*) => {
        eprintln!("\n{}[警告]{} {}", YELLOW, RESET, format!($($arg)*));
    }
}
macro_rules! log_error {
    ($($arg:tt)*) => {
        eprintln!("\n{}[错误]{} {}", RED, RESET, format!($($arg)*));
    }
}


#[derive(Debug)]
enum ConversionError {
    Io(io::Error),
    Regex(regex::Error),
    ParseInt(ParseIntError),
    InvalidFormat(String), 
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::Io(e) => write!(f, "I/O 错误: {}", e),
            ConversionError::Regex(e) => write!(f, "正则表达式错误: {}", e),
            ConversionError::ParseInt(e) => write!(f, "数字解析错误: {}", e),
            ConversionError::InvalidFormat(msg) => write!(f, "格式无效: {}", msg),
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
    Regex::new(r"Dialogue:\s*\d+,(\d+:\d+:\d+\.\d+),(\d+:\d+:\d+\.\d+),").expect("未能编译DIALOGUE_REGEX")
});
static K_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\{\\k[f]?(\d+)\}([^\\{]*)").expect("未能编译K_TAG_REGEX")
});
static QRC_TIMESTAMP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[(\d+),(\d+)\]").expect("未能编译QRC_TIMESTAMP_REGEX")
});
static WORD_TIME_TAG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([^\(\)]*)(?:\((\d+),(\d+)\))?").expect("未能编译WORD_TIME_TAG_REGEX")
});
static ASS_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Dialogue:\s*\d+,[^,]+,[^,]+,[^,]+,([^,]*),").expect("未能编译ASS_NAME_REGEX")
});
static LYS_PROPERTY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[(\d+)\](.*)").expect("未能编译LYS_PROPERTY_REGEX")
});
static LYS_WORD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([^\(\)]*)(?:\((\d+),(\d+)\))?").expect("未能编译LYS_WORD_REGEX")
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
        if extension == LYRICIFY_EXTENSION[1..] { 
                let output_path = auto_output_path(&input_path, ASS_EXTENSION);
                if let Err(e) = convert_lys_to_ass(&input_path, &output_path) {
                    log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
                }
                return; 
        }
        
        if extension == ASS_EXTENSION[1..] {
            if let Ok(has_special_names) = check_ass_has_special_names(&input_path) {
                if has_special_names {
                    let output_path = auto_output_path(&input_path, LYRICIFY_EXTENSION);
                    if let Err(e) = convert_ass_to_lys(&input_path, &output_path) {
                        log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
                    }
                    return;
                }
            }
            let output_path = auto_output_path(&input_path, QRC_EXTENSION);
            if let Err(e) = convert_ass_to_qrc(&input_path, &output_path) {
                log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
            }
        } else if extension == QRC_EXTENSION[1..] {
            let output_path = auto_output_path(&input_path, ASS_EXTENSION);
            if let Err(e) = convert_qrc_to_ass(&input_path, &output_path) {
                log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
            }
        } else {
            log_error!("无法根据文件后缀判断转换方向，请使用正确的 .ass 或 .qrc 文件");
            log_info!("按下任意键退出...");
                let mut dummy = String::new();
                let _ = io::stdin().read_line(&mut dummy);
                return;
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
            "ass2lys" | "2l" => convert_ass_to_lys as fn(&Path, &Path) -> Result<(), ConversionError>,
            "lys2ass" | "l2a" => convert_lys_to_ass as fn(&Path, &Path) -> Result<(), ConversionError>, 
            _ => {
                log_error!("转换方向参数无效，请使用 'ass2qrc'(或2q), 'qrc2ass'(或2a), 'ass2lys'(或2l) 或 'lys2ass'(或l2a)");
                log_info!("按下任意键退出...");
                let mut dummy = String::new();
                let _ = io::stdin().read_line(&mut dummy);
                return;
            }
        };
        if let Err(e) = conversion_action(&input_path, &output_path) {
            log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
        }
    } else {
        eprintln!("用法：QRCandASSConverter-rust [convert_direction] [input_file] [output_file]");
        eprintln!("  convert_direction: ass2qrc (或 2q), qrc2ass (或 2a), ass2lys (或 2l), lys2ass (或 l2a)");
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
    log_info!("直接将文件拖到程序上可自动转换\n");
    loop {
        println!("请选择源文件格式：");
        println!("{}. ASS 文件", ASS_FORMAT_CHOICE);
        println!("{}. QRC 文件", QRC_FORMAT_CHOICE);
        println!("{}. Lyricify Syllable 文件", LYS_FORMAT_CHOICE);
        
        let mut source_choice = String::new();
        if io::stdin().read_line(&mut source_choice).is_err() {
            log_error!("{}", READ_INPUT_ERROR); 
            continue;
        }
        let source_choice = source_choice.trim();

        if source_choice.is_empty() {
            log_error!("{}", INVALID_CHOICE_MESSAGE); 
            continue;
        }

        let (source_extension, target_options) = match source_choice {
            ASS_FORMAT_CHOICE => {
                log_info!("请选择目标文件格式：");
                println!("{}. QRC 文件", QRC_FORMAT_CHOICE);
                println!("{}. Lyricify Syllable 文件", LYS_FORMAT_CHOICE);
                (ASS_EXTENSION, vec![QRC_FORMAT_CHOICE, LYS_FORMAT_CHOICE])
            },
            QRC_FORMAT_CHOICE => {
                log_info!("请选择目标文件格式：");
                println!("{}. ASS 文件", ASS_FORMAT_CHOICE);
                (QRC_EXTENSION, vec![ASS_FORMAT_CHOICE])
            },
            LYS_FORMAT_CHOICE => { 
                log_info!("请选择目标文件格式：");
                println!("{}. ASS 文件", ASS_FORMAT_CHOICE);
                (LYRICIFY_EXTENSION, vec![ASS_FORMAT_CHOICE])
            },
            _ => {
                log_error!("{}", INVALID_CHOICE_MESSAGE); 
                continue;
            }
        };

        let mut target_choice = String::new();
        if io::stdin().read_line(&mut target_choice).is_err() {
            log_error!("{}", READ_INPUT_ERROR); 
            continue;
        }
        let target_choice = target_choice.trim();

        if target_choice.is_empty() || !target_options.contains(&target_choice) {
            log_error!("{}", INVALID_CHOICE_MESSAGE); 
            continue;
        }

        let (target_extension, conversion_action) = match (source_choice, target_choice) {
            (ASS_FORMAT_CHOICE, QRC_FORMAT_CHOICE) => 
                (QRC_EXTENSION, convert_ass_to_qrc as fn(&Path, &Path) -> Result<(), ConversionError>),
            (ASS_FORMAT_CHOICE, LYS_FORMAT_CHOICE) => 
                (LYRICIFY_EXTENSION, convert_ass_to_lys as fn(&Path, &Path) -> Result<(), ConversionError>),
            (QRC_FORMAT_CHOICE, ASS_FORMAT_CHOICE) => 
                (ASS_EXTENSION, convert_qrc_to_ass as fn(&Path, &Path) -> Result<(), ConversionError>),
            (LYS_FORMAT_CHOICE, ASS_FORMAT_CHOICE) => 
                (ASS_EXTENSION, convert_lys_to_ass as fn(&Path, &Path) -> Result<(), ConversionError>),
            _ => {
                log_error!("无效的转换组合"); 
                continue;
            }
        };

        let input_path = match read_file_path(INPUT_FILE_PATH_PROMPT, source_extension) {
            Ok(path) => path,
            Err(e) => {
                log_error!("{}", e);
                continue;
            }
        };

        if !input_path.exists() {
            log_error!("{}", FILE_NOT_FOUND_ERROR);
            continue;
        }

        let output_path = match read_file_path(OUTPUT_FILE_PATH_PROMPT, target_extension) {
            Ok(path) => path,
            Err(e) => {
                log_error!("{}", e);
                continue;
            }
        };

        if let Err(e) = conversion_action(&input_path, &output_path) {
            log_error!("{}", CONVERSION_ERROR_MSG.replace("{}", &e.to_string()));
        }
    }
}

fn read_file_path(prompt_template: &str, extension: &str) -> Result<PathBuf, ConversionError> {
    loop {
        print!("{}", prompt_template.replace("{}", extension));
        io::stdout().flush()?;
        
        let mut path_str = String::new();
        io::stdin().read_line(&mut path_str)?;

        let path_str = path_str.trim();

        if path_str.is_empty() {
            log_error!("{}", EMPTY_FILE_PATH_ERROR.replace("{}", extension)); 
            continue;
        }

        return Ok(PathBuf::from(path_str));
    }
}

fn display_progress_bar(current: usize, total: usize) {
    if total == 0 { return; }
    if total < PROGRESS_BAR_THRESHOLD { return; }
    
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

        let line_bytes = line.len() + if cfg!(windows) { 2 } else { 1 };
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

                if !check_time_consistency(duration_ms, sum_k_ms, dialogue_count) {
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

    writer.flush()?;
    log_success!("{}", ASS_TO_QRC_COMPLETE);

    if warning {
        log_warn!("一行或多行文字总时间与行持续时间不匹配，建议修改原文件后再次转换");
        log_info!("按下任意键退出...");
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy)?;
    }

    Ok(())
}

fn convert_qrc_to_ass(qrc_path: &Path, ass_path: &Path) -> Result<(), ConversionError> {
    let file = File::open(qrc_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes = 0;
    let mut warning = false;

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(ass_path)?);

    writeln!(writer, "[Script Info]")?;
    writeln!(writer, "PlayResX: 1920")?;
    writeln!(writer, "PlayResY: 1440")?;
    writeln!(writer)?;
    
    writeln!(writer, "[V4+ Styles]")?;
    writeln!(writer, "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding")?;
    writeln!(writer, "Style: Default,微软雅黑,100,&H00FFFFFF,&H004E503F,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,1")?;
    writeln!(writer)?;

    writeln!(writer, "[Events]")?;
    writeln!(writer, "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text")?;

    let mut line_count = 0;

    for line_result in reader.lines() {
        let line = line_result?;
        processed_bytes += line.len() + 1; 
        line_count += 1;

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
            let mut total_word_duration = 0;

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
                    total_word_duration += current_word_duration_ms;

                    if current_word_start_ms > last_word_end_ms {
                        let gap_ms = current_word_start_ms - last_word_end_ms;
                        let gap_k_value = (gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                        if gap_k_value > 0 {
                            ass_text.push_str(&format!("{{\\kf{}}}", gap_k_value));
                            total_word_duration += gap_ms;
                        }
                    }

                    let word_k_value = (current_word_duration_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER; 
                    ass_text.push_str(&format!("{{\\kf{}}}{}", word_k_value, word));

                    last_word_end_ms = current_word_start_ms + current_word_duration_ms;
                    
                } else {
                    ass_text.push_str(word);
                }
            }

            if last_word_end_ms < header_end_ms && (header_end_ms - last_word_end_ms) > QRC_GAP_THRESHOLD_MS {
                let final_gap_ms = header_end_ms - last_word_end_ms;
                let final_gap_k_value = (final_gap_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                if final_gap_k_value > 0 {
                    ass_text.push_str(&format!("{{\\kf{}}}", final_gap_k_value));
                    total_word_duration += final_gap_ms;
                }
            }

            if !check_time_consistency(header_duration_ms, total_word_duration, line_count) {
                warning = true;
            }

            let ass_text = ass_text.replace("{\\kf0}", "");
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
    log_success!("{}", QRC_TO_ASS_COMPLETE);
    
    if warning {
        log_warn!("一行或多行文字总时间与行持续时间不匹配，建议修改原文件后再次转换");
        log_info!("按下任意键退出...");
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy)?;
    }
    
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

fn convert_ass_to_lys(ass_path: &Path, lys_path: &Path) -> Result<(), ConversionError> {
    let file = File::open(ass_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut warning = false;

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(lys_path)?);

    let mut after_format = false;
    let mut dialogue_count = 0;
    let mut last_property = 0;  
    let mut dialogues = Vec::new(); 

    for line_result in reader.lines() {
        let line = line_result?;
        
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text") {
                after_format = true;
            }
            continue;
        }

        if line.starts_with("Dialogue:") {
            dialogues.push(line);
        }
    }

    for (i, line) in dialogues.iter().enumerate() {
        dialogue_count += 1;
        let processed_bytes = (dialogue_count * 100).min(total_bytes);

        if let Some(caps) = DIALOGUE_REGEX.captures(line) {
            let start_time_str = caps.get(1).unwrap().as_str();
            let end_time_str = caps.get(2).unwrap().as_str();

            let start_ms = time_to_milliseconds(start_time_str)
                .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 条对话 开始时间格式错误: {}", dialogue_count, e)))?;
            let end_ms = time_to_milliseconds(end_time_str)
                .map_err(|e| ConversionError::InvalidFormat(format!("第 {} 条对话 结束时间格式错误: {}", dialogue_count, e)))?;
            let duration_ms = end_ms.saturating_sub(start_ms);

            let property = if let Some(name_caps) = ASS_NAME_REGEX.captures(line) {
                let name = name_caps.get(1).unwrap().as_str();
                match name {
                    "左" => LYS_PROPERTY_LEFT,
                    "右" => LYS_PROPERTY_RIGHT,
                    "背" => {
                        if i > 0 {
                            if let Some(prev_name_caps) = ASS_NAME_REGEX.captures(&dialogues[i-1]) {
                                let prev_name = prev_name_caps.get(1).unwrap().as_str();
                                match prev_name {
                                    "左" => LYS_PROPERTY_BACK_LEFT,
                                    "右" => LYS_PROPERTY_BACK_RIGHT,
                                    "背" => last_property, 
                                    _ => LYS_PROPERTY_BACK_UNSET, 
                                }
                            } else {
                                LYS_PROPERTY_BACK_UNSET 
                            }
                        } else {
                            LYS_PROPERTY_BACK_UNSET 
                        }
                    },
                    _ => LYS_PROPERTY_UNSET, 
                }
            } else {
                LYS_PROPERTY_UNSET 
            };

            last_property = property;

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
                    "第 {} 行 k 值总和 {}{}{} ms 与持续时间 {}{}{} ms 不匹配",
                    dialogue_count,
                    RED, sum_k_ms, RESET,
                    GREEN, duration_ms, RESET
                );
                warning = true;
            }

            write!(writer, "[{}]", property)?;
            let mut current_ms = start_ms;
            for (seg_text, seg_ms) in segments {
                write!(writer, "{}({},{})", seg_text, current_ms, seg_ms)?;
                current_ms += seg_ms;
            }
            writeln!(writer)?;
        }
        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    }

    display_progress_bar(total_bytes, total_bytes);

    writer.flush()?;
    log_success!("{}", ASS_TO_LYS_COMPLETE);

    if warning {
        log_warn!("一行或多行文字总时间与行持续时间不匹配，建议修改原文件后再次转换");
        log_info!("按下任意键退出...");
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy)?;
    }

    Ok(())
}
fn check_ass_has_special_names(ass_path: &Path) -> Result<bool, ConversionError> {
    let file = File::open(ass_path)?;
    let reader = BufReader::new(file);
    let mut after_format = false;

    for line_result in reader.lines() {
        let line = line_result?;
        
        if !after_format {
            if line.trim_start().starts_with("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text") {
                after_format = true;
            }
            continue;
        }

        if line.starts_with("Dialogue:") {
            if let Some(name_caps) = ASS_NAME_REGEX.captures(&line) {
                let name = name_caps.get(1).unwrap().as_str();
                if name == "左" || name == "右" || name == "背" {
                    return Ok(true);
                }
            }
        }
    }
    
    Ok(false)
}

fn convert_lys_to_ass(lys_path: &Path, ass_path: &Path) -> Result<(), ConversionError> {
    let file = File::open(lys_path)?;
    let metadata = file.metadata()?;
    let total_bytes = metadata.len() as usize;
    let mut processed_bytes = 0;

    let reader = BufReader::new(file);
    let mut writer = BufWriter::new(File::create(ass_path)?);

    writeln!(writer, "[Script Info]")?;
    writeln!(writer, "PlayResX: 1920")?;
    writeln!(writer, "PlayResY: 1440")?;
    writeln!(writer)?;
    
    writeln!(writer, "[V4+ Styles]")?;
    writeln!(writer, "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding")?;
    writeln!(writer, "Style: Default,微软雅黑,100,&H00FFFFFF,&H004E503F,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,1")?;
    writeln!(writer)?;

    writeln!(writer, "[Events]")?;
    writeln!(writer, "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text")?;


    for line_result in reader.lines() {
        let line = line_result?;
        processed_bytes += line.len() + 1;

        if let Some(caps) = LYS_PROPERTY_REGEX.captures(&line) {
            let property = caps.get(1).unwrap().as_str().parse::<usize>().unwrap_or(0);
            let content = caps.get(2).unwrap().as_str();
            
            let name = match property {
                LYS_PROPERTY_LEFT => "左",
                LYS_PROPERTY_RIGHT => "右",
                LYS_PROPERTY_BACK_UNSET => "背",
                LYS_PROPERTY_BACK_LEFT => "背", 
                LYS_PROPERTY_BACK_RIGHT => "背", 
                _ => "",
            };
            
            let mut start_ms = 0;
            let mut end_ms = 0;
            let mut ass_text = String::new();
            let mut first_word = true;
            
            for word_cap in LYS_WORD_REGEX.captures_iter(content) {
                let word = word_cap.get(1).map(|m| m.as_str()).unwrap_or("");
                if word.is_empty() { continue; }
                
                if let (Some(ts_match), Some(dur_match)) = (word_cap.get(2), word_cap.get(3)) {
                    let word_start_ms: usize = ts_match.as_str().parse()?;
                    let word_duration_ms: usize = dur_match.as_str().parse()?;
                    
                    if first_word {
                        start_ms = word_start_ms;
                        first_word = false;
                    }
                    
                    end_ms = word_start_ms + word_duration_ms;
                    
                    let k_value = (word_duration_ms + K_TAG_MULTIPLIER / 2) / K_TAG_MULTIPLIER;
                    ass_text.push_str(&format!("{{\\kf{}}}{}", k_value, word));
                }
            }
            
            if !ass_text.is_empty() {
                let start_time = milliseconds_to_time(start_ms);
                let end_time = milliseconds_to_time(end_ms);
                
                writeln!(
                    writer,
                    "Dialogue: 0,{},{},Default,{},0,0,0,,{}", 
                    start_time,
                    end_time,
                    name,
                    ass_text
                )?;
            }
        }
        
        display_progress_bar(processed_bytes.min(total_bytes), total_bytes);
    }
    
    display_progress_bar(total_bytes, total_bytes);
    log_success!("{}", LYS_TO_ASS_COMPLETE);
    
    Ok(())
}

fn check_time_consistency(expected_duration: usize, actual_duration: usize, line_number: usize) -> bool {
    if expected_duration != actual_duration {
        log_warn!(
            "第 {} 行时间总和 {}{}{} ms 与持续时间 {}{}{} ms 不匹配",
            line_number,
            RED, actual_duration, RESET,
            GREEN, expected_duration, RESET
        );
        return false;
    }
    true
}