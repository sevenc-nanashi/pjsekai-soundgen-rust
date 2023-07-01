macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        format!("\x1b[38;2;{};{};{}m", $r, $g, $b)
    };
    ($hex:expr) => {
        format!("\x1b[38;2;{};{};{}m", $hex >> 16, $hex >> 8 & 0xff, $hex & 0xff)
    };
    () => {
        "\x1b[0m"
    };
}

#[cfg(debug_assertions)]
macro_rules! debug {
    ($($arg:tt)*) => {
        dbg!($($arg)*)
    };
}

#[cfg(not(debug_assertions))]
macro_rules! debug {
    ($($arg:tt)*) => {};
}

pub(crate) use debug;
pub(crate) use rgb;
