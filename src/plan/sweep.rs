//! Sweeps: a renderer that `vary`s its edits expanded into one renderer per pipeline.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, bail};

use super::{GraphRef, graph_file};

/// Which of a sweep's pipelines run: `cover = "all"` (the default) or `"pairs"`, on a swept
/// entry or for every one of the plan (`[sweep] cover`, [`Settings`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cover {
    /// Every pipeline the combinations give.
    All,
    /// A few of them, in the same order, that hold every value of every entry of `vary` and,
    /// of every two entries, each pair of their values that some pipeline holds together
    /// ([`cover`]).
    Pairs,
}

impl Cover {
    fn of(v: Option<&toml::Value>, default: Cover) -> anyhow::Result<Cover> {
        match v {
            None => Ok(default),
            Some(v) => match v.as_str() {
                Some("all") => Ok(Cover::All),
                Some("pairs") => Ok(Cover::Pairs),
                _ => bail!(
                    "`cover = {v}`: \"all\" (every pipeline of the sweep) or \"pairs\" (every value of each entry of `vary`, and of any two entries every pair of values, at least once)"
                ),
            },
        }
    }
}

/// The plan's `[sweep]` table: `cover`, the entries' default, and `part` of `parts` (the
/// pairs spread over that many runs, [`cover`]).
#[derive(Clone, Copy, Debug)]
struct Settings {
    cover: Cover,
    part: Option<(usize, usize)>,
}

impl Settings {
    fn of(v: Option<toml::Value>) -> anyhow::Result<Settings> {
        let Some(v) = v else {
            return Ok(Settings {
                cover: Cover::All,
                part: None,
            });
        };
        let t = v
            .as_table()
            .context("`sweep` is a table (`[sweep] cover = \"pairs\"`)")?;
        if let Some(k) = t
            .keys()
            .find(|k| !["cover", "part", "parts"].contains(&k.as_str()))
        {
            bail!("`sweep.{k}`: the sweep table has `cover`, `part` and `parts`");
        }
        let int = |k: &str| {
            t.get(k)
                .map(|v| {
                    v.as_integer()
                        .and_then(|i| usize::try_from(i).ok())
                        .with_context(|| format!("`sweep.{k} = {v}`: a count"))
                })
                .transpose()
        };
        let part = match (int("part")?, int("parts")?) {
            (None, None) => None,
            (Some(k), Some(n)) if k < n => Some((k, n)),
            _ => bail!(
                "`sweep.part` of `sweep.parts`: both, the part counted from 0 (`part = 2, parts = 15`)"
            ),
        };
        Ok(Settings {
            cover: Cover::of(t.get("cover"), Cover::All)?,
            part,
        })
    }
}

/// Expands every swept `[[renderer]]` (one with `vary`) into one renderer per pipeline it
/// stands for ([`expand_varied`]). Takes the plan's `[sweep]` table ([`Settings`]).
pub(super) fn expand_sweeps(table: &mut toml::Table) -> anyhow::Result<()> {
    let default = Settings::of(table.remove("sweep"))?;
    let Some(toml::Value::Array(entries)) = table.get_mut("renderer") else {
        return Ok(());
    };
    if let Some(e) = entries
        .iter()
        .find(|e| e.get("cover").is_some() && e.get("vary").is_none())
    {
        bail!("`cover` chooses among the pipelines of a sweep; this renderer has no `vary` ({e})");
    }
    if !entries.iter().any(|e| e.get("vary").is_some()) {
        return Ok(());
    }
    let mut out = vec![];
    for e in entries.drain(..) {
        match e.get("vary") {
            Some(_) => out.extend(expand_varied(
                e.as_table().context("a renderer is a table")?,
                default,
            )?),
            None => out.push(e),
        }
    }
    *entries = out;
    Ok(())
}

/// What one combination holds, for [`cover`]: value `i` of entry `a` of `vary` as
/// `(a, i, a, i)`, and values `i` and `j` of two entries `a < b` together as `(a, i, b, j)`.
pub(super) type Held = (usize, usize, usize, usize);

/// What combination `n` of entries of `lens` choices each holds (in `varied`'s order: the
/// first entry varying slowest).
pub(super) fn held(n: usize, lens: &[usize]) -> Vec<Held> {
    let mut digits = vec![0; lens.len()];
    let mut r = n;
    for a in (0..lens.len()).rev() {
        digits[a] = r % lens[a];
        r /= lens[a];
    }
    let mut out = vec![];
    for a in 0..lens.len() {
        out.push((a, digits[a], a, digits[a]));
        for b in a + 1..lens.len() {
            out.push((a, digits[a], b, digits[b]));
        }
    }
    out
}

/// Of `candidates`, each what it holds, a few that together hold everything any of them
/// holds, in their order: greedily the one holding most of what is still missing, the first
/// of equals (deterministic, no seed). Not always the fewest, but near it.
///
/// With `part = Some((k, n))`, part `k` of `n` (counted from 0): the pairs spread over `n`
/// runs (the greedy choice's `k`th, `k + n`th, ... pick), each with every value at least
/// once (completed greedily), so the `n` parts together hold every pair.
pub(super) fn cover(candidates: &[HashSet<Held>], part: Option<(usize, usize)>) -> Vec<usize> {
    let all: HashSet<Held> = candidates.iter().flatten().copied().collect();
    let mut chosen = greedy(candidates, all.clone());
    if let Some((k, n)) = part {
        chosen = chosen.into_iter().skip(k).step_by(n.max(1)).collect();
        let held: HashSet<Held> = chosen
            .iter()
            .flat_map(|&i| candidates[i].iter().copied())
            .collect();
        let values = all
            .into_iter()
            .filter(|h| h.0 == h.2 && !held.contains(h))
            .collect();
        chosen.extend(greedy(candidates, values));
    }
    chosen.sort_unstable();
    chosen.dedup();
    chosen
}

/// Candidates that together hold `missing`, in the order picked: each time the one holding
/// most of what is still missing, the first of equals.
fn greedy(candidates: &[HashSet<Held>], mut missing: HashSet<Held>) -> Vec<usize> {
    let mut chosen = vec![];
    while !missing.is_empty() {
        let (best, _) = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.iter().filter(|h| missing.contains(h)).count()))
            .fold((0, 0), |a, b| if b.1 > a.1 { b } else { a });
        for h in &candidates[best] {
            missing.remove(h);
        }
        chosen.push(best);
    }
    chosen
}

/// The renderers a swept entry stands for (`vary`, after `graph` and `edits`): one per
/// combination of its choices (`mr_components::graph::varied`) that can be made and that
/// `accept` takes, each distinct pipeline once, written as the entry with the combination's
/// edits after its own. A combination that is no pipeline does not exist; none at all is an
/// error, with the first reason. With [`Cover::Pairs`] only those [`cover`] keeps (a pipeline
/// several combinations give holds what each of them holds).
fn expand_varied(e: &toml::Table, default: Settings) -> anyhow::Result<Vec<toml::Value>> {
    use mr_components::graph::{CLIENT_KNOBS, Change, varied, varied_axes};
    let by = Cover::of(e.get("cover"), default.cover)?;
    let graph: GraphRef = e.get("graph").cloned().context("`vary` sweeps the edits of a `graph` (a file's name or the graph itself); this renderer has none")?.try_into()?;
    let edits: Vec<Change> = e
        .get("edits")
        .cloned()
        .map(|v| v.try_into())
        .transpose()?
        .unwrap_or_default();
    let base = match graph {
        GraphRef::File(f) => graph_file(&f)?,
        GraphRef::Inline(g) => *g,
    };
    let base = base
        .with_edits(&edits)
        .map_err(|e| anyhow::anyhow!("renderer graph: {e}"))?;
    let vary = e["vary"]
        .as_array()
        .context("`vary` is a list of edits whose values are lists")?;
    let given = e
        .get("options")
        .and_then(|o| o.as_table())
        .cloned()
        .unwrap_or_default();
    let lens: Vec<usize> = match by {
        Cover::All => vec![],
        Cover::Pairs => varied_axes(&base, vary)
            .map_err(|e| anyhow::anyhow!("vary: {e}"))?
            .iter()
            .map(Vec::len)
            .collect(),
    };
    let mut keys: HashMap<_, usize> = HashMap::new();
    let mut holds: Vec<HashSet<Held>> = vec![];
    let (mut out, mut first) = (vec![], None);
    for (n, combo) in varied(&base, vary)
        .map_err(|e| anyhow::anyhow!("vary: {e}"))?
        .into_iter()
        .enumerate()
    {
        let g = match base.with_edits(&combo).and_then(|g| g.accept().map(|_| g)) {
            Ok(g) => g,
            Err(why) => {
                first.get_or_insert(why);
                continue;
            }
        };
        // The options this pipeline's executor declares.
        let known = crate::pipelines::executor_of(&g)
            .map(|(_, p)| p)
            .unwrap_or_default();
        let options: toml::Table = given
            .iter()
            .filter(|(k, _)| known.iter().any(|p| p.name == **k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        // The client's region is the workload, not in the key, but a pass of its own.
        let region: Vec<Option<i64>> = CLIENT_KNOBS.iter().map(|k| g.client_knob(k)).collect();
        let key = (
            g.with_options(&options).key().map_err(anyhow::Error::msg)?,
            region,
        );
        let now = if by == Cover::Pairs {
            held(n, &lens)
        } else {
            vec![]
        };
        if let Some(&at) = keys.get(&key) {
            holds[at].extend(now);
            continue;
        }
        keys.insert(key, out.len());
        holds.push(now.into_iter().collect());
        let mut entry = e.clone();
        entry.remove("vary");
        entry.remove("label");
        entry.remove("cover");
        let all: Vec<Change> = edits.iter().chain(&combo).cloned().collect();
        entry.insert("edits".into(), toml::Value::try_from(&all)?);
        entry.insert("options".into(), toml::Value::Table(options));
        entry.insert("swept".into(), toml::Value::Boolean(true));
        out.push(toml::Value::Table(entry));
    }
    if out.is_empty() {
        bail!(
            "the sweep `vary = {}` gives no pipeline{}",
            e["vary"],
            first
                .map(|w| format!(" (the first combination: {w})"))
                .unwrap_or_default()
        );
    }
    if by == Cover::Pairs {
        let all = out.len();
        out = cover(&holds, default.part)
            .into_iter()
            .map(|i| out[i].clone())
            .collect();
        let graph = e["graph"].as_str().unwrap_or("an inline graph");
        let part = default
            .part
            .map(|(k, n)| format!(" (part {k} of {n}: every value, a share of the pairs)"))
            .unwrap_or_default();
        tracing::info!(
            "sweep of {graph}: {} of its {all} pipelines hold every value of `vary` and every pair of two entries' values{part}",
            out.len()
        );
    }
    Ok(out)
}
