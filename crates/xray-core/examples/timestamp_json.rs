//! Emits timestamp boundary cases for the JavaScript precision check.

use xray_core::UnixNanos;

fn main() {
    let timestamps = [0, 9_007_199_254_740_992, 9_007_199_254_740_993, i64::MAX]
        .map(|value| UnixNanos::try_from(value).unwrap());
    println!("{}", serde_json::to_string(&timestamps).unwrap());
}
