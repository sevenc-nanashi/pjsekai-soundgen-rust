use crate::level::Level;
use crate::sound::SOUND_MAP;
use crate::sound::{Effect, LOOP_SOUND_MAP};
use crate::utils::debug;
use crate::Sound;

use anyhow::{ensure, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::sync;
use std::{collections::HashMap, thread};

#[derive(Debug, Clone)]
pub struct ClipColor {
    pub fg: &'static str,
    pub bg: &'static str,
}

static COLOR_MAP: Lazy<HashMap<&'static str, ClipColor>> = Lazy::new(|| {
    HashMap::from([
        ("#PERFECT", ClipColor { fg: "cyan", bg: "blue" }),
        (
            "#PERFECT_ALTERNATIVE",
            ClipColor {
                fg: "red",
                bg: "yellow",
            },
        ),
        (
            "#HOLD",
            ClipColor {
                fg: "green",
                bg: "blue",
            },
        ),
        (
            "Sekai Tick",
            ClipColor {
                fg: "green",
                bg: "blue",
            },
        ),
        (
            "Sekai Critical Tap",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Hold",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Flick",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Tick",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai+ Normal Trace",
            ClipColor {
                fg: "black",
                bg: "white",
            },
        ),
        (
            "Sekai+ Critical Trace",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai+ Normal Trace Flick",
            ClipColor {
                fg: "red",
                bg: "yellow",
            },
        ),
        (
            "Sekai+ Critical Trace Flick",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
    ])
});
static NAME_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("#PERFECT", "通常タップ"),
        ("#PERFECT_ALTERNATIVE", "通常フリック"),
        ("#HOLD", "通常ホールド"),
        ("Sekai Tick", "スライド中継点"),
        ("Sekai Critical Tap", "金タップ"),
        ("Sekai Critical Hold", "金ホールド"),
        ("Sekai Critical Flick", "金フリック"),
        ("Sekai Critical Tick", "金スライド中継点"),
        ("Sekai+ Normal Trace", "トレース"),
        ("Sekai+ Critical Trace", "金トレース"),
        ("Sekai+ Normal Trace Flick", "トレースフリック"),
        ("Sekai+ Critical Trace Flick", "金トレースフリック"),
    ])
});
struct BpmChange {
    beat: f32,
    bpm: f32,
}

#[derive(Clone, Debug)]
pub struct Timing {
    single: HashMap<String, Vec<f32>>,
    connect: HashMap<String, Vec<(f32, f32)>>,
}

#[derive(Clone, Debug)]
pub struct ThreadInfo {
    pub color: ClipColor,
    pub max: i32,
}

#[derive(Clone, Debug)]
pub enum Progress {
    Info { threads: HashMap<String, ThreadInfo> },
    Update { id: String, current: i32 },
    Finish { id: String, sound: Sound },
}

pub async fn get_sound_timings(level: &Level, offset: f32) -> Result<Timing> {
    let mut timings: HashMap<String, Vec<f32>> = HashMap::new();
    let mut connect_timings: HashMap<String, Vec<(f32, f32)>> = HashMap::new();

    let mut bpm_changes: Vec<BpmChange> = vec![];
    for entity in level.data.entities.iter() {
        if entity.archetype == "#BPM_CHANGE" {
            bpm_changes.push(BpmChange {
                beat: entity
                    .get_value("#BEAT")
                    .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：#BPM_CHANGEに#BEATがありません"))?,
                bpm: entity
                    .get_value("#BPM")
                    .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：#BPM_CHANGEに#BPMがありません"))?,
            });
        }
    }
    bpm_changes.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
    let resolve_time = |beat: f32| -> f32 {
        let mut time = 0.0;
        let mut last_bpm = bpm_changes[0].bpm;
        let mut last_beat = 0.0;
        for bpm_change in bpm_changes.iter() {
            if bpm_change.beat > beat {
                break;
            }
            time += (bpm_change.beat - last_beat) * 60.0 / last_bpm;
            last_bpm = bpm_change.bpm;
            last_beat = bpm_change.beat;
        }
        time += (beat - last_beat) * 60.0 / last_bpm;
        time + level.data.bgm_offset + offset
    };
    for note in level.data.entities.iter() {
        let Some(sound_map_data) = SOUND_MAP.get(&note.archetype.as_str()) else {
            continue;
        };
        let sound_data = sound_map_data.to_string();
        if timings.get(&sound_data).is_none() {
            timings.insert(sound_data.clone(), vec![]);
        }
        let time = resolve_time(note.get_value("#BEAT").ok_or_else(|| {
            debug!(&note);
            anyhow::anyhow!("譜面データが壊れています：#BEATがありません")
        })?);
        timings.get_mut(&sound_data).unwrap().push(time);
    }
    let mut slide_connectors: HashMap<String, Vec<(f32, i32)>> = HashMap::new();
    for note in level.data.entities.iter() {
        let Some(key) = LOOP_SOUND_MAP.get(&note.archetype.as_str()) else {
            continue;
        };
        let key = key.to_string();
        let head = note
            .get_ref(&level.data.entities, "head")
            .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorにheadがありません"))?;
        let tail = note
            .get_ref(&level.data.entities, "tail")
            .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorにtailがありません"))?;
        let head_time = resolve_time(
            head.get_value("#BEAT")
                .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorのheadに#BEATがありません"))?,
        );
        let tail_time = resolve_time(
            tail.get_value("#BEAT")
                .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorのtailに#BEATがありません"))?,
        );
        if slide_connectors.get(&key).is_none() {
            slide_connectors.insert(key.clone(), vec![]);
        }
        slide_connectors.get_mut(&key).unwrap().push((head_time, 1));
        slide_connectors.get_mut(&key).unwrap().push((tail_time, -1));
    }
    for (key, changes) in slide_connectors.iter() {
        let mut slide_count = 0;
        let mut grouped_changes = changes
            .iter()
            .group_by(|(time, _)| *time)
            .into_iter()
            .map(|(time, changes)| (time, changes.map(|(_, change)| *change).collect::<Vec<_>>()))
            .collect::<Vec<_>>();

        grouped_changes.sort_by(|(time1, _), (time2, _)| time1.partial_cmp(time2).unwrap());

        for (time, changes) in &grouped_changes {
            if connect_timings.get(key).is_none() {
                connect_timings.insert(key.clone(), vec![]);
            }
            let time = *time;
            let change = changes.iter().sum::<i32>();
            if change == 0 {
                continue;
            }
            slide_count += change;
            let timing = connect_timings.get_mut(key).unwrap();
            if timing.is_empty() {
                timing.push((time, -1.0));
            } else if slide_count == 0 && change < 0 {
                timing.last_mut().unwrap().1 = time;
            } else if slide_count == 1 && change > 0 {
                timing.push((time, -1.0));
            }
            ensure!(slide_count >= 0, "譜面データが壊れています：スライドの開始と終了の数が一致しません");
        }
        ensure!(slide_count == 0, "譜面データが壊れています：スライドの開始と終了の数が一致しません");
        ensure!(
            connect_timings.get(key).unwrap().last().unwrap().1 != -1.0,
            "譜面データが壊れています：スライドの開始と終了の数が一致しません"
        );
    }
    timings.values_mut().for_each(|v| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup()
    });
    Ok(Timing {
        single: timings,
        connect: connect_timings,
    })
}

pub async fn synthesis(timing: &Timing, effect: &Effect, notes_per_thread: usize) -> sync::mpsc::Receiver<Progress> {
    let (tx, rx) = sync::mpsc::channel::<Progress>();
    let timing = timing.clone();
    let effect = effect.clone();

    thread::spawn(move || {
        let mut thread_infos: HashMap<String, ThreadInfo> = HashMap::new();
        let mut threads: Vec<thread::JoinHandle<()>> = vec![];
        for (sound_name, timings) in timing.single.iter() {
            let thread_count = (timings.len() + notes_per_thread - 1) / notes_per_thread;
            let notes_per_thread = (timings.len() + thread_count - 1) / thread_count;
            for i in 0..thread_count {
                let start = i * notes_per_thread;
                let end = if i == thread_count - 1 {
                    timings.len()
                } else {
                    std::cmp::min((i + 1) * notes_per_thread, timings.len())
                };
                let timings = timings[start..end].to_vec();
                let effect = effect.clone();
                let tx = tx.clone();
                debug!(&sound_name);
                let sound = effect
                    .audio
                    .get(sound_name)
                    .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                    .clone();
                let color = COLOR_MAP
                    .get(&sound_name.as_str())
                    .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                    .to_owned();
                let name = NAME_MAP
                    .get(&sound_name.as_str())
                    .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                    .to_owned();

                let id = format!("{} ({})", name, i + 1);

                thread_infos.insert(
                    id.clone(),
                    ThreadInfo {
                        color: color.clone(),
                        max: timings.len() as i32,
                    },
                );
                threads.push(thread::spawn(move || {
                    thread::park();
                    let mut local_sound = Sound::empty(None);
                    for (i, time) in timings.iter().enumerate() {
                        let next_time = timings.get(i + 1).unwrap_or(&(*time + 5.0)).to_owned();
                        local_sound = local_sound.overlay_until(&sound, *time, next_time);
                        tx.send(Progress::Update {
                            id: id.clone(),
                            current: i as i32 + 1,
                        })
                        .unwrap();
                    }
                    tx.send(Progress::Finish { id, sound: local_sound }).unwrap();
                }));
            }
        }
        for (sound_name, timings) in timing.connect.iter() {
            let timings = timings.clone();
            let effect = effect.clone();
            let tx = tx.clone();
            let sound = effect
                .audio
                .get(sound_name)
                .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                .clone();
            let color = COLOR_MAP
                .get(&sound_name.as_str())
                .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                .to_owned();
            let name = NAME_MAP
                .get(&sound_name.as_str())
                .unwrap_or_else(|| panic!("不明なSEです：{}。Issueに報告してください。", sound_name))
                .to_owned();

            let id = name.to_string();

            thread_infos.insert(
                id.clone(),
                ThreadInfo {
                    color: color.clone(),
                    max: timings.len() as i32,
                },
            );
            threads.push(thread::spawn(move || {
                thread::park();
                let mut local_sound = Sound::empty(None);
                for (i, (start, end)) in timings.iter().enumerate() {
                    local_sound = local_sound.overlay_loop(&sound, start.to_owned(), end.to_owned());
                    tx.send(Progress::Update {
                        id: id.clone(),
                        current: i as i32 + 1,
                    })
                    .unwrap();
                }
                tx.send(Progress::Finish { id, sound: local_sound }).unwrap();
            }));
        }
        tx.send(Progress::Info { threads: thread_infos }).unwrap();
        for thread in threads {
            thread.thread().unpark();
        }
    });

    rx
}
