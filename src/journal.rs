//! A run's journal (`passes.jsonl` in its run directory): each pass's reports appended as the
//! pass ends, so a run that dies part-way (killed, the machine off, the disk full, a renderer
//! taking a thread-isolated host down) loses only the pass it was in. With `output.resume` a
//! later run of the plan continues it: the passes it finished are kept, not run again; a pass
//! it died in runs again once, and a pass the run died in twice is recorded as failed with
//! that reason (it would only take the run down again). The reference passes run again (their
//! frames are kept in memory for the comparisons, not in the journal). When the run finishes,
//! its reports go to `report.json` and the journal is removed: an unfinished run is a run
//! directory with a journal and no report.

use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::report::PassReport;

pub const FILE: &str = "passes.jsonl";

/// A pass of the plan: its label, mode and repetition.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub label: String,
    pub mode: String,
    pub repeat: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Entry {
    /// A pass begun.
    Started(Key),
    /// A report of the pass under way (a pass may write several: its settled pass, the
    /// references it needed).
    Report(Box<PassReport>),
    /// The pass ended, its reports all written.
    Done(Key),
}

pub struct Journal {
    file: std::fs::File,
    done: HashSet<Key>,
    /// How often each unfinished pass was begun.
    died_in: HashMap<Key, u32>,
    /// The reports of the passes the earlier runs finished.
    pub kept: Vec<PassReport>,
}

/// How often a pass may take the run down before it is recorded as failed instead.
const TRIES: u32 = 2;

impl Journal {
    /// Opens `dir`'s journal, reading what an earlier run wrote (a last line cut short by the
    /// run's death is dropped; the pass it belonged to runs again).
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(FILE);
        let (mut done, mut died_in, mut reports): (
            HashSet<Key>,
            HashMap<Key, u32>,
            Vec<(Option<Key>, PassReport)>,
        ) = Default::default();
        let mut current = None;
        if let Ok(text) = std::fs::read_to_string(&path) {
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let entry = match serde_json::from_str::<Entry>(line) {
                    Ok(e) => e,
                    Err(_) if i + 1 == lines.len() => break,
                    Err(e) => {
                        return Err(e)
                            .with_context(|| format!("{}: line {}", path.display(), i + 1));
                    }
                };
                match entry {
                    Entry::Started(k) => {
                        *died_in.entry(k.clone()).or_default() += 1;
                        current = Some(k);
                    }
                    Entry::Report(r) => reports.push((current.clone(), *r)),
                    Entry::Done(k) => {
                        died_in.remove(&k);
                        done.insert(k);
                        current = None;
                    }
                }
            }
        }
        // Only the finished passes' reports stand, and of the references only those beside them
        // (the main references run again).
        let stand: Vec<(Key, PassReport)> = reports
            .into_iter()
            .filter_map(|(k, r)| k.filter(|k| done.contains(k)).map(|k| (k, r)))
            .collect();
        // Written afresh: what stands, then this run's passes.
        let mut file = std::fs::File::create(&path).with_context(|| path.display().to_string())?;
        let mut text = String::new();
        for k in &done {
            text += &serde_json::to_string(&Entry::Started(k.clone()))?;
            text.push('\n');
            for (_, r) in stand.iter().filter(|(rk, _)| rk == k) {
                text += &serde_json::to_string(&Entry::Report(Box::new(r.clone())))?;
                text.push('\n');
            }
            text += &serde_json::to_string(&Entry::Done(k.clone()))?;
            text.push('\n');
        }
        for (k, n) in &died_in {
            for _ in 0..*n {
                text += &serde_json::to_string(&Entry::Started(k.clone()))?;
                text.push('\n');
            }
        }
        file.write_all(text.as_bytes())?;
        file.sync_data()?;
        Ok(Self {
            file,
            done,
            died_in,
            kept: stand.into_iter().map(|(_, r)| r).collect(),
        })
    }

    /// Whether an earlier run finished this pass.
    pub fn done(&self, k: &Key) -> bool {
        self.done.contains(k)
    }

    /// Why this pass is not tried again: the run died in it each time it was begun.
    pub fn given_up(&self, k: &Key) -> Option<String> {
        let n = *self.died_in.get(k)?;
        (n >= TRIES).then(|| format!("the run died during this pass each of the {n} times it was begun (resumed with output.resume); not tried again"))
    }

    pub fn started(&mut self, k: &Key) -> anyhow::Result<()> {
        self.append(&Entry::Started(k.clone()))
    }

    /// A finished pass: its reports, then its end.
    pub fn finished(&mut self, k: &Key, reports: &[PassReport]) -> anyhow::Result<()> {
        for r in reports {
            self.append(&Entry::Report(Box::new(r.clone())))?;
        }
        self.append(&Entry::Done(k.clone()))
    }

    fn append(&mut self, e: &Entry) -> anyhow::Result<()> {
        let mut line = serde_json::to_string(e)?;
        line.push('\n');
        self.file.write_all(line.as_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }
}

/// The newest unfinished run of a plan named `name` (sanitized) under `base`: a run directory
/// with a journal and no report.
pub fn unfinished(base: &Path, name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.len() > 16 && &n[16..] == name)
        })
        .filter(|p| p.join(FILE).exists() && !p.join("report.json").exists())
        .collect();
    dirs.sort();
    dirs.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(l: &str) -> Key {
        Key {
            label: l.into(),
            mode: "unlit".into(),
            repeat: 0,
        }
    }

    fn report(l: &str) -> PassReport {
        PassReport {
            label: l.into(),
            mode: Some(mr_core::PassMode::UNLIT),
            fps: 60.0,
            ..Default::default()
        }
    }

    #[test]
    fn a_report_reads_back_as_written_an_identical_frame_included() {
        let q = crate::compare::Quality {
            psnr: f64::INFINITY,
            ssim: 1.0,
            ..Default::default()
        };
        let frame = crate::report::FrameRecord {
            frame: 3,
            latency_ms: 1.5,
            cpu_ms: 1.0,
            gpu_ms: None,
            update_ms: 0.0,
            arrived_bytes: 7,
            holes: 0,
            counters: vec![("chunks".into(), 4.0)],
            stages: vec![],
            quality: Some(q),
            quality_like: None,
        };
        let r = PassReport {
            per_frame: vec![frame],
            error: Some("x".into()),
            ..report("a")
        };
        let text = serde_json::to_string(&r).unwrap();
        let back: PassReport = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text);
        assert_eq!(
            back.per_frame[0].quality.unwrap().psnr,
            f64::INFINITY,
            "written as null, read as inf"
        );
    }

    #[test]
    fn a_resumed_journal_keeps_finished_passes_and_gives_up_after_two_deaths() {
        let dir = std::env::temp_dir().join(format!("mr-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        {
            let mut j = Journal::open(&dir).unwrap();
            j.started(&key("a")).unwrap();
            j.finished(&key("a"), &[report("a"), report("a (settled)")])
                .unwrap();
            j.started(&key("b")).unwrap();
            // Dies in b; a line cut short by the death.
            j.file.write_all(b"{\"report\":{\"lab").unwrap();
        }
        let j = Journal::open(&dir).unwrap();
        assert!(j.done(&key("a")) && !j.done(&key("b")));
        assert_eq!(
            j.kept.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            ["a", "a (settled)"]
        );
        assert_eq!(j.kept[0].fps, 60.0);
        assert!(j.given_up(&key("b")).is_none(), "one death: tried again");
        let mut j = j;
        j.started(&key("b")).unwrap();
        drop(j);
        let j = Journal::open(&dir).unwrap();
        assert!(j.given_up(&key("b")).unwrap().contains("2 times"));
        assert_eq!(j.kept.len(), 2, "a rewritten journal keeps what stands");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
