pub mod console;
pub mod sonolus;
pub mod sound;
pub mod utils;

extern crate getopts;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::thread;
use std::{collections::HashMap, sync::mpsc};
use std::{env, fs};

use console::show_title;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use getopts::Options;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sonolus::*;
use sound::Sound;

use crate::sound::{NOTE_NAME_MAP, SOUND_MAP};

static LOG_STYLE: &str = "[{elapsed_precise} / {eta_precise}] [{bar:50.{color_fg}/{color_bg}}] {pos:>7}/{len:7} {msg}";

struct Args {
    bgm_override: Option<String>,
    bgm_volume: f32,
    shift: f32,
    silent: bool,
    output: Option<String>,
    id: Option<String>,
    notes_per_thread: Option<usize>,
}

fn parse_args() -> Args {
    let mut opts = Options::new();
    opts.optflag("h", "help", "ヘルプを表示して終了します。");
    opts.optopt("b", "bgm", "BGMを上書きします。", "PATH");
    opts.optopt("v", "bgm-volume", "BGMのボリュームを指定します。（1.0で等倍）", "VOLUME");
    opts.optopt("s", "shift", "SEをずらします。（秒単位）", "SECONDS");
    opts.optflag("S", "silent", "SEのみを生成します。");
    opts.optopt("n", "notes-per-thread", "スレッド毎のノーツ数を指定します。（β）", "NUMBER");
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
        notes_per_thread: matches.opt_str("n").map(|s| s.parse::<usize>().unwrap()),
    }
}

fn select_level_from_query(query: &String) -> String {
    let client = reqwest::blocking::Client::new();
    let levels = client
        .get("https://servers-legacy.purplepalette.net/levels/list")
        .query(&[("keywords", query)])
        .send()
        .unwrap()
        .json::<LevelListResponse>()
        .unwrap();
    if levels.items.len() == 0 {
        println!("{}", "該当するレベルが見つかりませんでした。");
        std::process::exit(1);
    }
    let level_names = levels.items.iter().map(|level| format!("{}", level)).collect::<Vec<String>>();
    let selected = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("譜面を選択してください。")
        .items(&level_names)
        .default(0)
        .interact()
        .unwrap();
    levels.items[selected as usize].name.clone()
}

fn load_bgm(id: &String, buf: &mut Vec<u8>) {
    let client = reqwest::blocking::Client::new();
    let level = client
        .get(format!("https://servers-legacy.purplepalette.net/levels/{}", id).as_str())
        .send()
        .unwrap()
        .json::<SingleLevelResponse>()
        .unwrap();
    let mut bgm_response = client
        .get(format!("https://servers-legacy.purplepalette.net{}", level.item.bgm.url))
        .send()
        .unwrap();
    bgm_response.copy_to(buf).unwrap();
}

fn main() {
    enable_ansi_support::enable_ansi_support().unwrap_or_else(|_| {
        println!("ANSIがサポートされていない環境です。");
        std::process::exit(1);
    });
    show_title();
    let args = parse_args();
    if args.output == None {
        fs::create_dir("./dist").unwrap_or_else(|err| {
            if err.kind() != ErrorKind::AlreadyExists {
                panic!("distフォルダを作成できませんでした。");
            }
        });
    }
    let id: String;
    if args.id == None {
        println!("曲名、またはIDを入力してください。\nIDを入力する場合は、先頭に「#」を付けてください。");
        let mut id_or_query: String =
            Input::with_theme(&ColorfulTheme::default()).allow_empty(false).with_prompt("").interact().unwrap();
        id_or_query = id_or_query.trim().to_string();
        if id_or_query.starts_with('#') {
            id = id_or_query.trim_start_matches('#').to_string();
        } else {
            id = select_level_from_query(&id_or_query);
        }
    } else {
        id = args.id.unwrap().trim_start_matches("#").to_string();
    }
    let level = Level::fetch(&id).unwrap_or_else(|err| match err {
        LevelError::NotFound => {
            println!("譜面が見つかりませんでした。");
            std::process::exit(1);
        }
        LevelError::InvalidFormat => {
            println!("サーバーが不正なデータを返しています。");
            std::process::exit(1);
        }
    });
    println!("{} を選択しました。", level);
    println!("BGMを読み込んでいます...");
    let mut bgm_buf: Vec<u8> = Vec::new();
    if args.bgm_override != None {
        let mut file = File::open(args.bgm_override.unwrap()).expect("ファイルを開けませんでした。");
        file.read_to_end(&mut bgm_buf).unwrap();
    } else {
        load_bgm(&id, &mut bgm_buf);
    }
    let bgm_raw = Sound::load(&bgm_buf);
    let bgm = bgm_raw * args.bgm_volume;
    println!("ノーツを読み込んでいます...");
    let (tap_sound_timings, connect_note_timings) = level.get_sound_timings(args.shift);
    println!("ノーツのSEを読み込んでいます...");
    let mut threads = vec![];

    let note_sound_data = SOUND_MAP
        .iter()
        .map(|(_key, value)| {
            let raw = Sound {
                data: value.0.to_vec().chunks_exact(2).into_iter().map(|a| i16::from_le_bytes([a[0], a[1]])).collect(),
                bitrate: 48000,
            };
            (value.1, raw)
        })
        .collect::<HashMap<_, _>>();
    let progresses = MultiProgress::new();
    let style = ProgressStyle::default_bar().progress_chars("-♪ ");
    let (tx, rx) = mpsc::channel();
    for (note, times) in tap_sound_timings.clone() {
        let sound = note_sound_data.get(note.as_str()).unwrap().clone();
        let style = style.clone();
        let is_critical = note.starts_with("critical_");
        let (color_fg, color_bg) = if is_critical {
            ("yellow", "orange")
        } else if note == "flick" {
            ("red", "yellow")
        } else {
            ("cyan", "blue")
        };
        let mut progress = ProgressBar::new(times.len() as u64)
            .with_style(
                style.template(LOG_STYLE.replace("{color_fg}", color_fg).replace("{color_bg}", color_bg).as_str()),
            )
            .with_message(NOTE_NAME_MAP.get(note.strip_prefix("critical_").unwrap_or(&note)).unwrap().clone());
        let notes_per_thread = args.notes_per_thread.unwrap_or(times.len());
        let thread_num: usize = if args.notes_per_thread.is_some() {
            (times.len() as f32 / (notes_per_thread as f32)).ceil() as usize
        } else {
            1
        };

        if args.notes_per_thread.is_some() {
            progress = progress.with_message(format!(
                "{} ({})",
                NOTE_NAME_MAP.get(note.strip_prefix("critical_").unwrap_or(&note)).unwrap().clone(),
                thread_num
            ));
        }

        progresses.add(progress.clone());
        for i in 0..thread_num as usize {
            let lprogress = progress.clone();
            let lsound = sound.clone();
            let ltx = tx.clone();
            let ltimes = times[(i * notes_per_thread)..=((i + 1) * notes_per_thread).min(times.len() - 1)].to_vec();
            threads.push(std::thread::spawn(move || {
                let mut local_sound = Sound::empty(None);
                for (i, time) in ltimes.iter().enumerate() {
                    if i == notes_per_thread {
                        continue;
                    }
                    lprogress.inc(1);
                    let next_time = ltimes.get(i + 1).unwrap_or(&(*time + 5.0)) + args.shift;
                    local_sound = local_sound.overlay_until(&lsound, time.clone(), next_time);
                }
                lprogress.finish(); // FIXME: 別スレッドの処理が終わったことを確認してからfinishする
                ltx.send(local_sound).unwrap();
            }));
        }
    }
    for (note, times) in connect_note_timings.clone() {
        let mut events = vec![];
        for (start, end) in times.clone() {
            events.push((1, start));
            events.push((-1, end));
        }
        events.sort_by(|a, b| {
            if a.1 == b.1 {
                b.0.partial_cmp(&a.0).unwrap()
            } else {
                a.1.partial_cmp(&b.1).unwrap()
            }
        });
        let sound = note_sound_data.get(note.as_str()).unwrap().clone();
        let style = style.clone();
        let (color_fg, color_bg) = if note.starts_with("critical_") {
            ("yellow", "orange")
        } else {
            ("green", "blue")
        };
        let progress = ProgressBar::new((times.len() as u64) * 2)
            .with_style(
                style.template(LOG_STYLE.replace("{color_fg}", color_fg).replace("{color_bg}", color_bg).as_str()),
            )
            .with_message(NOTE_NAME_MAP.get(note.strip_prefix("critical_").unwrap_or(&note)).unwrap().clone());
        progresses.add(progress.clone());
        let ltx = tx.clone();
        threads.push(std::thread::spawn(move || {
            let mut local_sound = Sound::empty(None);
            let lsound = sound.clone();
            drop(sound);
            let mut current = 0;
            let mut start_time = 0.0;
            for (sign, time) in events.clone() {
                current += sign;
                progress.inc(1);
                if sign == -1 && current == 0 {
                    local_sound = local_sound.overlay_loop(&lsound, start_time, time);
                } else if sign == 1 && current == 1 {
                    start_time = time;
                }
            }
            assert_eq!(current, 0);
            progress.finish();
            ltx.send(local_sound).unwrap();
        }));
    }
    println!("{}スレッドで処理を開始します。", threads.len());
    let draw_thread = thread::spawn(move || progresses.join().unwrap());
    let mut merged_sounds = Sound::empty(None);
    for _ in 0..threads.len() {
        let received = rx.recv().unwrap();
        merged_sounds = merged_sounds.overlay_at(&received, 0.0);
        drop(received);
    }
    draw_thread.join().unwrap();
    println!("BGMとSEを合成中...");
    let mut final_bgm: Sound;
    if args.silent {
        final_bgm = Sound::empty(None);
    } else {
        final_bgm = bgm;
    }
    final_bgm = final_bgm.overlay_at(&merged_sounds, 0.0);
    let output = args.output.unwrap_or(format!("dist/{}.mp3", id));
    println!("出力中...");
    final_bgm.export(output.as_str());
    println!("{} に出力しました。", output);
}
