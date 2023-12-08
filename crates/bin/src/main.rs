mod console;
mod utils;

use crate::{console::show_title, utils::rgb};
use dialoguer::{theme::ColorfulTheme, Input};
use getopts::Options;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use octocrab::Octocrab;
use pjsekai_soundgen_core::{server::Server, sound::Sound, synthesis::Progress};
use std::{
    collections::HashMap,
    io::ErrorKind,
    thread, {env, fs},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

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
    let matches = match opts.parse(env::args().collect::<Vec<_>>()) {
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

async fn should_check_update() -> bool {
    let executable_path = process_path::get_executable_path().unwrap();
    let flag_path = executable_path.parent().unwrap().join(".update-check");
    if !flag_path.exists() {
        return true;
    }
    let flag = fs::read_to_string(flag_path).unwrap();
    let now = chrono::Local::now();
    let last_checked = chrono::DateTime::parse_from_rfc3339(flag.as_str()).unwrap();
    if now.signed_duration_since(last_checked).num_days() >= 1 {
        return true;
    }
    false
}

async fn check_update() {
    let executable_path = process_path::get_executable_path().unwrap();
    let flag_path = executable_path.parent().unwrap().join(".update-check");
    let mut file = File::create(flag_path).await.unwrap();
    let now = chrono::Local::now();
    file.write_all(now.to_rfc3339().as_bytes()).await.unwrap();
    let octocrab = Octocrab::builder().build().unwrap();
    let release = octocrab.repos("sevenc-nanashi", "pjsekai-soundgen-rust").releases().get_latest().await.unwrap();
    let version = release.tag_name.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");
    if version != current_version {
        console::info(&format!("新しいバージョンがリリースされています：v{} -> v{}", current_version, version));
        console::info(&format!("ダウンロード：{}", release.html_url));
    }
}

#[tokio::main]
async fn main() {
    let ansi = enable_ansi_support::enable_ansi_support().is_ok();
    console::ANSI.store(ansi, std::sync::atomic::Ordering::SeqCst);
    show_title();
    if should_check_update().await {
        check_update().await;
    }
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
    let timing = match pjsekai_soundgen_core::get_sound_timings(&level, args.shift).await {
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
    let rx = pjsekai_soundgen_core::synthesis(&timing, &effect, args.notes_per_thread).await;
    let Progress::Info { threads } = rx.recv().unwrap() else {
        unreachable!()
    };
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
