mod console;
mod level;
mod server;
mod sonolus;
mod sound;
mod synthesis;
mod utils;

use dialoguer::{theme::ColorfulTheme, Input};
use getopts::Options;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::thread;
use std::{env, fs};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::console::show_title;
use crate::server::Server;
use crate::sonolus::*;
use crate::sound::Sound;
use crate::synthesis::Progress;
use crate::utils::rgb;

static LOG_STYLE: &str = "[{elapsed_precise} / {eta_precise}] [{bar:50.{color_fg}/{color_bg}}] {pos:>7}/{len:7} {msg}";

struct Args {
    bgm_override: Option<String>,
    bgm_volume: f32,
    shift: f32,
    silent: bool,
    output: Option<String>,
    id: Option<String>,
    notes_per_thread: usize,
}

fn parse_args() -> Args {
    let mut opts = Options::new();
    opts.optflag("h", "help", "ヘルプを表示して終了します。");
    opts.optopt("b", "bgm", "BGMを上書きします。", "PATH");
    opts.optopt("v", "bgm-volume", "BGMのボリュームを指定します。（1.0で等倍）", "VOLUME");
    opts.optopt("s", "shift", "SEをずらします。（秒単位）", "SECONDS");
    opts.optflag("S", "silent", "SEのみを生成します。");
    opts.optopt("n", "notes-per-thread", "スレッド毎のノーツ数を指定します。", "NUMBER");
    opts.optopt("o", "output", "出力先を指定します。", "OUTPUT");
    let matches = match opts.parse(&env::args().collect::<Vec<_>>()) {
        Ok(m) => m,
        Err(f) => {
            println!("{}", f);
            println!("{}", opts.usage(""));
            std::process::exit(1);
        }
    };
    if matches.opt_present("h") {
        let args: Vec<String> = env::args().collect();
        println!("{}", opts.usage(format!("{} [OPTIONS] [ID]", &args[0]).as_str()));
        std::process::exit(0);
    }
    Args {
        bgm_override: matches.opt_str("b"),
        bgm_volume: matches.opt_str("v").map(|s| s.parse::<f32>().unwrap()).unwrap_or(1.0),
        shift: matches.opt_str("s").map(|s| s.parse::<f32>().unwrap()).unwrap_or(0.0),
        silent: matches.opt_present("S"),
        output: matches.opt_str("o"),
        id: matches.free.get(1).map(|s| s.to_string()),
        notes_per_thread: matches.opt_str("n").map(|s| s.parse::<usize>().unwrap()).unwrap_or(1000),
    }
}

#[tokio::main]
async fn main() {
    let ansi = enable_ansi_support::enable_ansi_support().is_ok();
    console::ANSI.store(ansi, std::sync::atomic::Ordering::SeqCst);
    show_title();
    let args = parse_args();
    if args.output.is_none() {
        fs::create_dir("./dist").unwrap_or_else(|err| {
            if err.kind() != ErrorKind::AlreadyExists {
                console::error("distフォルダを作成できませんでした。");
                std::process::exit(1);
            }
        });
    }
    let name = if args.id.is_none() {
        console::ask("譜面IDをプレフィックス込みで入力してください。");

        Input::<String>::with_theme(&ColorfulTheme::default())
            .allow_empty(false)
            .with_prompt("")
            .interact()
            .unwrap()
            .trim_start_matches('#')
            .to_string()
    } else {
        args.id.unwrap().trim_start_matches('#').to_string()
    };
    let server = Server::guess(&name).unwrap_or_else(|e| {
        console::error(&e.to_string());
        std::process::exit(1);
    });

    console::info(&format!("{}{}{} から譜面を取得中...", rgb!(server.color), server.name, rgb!()));
    let level = server.fetch_level(&name).await.unwrap_or_else(|err| {
        console::error(&err.to_string());
        std::process::exit(1);
    });
    console::info(&format!(
        "{} / {} - {} (Lv. {}) が選択されました。",
        level.info.title, level.info.artists, level.info.author, level.info.rating
    ));

    console::info("BGMを読み込んでいます...");
    let mut bgm_buf: Vec<u8> = Vec::new();
    if args.bgm_override.is_some() {
        let mut file = File::open(args.bgm_override.unwrap()).await.expect("ファイルを開けませんでした。");
        file.read_to_end(&mut bgm_buf).await.unwrap();
    } else {
        level.fetch_bgm(&mut bgm_buf).await.unwrap_or_else(|err| {
            console::error(&err.to_string());
            std::process::exit(1);
        });
    }
    let bgm = Sound::load(&bgm_buf) * args.bgm_volume;

    console::info("譜面を読み込んでいます...");
    let timing = match synthesis::get_sound_timings(&level, args.shift).await {
        Ok(t) => t,
        Err(err) => {
            console::error(&err.to_string());
            std::process::exit(1);
        }
    };

    console::info("効果音を読み込んでいます...");
    let effect = server.fetch_effect(level.info.engine.effect).await.unwrap_or_else(|err| {
        console::error(&err.to_string());
        std::process::exit(1);
    });

    let progresses = MultiProgress::new();
    let mut progresses_map: HashMap<String, ProgressBar> = HashMap::new();
    let style = ProgressStyle::default_bar().progress_chars("- ");
    let rx = synthesis::synthesis(&timing, &effect, args.notes_per_thread).await;
    let Progress::Info { threads } = rx.recv().unwrap() else { unreachable!()};
    console::info(format!("{}スレッドで合成を開始します。", threads.len()).as_str());
    for (name, info) in threads.iter() {
        let progress =
            ProgressBar::new(info.max as u64)
                .with_style(style.clone().template(
                    LOG_STYLE.replace("{color_fg}", info.color.fg).replace("{color_bg}", info.color.bg).as_str(),
                ))
                .with_message(name.clone());
        progresses.add(progress.clone());
        progresses_map.insert(name.clone(), progress);
    }
    let draw_thread = thread::spawn(move || progresses.join().unwrap());
    let mut merged_sounds = Sound::empty(None);
    while !progresses_map.is_empty() {
        match rx.recv().unwrap() {
            Progress::Update { id, current } => {
                progresses_map.get(&id).unwrap().set_position(current as u64);
            }
            Progress::Finish { id, sound } => {
                progresses_map.get(&id).unwrap().finish();
                merged_sounds = merged_sounds.overlay_at(&sound, 0.0);
                progresses_map.remove(&id);
            }
            _ => unreachable!(),
        }
    }
    draw_thread.join().unwrap();
    console::info("合成が完了しました。");
    let mut final_bgm: Sound;
    if args.silent {
        final_bgm = Sound::empty(None);
    } else {
        final_bgm = bgm;
    }
    final_bgm = final_bgm.overlay_at(&merged_sounds, 0.0);
    let output = args.output.unwrap_or(format!("dist/{}.mp3", name));
    console::info("出力しています...");
    final_bgm.export(output.as_str());
    console::info(format!("完了しました：{}", output).as_str());
}
