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

pub(crate) use rgb;
