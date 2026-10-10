/// 版本字符串：0.1.0 (target, build-time, commit)
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("BUILD_TARGET"),
    ", ",
    env!("BUILD_TIME"),
    ", ",
    env!("BUILD_COMMIT"),
    ")"
);

pub const BUILD_COMMIT: &str = env!("BUILD_COMMIT");
