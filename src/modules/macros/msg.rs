#[macro_export]
macro_rules! update {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let millis = format!("{:03}", now.timestamp_subsec_millis());
        let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);
        let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
        let tab_prefix = std::iter::repeat("\t").take($crate::UPDATE_LEVEL as usize).collect::<String>();
        let msg = format!($($arg)*);
        let file_msg = format!("{} {} {}{}", timestamp, $crate::UPDATE_LEVEL_STR, tab_prefix, msg);
        println!("{}", file_msg);
        $crate::log_to_file(&file_msg);
    }};
}

#[macro_export]
macro_rules! result {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let millis = format!("{:03}", now.timestamp_subsec_millis());
        let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);
        let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
        let tab_prefix = std::iter::repeat("\t").take($crate::RESULT_LEVEL as usize).collect::<String>();
        let level_display = $crate::RESULT_LEVEL_RAW.cyan();
        let msg = format!($($arg)*);
        let _log_msg = format!("{} {} {}{}", timestamp.cyan(), level_display, tab_prefix, msg);
        let file_msg = format!("{} {} {}{}", timestamp, $crate::RESULT_LEVEL_RAW, tab_prefix, msg);
        println!("{}", file_msg);
        $crate::log_to_file(&file_msg);
    }};
}

#[macro_export]
macro_rules! alert {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let millis = format!("{:03}", now.timestamp_subsec_millis());
        let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);
        let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
        let tab_prefix = std::iter::repeat("\t").take($crate::ALERT_LEVEL as usize).collect::<String>();
        let level_display = $crate::ALERT_LEVEL_RAW;
        let msg = format!($($arg)*);
        let _log_msg = format!("{} {} {}{}", timestamp, level_display, tab_prefix, msg);
        let file_msg = format!("{} {} {}{}", timestamp, $crate::ALERT_LEVEL_STR, tab_prefix, msg);
        println!("{}", file_msg);
        $crate::log_to_file(&file_msg);
    }};
}

#[macro_export]
macro_rules! dev_log {
    ($($arg:tt)*) => {{
        if *$crate::DEV_MODE {
            let now = chrono::Local::now();
            let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
            let millis = format!("{:03}", now.timestamp_subsec_millis());
            let micros = format!("{:03}", now.timestamp_subsec_micros() % 1000);
            let timestamp = format!("{}.{} {}", formatted_time, millis, micros);
            let tab_prefix = std::iter::repeat("\t").take($crate::DEV_LEVEL as usize).collect::<String>();
            let msg = format!($($arg)*);
            let file_msg = format!("{} {} {}{}", timestamp, $crate::DEV_LEVEL_STR, tab_prefix, msg);
            println!("{}", file_msg);
            $crate::log_to_file(&file_msg);
        }
    }};
}
