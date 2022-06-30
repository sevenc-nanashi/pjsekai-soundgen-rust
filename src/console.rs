use crate::utils::*;

pub fn show_title() {
    println!(
        "{}== pjsekai-soundgen-rust ------------------------------------------------------{}",
        rgb!(0x00b5c9),
        rgb!()
    );
    println!(
        "    {}pjsekai-soundgen-rust / Rust版プロセカ風譜面音声生成ツール{}",
        rgb!(0x00afc7),
        rgb!()
    );
    println!("    Version: {}{}{}", rgb!(0x0f6ea3), env!("CARGO_PKG_VERSION"), rgb!());
    println!(
        "    Developed by {}名無し｡(@sevenc-nanashi){}",
        rgb!(0x48b0d5),
        rgb!()
    );
    println!("    https://github.com/sevenc-nanashi/pjsekai-soundgen-rust");
    println!(
        "{}-------------------------------------------------------------------------------{}",
        rgb!(0xff5a91),
        rgb!()
    );
}
