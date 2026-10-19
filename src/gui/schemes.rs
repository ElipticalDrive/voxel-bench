//! The encyclopedia (`docs/schemes/`, embedded at build time) in the GUI: a component's
//! entry beside its description (`hover`, `link`), and the Schemes tab to browse every entry
//! (`browser`), its related entries as links. An untested idea says so wherever it shows.

use std::sync::OnceLock;

use egui::{Color32, RichText, Ui};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/schemes.rs"));
}

pub struct Entry {
    /// Under `docs/schemes/`, e.g. `storage/bits.md`.
    pub path: &'static str,
    pub slug: &'static str,
    pub title: String,
    pub category: String,
    pub status: String,
    components: Vec<String>,
    related: Vec<String>,
    /// The text after the header (title line included).
    body: &'static str,
}

#[derive(serde::Deserialize)]
struct Header {
    title: String,
    category: String,
    status: String,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    related: Vec<String>,
}

pub fn all() -> &'static [Entry] {
    static ALL: OnceLock<Vec<Entry>> = OnceLock::new();
    ALL.get_or_init(|| {
        embedded::ENTRIES
            .iter()
            .filter_map(|(path, text)| {
                let (head, body) = text.strip_prefix("+++\n")?.split_once("\n+++\n")?;
                let h: Header = toml::from_str(head).ok()?;
                let slug = path.rsplit('/').next()?.trim_end_matches(".md");
                Some(Entry { path, slug, title: h.title, category: h.category, status: h.status, components: h.components, related: h.related, body })
            })
            .collect()
    })
}

/// The entry naming component `name` of `kind` (a graph role or a slot).
pub fn of(kind: &str, name: &str) -> Option<&'static Entry> {
    let kind = kind.strip_prefix("shadow_").unwrap_or(kind);
    let key = format!("{kind}/{name}");
    all().iter().find(|e| e.components.contains(&key))
}

fn by_slug(slug: &str) -> Option<&'static Entry> {
    all().iter().find(|e| e.slug == slug)
}

impl Entry {
    pub fn untested(&self) -> bool {
        self.status == "idea"
    }

    /// "measured", "built", or "untested idea", coloured.
    pub fn badge(&self) -> RichText {
        match self.status.as_str() {
            "idea" => RichText::new("untested idea").color(Color32::from_rgb(230, 180, 90)).italics(),
            "measured" => RichText::new("measured").color(Color32::from_rgb(120, 200, 120)),
            s => RichText::new(s).color(Color32::from_rgb(140, 170, 230)),
        }
    }

    /// The "What it is" section, its lines joined into paragraphs.
    pub fn what(&self) -> String {
        let s = self.body.split_once("## What it is").map_or("", |(_, r)| r);
        let s = s.split("\n## ").next().unwrap_or("");
        paragraphs(s).join("\n\n")
    }
}

/// Lines joined into paragraphs (list items stay separate).
fn paragraphs(s: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    for line in s.lines().map(str::trim_end) {
        let item = line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ");
        if line.trim().is_empty() || item {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if item {
                cur = format!("• {}", line.trim_start()[2..].trim());
            }
        } else if cur.is_empty() {
            cur = line.trim().to_string();
        } else {
            cur.push(' ');
            cur.push_str(line.trim());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A hover's entry part: the title, its status and "What it is".
pub fn hover(ui: &mut Ui, e: &Entry) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("📖 {}", e.title)).strong());
        ui.label(e.badge());
    });
    ui.label(e.what());
}

const REQUEST: &str = "schemes-open";

/// A link to the entry of `kind`/`name`, if there is one: hovering shows its "What it is",
/// clicking opens it in the Schemes tab.
pub fn link(ui: &mut Ui, kind: &str, name: &str) {
    let Some(e) = of(kind, name) else { return };
    ui.horizontal_wrapped(|ui| {
        if ui.link(format!("📖 {}", e.title)).on_hover_ui(|ui| hover(ui, e)).on_hover_text("Open the full entry in the Schemes tab").clicked() {
            open(ui.ctx(), e.path);
        }
        ui.label(e.badge());
    });
}

/// Asks the app to show entry `path` in the Schemes tab.
pub fn open(ctx: &egui::Context, path: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(REQUEST), path.to_string()));
    ctx.request_repaint();
}

/// The entry asked for since the last call.
pub fn take_request(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new(REQUEST)))
}

/// The Schemes tab's state.
#[derive(Default)]
pub struct Browser {
    pub open: Option<String>,
    filter: String,
}

pub fn browser(ui: &mut Ui, b: &mut Browser) {
    if let Some(e) = b.open.as_deref().and_then(|p| all().iter().find(|e| e.path == p)) {
        ui.horizontal(|ui| {
            if ui.button("⬅ all entries").clicked() {
                b.open = None;
            }
            ui.weak(format!("docs/schemes/{}", e.path));
        });
        ui.separator();
        entry(ui, e, b);
        return;
    }
    ui.label("The encyclopedia: every scheme built here (its measurements included) and every idea, the untested ones marked. Hover a component anywhere for its entry.");
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut b.filter).hint_text("filter by title or text").desired_width(220.0));
    });
    let f = b.filter.to_lowercase();
    let mut cats: Vec<&str> = all().iter().map(|e| e.category.as_str()).collect();
    cats.sort();
    cats.dedup();
    for c in cats {
        let es: Vec<&Entry> = all().iter().filter(|e| e.category == c && (f.is_empty() || e.title.to_lowercase().contains(&f) || e.body.to_lowercase().contains(&f))).collect();
        if es.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(format!("{c} ({})", es.len())).id_salt(("cat", c)).default_open(!f.is_empty()).show(ui, |ui| {
            for e in es {
                ui.horizontal_wrapped(|ui| {
                    let t = RichText::new(&e.title);
                    if ui.link(if e.untested() { t.italics() } else { t }).on_hover_ui(|ui| hover(ui, e)).clicked() {
                        b.open = Some(e.path.to_string());
                    }
                    if e.untested() {
                        ui.label(e.badge().small());
                    }
                });
            }
        });
    }
}

/// An entry's text: headings, paragraphs, list items, code blocks; then its related entries.
fn entry(ui: &mut Ui, e: &Entry, b: &mut Browser) {
    ui.horizontal_wrapped(|ui| {
        ui.label(e.badge());
        ui.weak(&e.category);
        if !e.components.is_empty() {
            ui.weak(format!("components: {}", e.components.join(", ")));
        }
    });
    let mut code = false;
    let mut text = String::new();
    let flush = |ui: &mut Ui, text: &mut String| {
        for p in paragraphs(text) {
            ui.label(p);
        }
        text.clear();
    };
    for line in e.body.lines() {
        if line.starts_with("```") {
            flush(ui, &mut text);
            code = !code;
            continue;
        }
        if code {
            ui.monospace(line);
        } else if let Some(h) = line.strip_prefix("# ") {
            flush(ui, &mut text);
            ui.heading(h);
        } else if let Some(h) = line.strip_prefix("## ") {
            flush(ui, &mut text);
            ui.add_space(4.0);
            ui.label(RichText::new(h).strong().size(15.0));
        } else {
            text.push_str(line);
            text.push('\n');
        }
    }
    flush(ui, &mut text);
    let related: Vec<&Entry> = e.related.iter().filter_map(|s| by_slug(s)).collect();
    if !related.is_empty() {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Related").strong());
            for r in related {
                if ui.link(&r.title).on_hover_ui(|ui| hover(ui, r)).clicked() {
                    b.open = Some(r.path.to_string());
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every embedded entry parses, has a "What it is", and its related slugs exist.
    #[test]
    fn entries_parse() {
        assert_eq!(all().len(), embedded::ENTRIES.len(), "an entry's header did not parse");
        assert!(all().len() > 50);
        for e in all() {
            assert!(!e.what().is_empty(), "{}: no \"What it is\"", e.path);
            for r in &e.related {
                assert!(by_slug(r).is_some(), "{}: related `{r}` is not an entry", e.path);
            }
        }
        assert_eq!(of("storage", "bits").map(|e| e.slug), Some("bits"));
        assert!(of("shadow_storage", "bits").is_some());
    }
}
