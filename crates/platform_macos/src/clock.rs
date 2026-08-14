//! Local wall-clock formatting for UI timestamps.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use objc2::rc::Retained;
use objc2_foundation::{NSDate, NSDateFormatter, NSString};

thread_local! {
    static CLOCK_FORMATTER: RefCell<Retained<NSDateFormatter>> = RefCell::new({
        let formatter = NSDateFormatter::new();
        formatter.setDateFormat(Some(&NSString::from_str("HH:mm")));
        formatter
    });
}

/// Local `HH:MM` for `at`, using the system time zone.
#[must_use]
pub fn format_local_hour_minute(at: SystemTime) -> String {
    let Ok(elapsed) = at.duration_since(UNIX_EPOCH) else {
        return String::new();
    };
    let date = NSDate::dateWithTimeIntervalSince1970(elapsed.as_secs_f64());
    CLOCK_FORMATTER.with(|formatter| formatter.borrow().stringFromDate(&date).to_string())
}

#[cfg(test)]
mod tests {
    use super::format_local_hour_minute;
    use std::time::SystemTime;

    #[test]
    fn local_hour_minute_is_zero_padded_hh_mm() {
        let stamp = format_local_hour_minute(SystemTime::now());
        let bytes = stamp.as_bytes();
        assert_eq!(bytes.len(), 5, "{stamp}");
        assert_eq!(bytes[2], b':');
        let hour: u8 = stamp[..2].parse().expect("hour");
        let minute: u8 = stamp[3..].parse().expect("minute");
        assert!(hour <= 23, "{stamp}");
        assert!(minute <= 59, "{stamp}");
    }
}
