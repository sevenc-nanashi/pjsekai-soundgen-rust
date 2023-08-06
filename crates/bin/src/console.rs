#![allow(dead_code)]

use crate::utils::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::atomic::AtomicBool;

pub static ANSI: AtomicBool = AtomicBool::new(false);
pub static ANSI_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());

pub fn show_title() {
    let messages = vec![
        format!(
            "{}== pjsekai-soundgen-rust ------------------------------------------------------{}",
            rgb!(0x00b5c9),
            rgb!()
        ),
        format!("    {}pjsekai-soundgen-rust / Rust版プロセカ風譜面音声生成ツール{}", rgb!(0x00afc7), rgb!()),
        format!("    Version: {}{}{}", rgb!(0x0f6ea3), env!("CARGO_PKG_VERSION"), rgb!()),
        format!("    Developed by {}名無し｡(@sevenc-nanashi){}", rgb!(0x48b0d5), rgb!()),
        format!("    https://github.com/sevenc-nanashi/pjsekai-soundgen-rust"),
        format!(
            "{}-------------------------------------------------------------------------------{}",
            rgb!(0xff5a91),
            rgb!()
        ),
    ];
    let joined_message = messages.join("\n");
    if ANSI.load(std::sync::atomic::Ordering::Relaxed) {
        println!("{}", joined_message);
    } else {
        println!("{}", ANSI_REGEX.replace_all(joined_message.as_str(), ""));
    }
}

#[inline]
pub fn colored_log(prefix: &str, msg: &str, escape_code: &str) {
    let message = format!("{}{}) {}{}", escape_code, prefix, rgb!(), msg);
    if ANSI.load(std::sync::atomic::Ordering::Relaxed) {
        println!("{}", message);
    } else {
        println!("{}", console::strip_ansi_codes(message.as_str()));
    }
}

pub fn error(msg: &str) {
    colored_log("X", msg, "\x1b[31m");
}

pub fn warning(msg: &str) {
    colored_log("!", msg, "\x1b[33m");
}

pub fn info(msg: &str) {
    colored_log("i", msg, "\x1b[36m");
}

pub fn ask(msg: &str) {
    colored_log("?", msg, "\x1b[32m");
}
