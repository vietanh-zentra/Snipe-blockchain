#[macro_export]
macro_rules! log {
  ($($arg:tt)*) => {{
    let now = chrono::Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let millis = format!("{:03}", now.timestamp_subsec_millis());
    let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);

    let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
    let tab_prefix = std::iter::repeat("\t").take($crate::LOG_LEVEL as usize).collect::<String>();
    let msg = format!($($arg)*);
    let file_msg = format!("{} {} {}{}", timestamp, $crate::LOG_LEVEL_STR, tab_prefix, msg);

    println!("{}", file_msg);
    $crate::log_to_file(&file_msg);
  }}
}

#[macro_export]
macro_rules! info {
  ($($arg:tt)*) => {{
    let now = chrono::Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let millis = format!("{:03}", now.timestamp_subsec_millis());
    let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);

    let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
    let tab_prefix = std::iter::repeat("\t").take($crate::INFO_LEVEL as usize).collect::<String>();
    let msg = format!($($arg)*);
    let file_msg = format!("{} {} {}{}", timestamp, $crate::INFO_LEVEL_STR, tab_prefix, msg);

    println!("{}", file_msg);
    $crate::log_to_file(&file_msg);
  }}
}

#[macro_export]
macro_rules! success {
  ($($arg:tt)*) => {{
    let now = chrono::Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let millis = format!("{:03}", now.timestamp_subsec_millis());
    let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);

    let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
    let tab_prefix = std::iter::repeat("\t").take($crate::SUCCESS_LEVEL as usize).collect::<String>();
    let msg = format!($($arg)*);
    let file_msg = format!("{} {} {}{}", timestamp, SUCCESS_LEVEL_STR, tab_prefix, msg);

    println!("{}", file_msg);
    $crate::log_to_file(&file_msg);
  }}
}

#[macro_export]
macro_rules! warning {
  ($($arg:tt)*) => {{
    let now = chrono::Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let millis = format!("{:03}", now.timestamp_subsec_millis());
    let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);

    let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
    let tab_prefix = std::iter::repeat("\t").take($crate::WARNING_LEVEL as usize).collect::<String>();
    let msg = format!($($arg)*);
    let file_msg = format!("{} {} {}{}", timestamp, WARNING_LEVEL_STR, tab_prefix, msg);

    println!("{}", file_msg);
    $crate::log_to_file(&file_msg);
  }}
}

#[macro_export]
macro_rules! error {
  ($($arg:tt)*) => {{
    let now = chrono::Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let millis = format!("{:03}", now.timestamp_subsec_millis());
    let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);

    let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
    let tab_prefix = std::iter::repeat("\t").take($crate::ERROR_LEVEL as usize).collect::<String>();
    let msg = format!($($arg)*);
    let file_msg = format!("{} {} {}{}", timestamp, ERROR_LEVEL_STR, tab_prefix, msg);

    println!("{}", file_msg);
    $crate::log_to_file(&file_msg);
  }}
}
