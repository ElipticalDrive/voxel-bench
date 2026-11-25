//! A run's journal (`passes.jsonl` in its run directory): each pass's reports appended as the
//! pass ends, so a run that dies part-way (killed, the machine off, the disk full, a renderer
//! taking a thread-isolated host down) loses only the pass it was in. With `output.resume` a
//! later run of the plan continues it: the passes it finished are kept, not run again; a pass
//! it died in runs again once, and a pass the run died in twice is recorded as failed with
//! that reason (it would only take the run down again). The reference passes run again (their
//! frames are kept in memory for the comparisons; their reports are journaled beside the
//! passes, and dropped on resuming).
//!
//! The journal is also where a run's full reports live while it runs: the run keeps only
//! [`PassReport::slim`] copies in memory, so its memory does not grow with its passes' frames.
//! When the run finishes, [`each`] streams the journal into `report.json` and the observations,
//! one report at a time, and the journal is removed: an unfinished run is a run directory with
//! a journal and no report.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead as _, Write as _};
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
    /// A report: of the pass under way (a pass may write several: its settled pass, the
    /// references it needed), or between passes one of the main references.
    Report(Box<PassReport>),
    /// The pass ended, its reports all written.
    Done(Key),
    /// A run opened the journal: what comes before the last of these an earlier run wrote.
    Opened,
}

/// An entry read without its report (the first reading of a journal looks only at its shape).
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Shape {
    Started(Key),
    Report(serde::de::IgnoredAny),
    Done(Key),
    Opened,
}

pub struct Journal {
    file: std::fs::File,
    path: PathBuf,
    done: HashSet<Key>,
    /// How often each unfinished pass was begun.
    died_in: HashMap<Key, u32>,
    /// The reports of the passes the earlier runs finished, slim.
    pub kept: Vec<PassReport>,
}

/// How often a pass may take the run down before it is recorded as failed instead.
const TRIES: u32 = 2;

/// Each line of a journal with its number; a last line cut short (by the run's death) is left out.
fn lines<T: serde::de::DeserializeOwned>(path: &Path, mut f: impl FnMut(usize, T) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    let mut it = std::io::BufReader::new(file).lines().enumerate().peekable();
    while let Some((i, line)) = it.next() {
        let line = line.with_context(|| path.display().to_string())?;
        match serde_json::from_str::<T>(&line) {
            Ok(e) => f(i, e)?,
            Err(_) if it.peek().is_none() => break,
            Err(e) => return Err(e).with_context(|| format!("{}: line {}", path.display(), i + 1)),
        }
    }
    Ok(())
}

impl Journal {
    /// Opens `dir`'s journal, reading what an earlier run wrote (a last line cut short by the
    /// run's death is dropped; the pass it belonged to runs again). Read twice, a line at a
    /// time: its shape first, then the reports that stand, so opening holds no more than one.
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(FILE);
        let (mut done, mut died_in): (HashSet<Key>, HashMap<Key, u32>) = Default::default();
        // The report lines of the attempt under way, and those of finished attempts: only
        // these stand (of a pass the run died in, partial reports are dropped; so are the main
        // references, which run again).
        let (mut pending, mut stand): (Vec<usize>, HashSet<usize>) = Default::default();
        lines::<Shape>(&path, |i, e| {
            match e {
                Shape::Started(k) => {
                    *died_in.entry(k).or_default() += 1;
                    pending.clear();
                }
                Shape::Report(_) => pending.push(i),
                Shape::Done(k) => {
                    died_in.remove(&k);
                    done.insert(k);
                    stand.extend(pending.drain(..));
                }
                Shape::Opened => pending.clear(),
            }
            Ok(())
        })?;
        // Written afresh beside it, then put in its place: what stands, in the order it was
        // written, then the passes begun and not finished, then this run's mark.
        let new = dir.join(format!("{FILE}.new"));
        let opened = Self::rewrite(&path, &new, &done, &died_in, &stand);
        if opened.is_err() {
            // A part written (a full disk) goes; the journal stands as it was.
            let _ = std::fs::remove_file(&new);
        }
        let kept = opened.with_context(|| format!("journal {}: rewriting it to resume failed", path.display()))?;
        let file = std::fs::OpenOptions::new().append(true).open(&path).with_context(|| path.display().to_string())?;
        Ok(Self { file, path, done, died_in, kept })
    }

    /// Writes what stands of the journal at `path` to `new`, then puts it in `path`'s place;
    /// returns the reports that stand, slim.
    fn rewrite(path: &Path, new: &Path, done: &HashSet<Key>, died_in: &HashMap<Key, u32>, stand: &HashSet<usize>) -> anyhow::Result<Vec<PassReport>> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(new).with_context(|| new.display().to_string())?);
        let mut kept = vec![];
        let put = |out: &mut std::io::BufWriter<std::fs::File>, e: &Entry| -> anyhow::Result<()> {
            serde_json::to_writer(&mut *out, e)?;
            out.write_all(b"\n")?;
            Ok(())
        };
        // A finished pass's first beginning stands (passes begun side by side end together,
        // their reports between their beginnings and their ends).
        let mut begun: HashSet<Key> = HashSet::new();
        lines::<Entry>(path, |i, e| {
            match e {
                e @ Entry::Report(_) if stand.contains(&i) => {
                    put(&mut out, &e)?;
                    if let Entry::Report(r) = e {
                        kept.push(r.slim());
                    }
                }
                Entry::Started(k) if done.contains(&k) && !begun.contains(&k) => {
                    put(&mut out, &Entry::Started(k.clone()))?;
                    begun.insert(k);
                }
                Entry::Done(k) => put(&mut out, &Entry::Done(k))?,
                _ => {}
            }
            Ok(())
        })?;
        for (k, n) in died_in {
            for _ in 0..*n {
                put(&mut out, &Entry::Started(k.clone()))?;
            }
        }
        put(&mut out, &Entry::Opened)?;
        let file = out.into_inner().map_err(|e| e.into_error()).with_context(|| new.display().to_string())?;
        file.sync_data().with_context(|| new.display().to_string())?;
        drop(file);
        std::fs::rename(new, path).with_context(|| format!("{} -> {}", new.display(), path.display()))?;
        Ok(kept)
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

    /// Reports outside any pass: the main references (run again on resuming).
    pub fn beside(&mut self, reports: &[PassReport]) -> anyhow::Result<()> {
        for r in reports {
            self.append(&Entry::Report(Box::new(r.clone())))?;
        }
        Ok(())
    }

    fn append(&mut self, e: &Entry) -> anyhow::Result<()> {
        let mut line = serde_json::to_string(e)?;
        line.push('\n');
        let w = self.file.write_all(line.as_bytes()).and_then(|_| self.file.sync_data());
        w.with_context(|| format!("journal {}: writing a pass's record failed (the run stops here; with output.resume a later run continues it)", self.path.display()))
    }
}

/// Every report of `dir`'s journal, in the order the run had them, one at a time: those of the
/// passes earlier runs finished, then this run's (the last run to open it); a report both
/// recorded (a reference beside a pass, run again), once: this run's.
pub fn each(dir: &Path, mut f: impl FnMut(PassReport) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let path = dir.join(FILE);
    #[derive(Deserialize)]
    struct Id {
        label: String,
        mode: Option<mr_core::PassMode>,
        #[serde(default)]
        repeat: u32,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    enum Ids {
        Started(serde::de::IgnoredAny),
        Report(Id),
        Done(serde::de::IgnoredAny),
        Opened,
    }
    // Where this run's part begins (after the last mark), and which reports it has.
    let (mut last_opened, mut ids) = (0, vec![]);
    lines::<Ids>(&path, |i, e| {
        match e {
            Ids::Opened => last_opened = i,
            Ids::Report(id) => ids.push((i, (id.label, id.mode, id.repeat))),
            Ids::Started(_) | Ids::Done(_) => {}
        }
        Ok(())
    })?;
    let fresh: HashSet<_> = ids.into_iter().filter(|(i, _)| *i > last_opened).map(|(_, id)| id).collect();
    lines::<Entry>(&path, |i, e| {
        if let Entry::Report(r) = e
            && (i > last_opened || !fresh.contains(&(r.label.clone(), r.mode, r.repeat)))
        {
            f(*r)?;
        }
        Ok(())
    })
}

/// The newest unfinished run of a plan named `name` (sanitized) under `base`: a run directory
/// with a journal and no report.
pub fn unfinished(base: &Path, name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.len() > 16 && &n[16..] == name))
        .filter(|p| p.join(FILE).exists() && !p.join("report.json").exists())
        .collect();
    dirs.sort();
    dirs.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(l: &str) -> Key {
        Key { label: l.into(), mode: "unlit".into(), repeat: 0 }
    }

    fn report(l: &str) -> PassReport {
        PassReport { label: l.into(), mode: Some(mr_core::PassMode::UNLIT), fps: 60.0, ..Default::default() }
    }

    #[test]
    fn a_report_reads_back_as_written_an_identical_frame_included() {
        let q = crate::compare::Quality { psnr: f64::INFINITY, ssim: 1.0, ..Default::default() };
        let frame = crate::report::FrameRecord { frame: 3, latency_ms: 1.5, cpu_ms: 1.0, gpu_ms: None, update_ms: 0.0, arrived_bytes: 7, holes: 0, counters: vec![("chunks".into(), 4.0)], stages: vec![], quality: Some(q), quality_like: None };
        let r = PassReport { per_frame: vec![frame], error: Some("x".into()), ..report("a") };
        let text = serde_json::to_string(&r).unwrap();
        let back: PassReport = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text);
        assert_eq!(back.per_frame[0].quality.unwrap().psnr, f64::INFINITY, "written as null, read as inf");
    }

    #[test]
    fn a_resumed_journal_keeps_finished_passes_and_gives_up_after_two_deaths() {
        let dir = std::env::temp_dir().join(format!("mr-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        {
            let mut j = Journal::open(&dir).unwrap();
            j.started(&key("a")).unwrap();
            j.finished(&key("a"), &[report("a"), report("a (settled)")]).unwrap();
            j.started(&key("b")).unwrap();
            // Dies in b; a line cut short by the death.
            j.file.write_all(b"{\"report\":{\"lab").unwrap();
        }
        let j = Journal::open(&dir).unwrap();
        assert!(j.done(&key("a")) && !j.done(&key("b")));
        assert_eq!(j.kept.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["a", "a (settled)"]);
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

    fn framed(l: &str, frames: u32) -> PassReport {
        let f = |frame| crate::report::FrameRecord { frame, latency_ms: 1.0, cpu_ms: 1.0, gpu_ms: None, update_ms: 0.0, arrived_bytes: 0, holes: 0, counters: vec![], stages: vec![], quality: None, quality_like: None };
        PassReport { per_frame: (0..frames).map(f).collect(), ..report(l) }
    }

    fn all(dir: &Path) -> Vec<(String, usize)> {
        let mut v = vec![];
        each(dir, |r| {
            v.push((r.label.clone(), r.per_frame.len()));
            Ok(())
        })
        .unwrap();
        v
    }

    fn labels(v: &[(&str, usize)]) -> Vec<(String, usize)> {
        v.iter().map(|(l, n)| (l.to_string(), *n)).collect()
    }

    #[test]
    fn the_records_stream_from_the_journal_in_order_kept_ones_slim_and_references_once() {
        let dir = std::env::temp_dir().join(format!("mr-journal-each-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        {
            let mut j = Journal::open(&dir).unwrap();
            j.beside(&[framed("reference", 4)]).unwrap();
            // Two passes side by side: begun together, their reports written once, ended together.
            j.started(&key("a")).unwrap();
            j.started(&key("b")).unwrap();
            j.finished(&key("a"), &[framed("a", 5), framed("b", 5), framed("like (ref)", 2)]).unwrap();
            j.finished(&key("b"), &[]).unwrap();
            j.started(&key("c")).unwrap();
            // Dies in c, a report of it written.
            j.append(&Entry::Report(Box::new(framed("c", 5)))).unwrap();
        }
        assert_eq!(all(&dir), labels(&[("reference", 4), ("a", 5), ("b", 5), ("like (ref)", 2), ("c", 5)]), "a run's own records, as written");
        for _ in 0..2 {
            // Resumed (twice: what stands survives a rewrite of a rewrite).
            let j = Journal::open(&dir).unwrap();
            assert!(j.done(&key("a")) && j.done(&key("b")) && !j.done(&key("c")));
            let kept: Vec<(&str, usize)> = j.kept.iter().map(|r| (r.label.as_str(), r.per_frame.len())).collect();
            assert_eq!(kept, [("a", 1), ("b", 1), ("like (ref)", 1)], "kept slim; the main reference and the dead pass's report dropped");
        }
        let mut j = Journal::open(&dir).unwrap();
        j.beside(&[framed("reference", 4)]).unwrap();
        j.started(&key("c")).unwrap();
        j.finished(&key("c"), &[framed("c", 6), framed("like (ref)", 3)]).unwrap();
        drop(j);
        assert_eq!(
            all(&dir),
            labels(&[("a", 5), ("b", 5), ("reference", 4), ("c", 6), ("like (ref)", 3)]),
            "earlier runs' passes in full, then this run's; the reference both recorded once, this run's"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
