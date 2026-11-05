//! 日期与时间的纯计算。
//!
//! 只做换算，不读取系统时间：调用方传入时间点，便于测试与替换。

use std::time::{SystemTime, UNIX_EPOCH};

/// 东八区相对 UTC 的偏移秒数。
const BEIJING_OFFSET_SECONDS: u64 = 8 * 60 * 60;

/// 一天的秒数。
const SECONDS_PER_DAY: u64 = 86_400;

/// 把 1970-01-01 起算的天数换算为公历年月日。
///
/// 使用 Howard Hinnant 的 civil_from_days 算法，避免引入日期库。
pub(crate) fn civil_date_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

/// 返回东八区的日期字符串与小时。
pub(crate) fn beijing_date_and_hour(now: SystemTime) -> (String, u32) {
    let seconds =
        now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() + BEIJING_OFFSET_SECONDS;
    let days = (seconds / SECONDS_PER_DAY) as i64;
    let hour = ((seconds % SECONDS_PER_DAY) / 3_600) as u32;
    let (year, month, day) = civil_date_from_days(days);
    (format!("{year:04}-{month:02}-{day:02}"), hour)
}
