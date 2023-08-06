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
