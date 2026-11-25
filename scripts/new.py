#!/usr/bin/env python3
"""new.py: scaffold, remove and try components, so adding a technique is one command.

  scripts/new.py storage NAME ["description"]   a CPU storage: a working copy of `bricks` (u16 per
                                                voxel + brick mask) under its own name, registered
                                                everywhere a CPU storage is listed; edit its cpu.rs
  scripts/new.py remove storage NAME            undo exactly that (every hand-kept list line + the dir)
  scripts/new.py try NAME [--sweep]             what a new component must pass: cargo test (registry,
                                                storage exactness, walk agreement), clippy, then with
                                                --sweep a run of every graph that can take it
  scripts/new.py accept-sweep                   a new component changes the size of the full sweep, which two host
                                                tests state: reads the new size from the failing test and rewrites
                                                it (review the diff)
  scripts/new.py sites                          the hand-kept lists a storage joins (what `storage` edits)

Each edit anchors on the line after which a list ends and fails loudly if the anchor is not
exactly there, so a refactor that moves a list breaks this script, not the result. Prints the
files it touched. Nothing is committed."""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
C = ROOT / "crates" / "mr-components" / "src"
KEYWORDS = {"as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl",
            "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct",
            "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn"}


def die(msg):
    print(f"new.py: {msg}", file=sys.stderr)
    sys.exit(1)


def camel(name):
    return "".join(p.capitalize() for p in name.split("_"))


class Edit:
    """One line (or block) a storage owns in a hand-kept list."""

    def __init__(self, label, path, anchor, before=True, last=False):
        self.label, self.path, self.anchor, self.before, self.last = label, path, anchor, before, last

    def where(self, text):
        hits = [m for m in re.finditer(self.anchor, text, re.M)]
        if not hits:
            die(f"{self.label}: anchor /{self.anchor}/ not found in {self.path.relative_to(ROOT)}")
        if len(hits) > 1 and not self.last:
            die(f"{self.label}: anchor /{self.anchor}/ matches {len(hits)} times in {self.path.relative_to(ROOT)}")
        m = hits[-1]
        return m.start() if self.before else m.end()

    def insert(self, line):
        text = self.path.read_text()
        at = self.where(text)
        text = text[:at] + line + text[at:]
        self.path.write_text(text)


def sites(name, ty, desc):
    """(Edit, text) per list. `ty` is the CamelCase struct."""
    path = f"mr_components::storage::{name}::cpu::{ty}"
    s = f"crate::storage::{name}::cpu::{ty}"
    return [
        (Edit("module list", C / "storage/mod.rs", r"^use mr_core::DenseChunk;\n"), ""),
        (Edit("registry", C / "lib.rs", r"^    // Visibility\.\n"), ""),  # text: registry_block
        (Edit("discover world", C / "visibility/discover/grid.rs", r'^            other => anyhow::bail!\("storage `\{other\}` has no CPU implementation"\),\n'),
         f'            "{name}" => store::<s::{name}::cpu::{ty}>(),\n'),
        (Edit("cpu type names", ROOT / "crates/mr-exec-cpu/src/raycast.rs", r"^        _ => return None,\n"),
         f'        "{name}" => "{path}",\n'),
        (Edit("cpu dispatch", ROOT / "crates/mr-exec-cpu/src/raycast.rs", r'^            s => anyhow::bail!\("storage `\{s\}` has no CPU implementation"\),\n'),
         f'            "{name}" => $f::<{path} $(, $t)*>($($a),*),\n'),
        (Edit("occupancy test", C / "storage/cpu_tests.rs", r"^            check_occupied::<crate::storage::morton::Cpu>\(&d\);\n", before=False),
         f"            check_occupied::<{s}>(&d);\n"),
        (Edit("exactness test", C / "storage/cpu_tests.rs", r"^            check::<crate::storage::morton::Cpu>\(&d\);\n", before=False),
         f"            check::<{s}>(&d);\n"),
        (Edit("walk agreement test", C / "traversal/cpu_tests.rs", r"^            // Level subsets \(the `skip\*` knobs\)\.\n"),
         f"            agree::<{s}, false, 0b111111>(seed);\n"),
    ]


def registry_block(name, ty, desc):
    d = desc.replace("\\", "\\\\").replace('"', '\\"')
    return f'''    ComponentInfo {{
        role: Role::Storage,
        name: "{name}",
        description: "{d}",
        impls: &[Impl {{
            kind: Kind::Cpu,
            apis: ISAS,
            provides: <storage::{name}::cpu::{ty} as Repr>::CAPS,
        }}],
        needs: &[],
    }},
'''


def cmd_storage(args):
    if not args:
        die("storage NAME [description]")
    name = args[0]
    desc = args[1] if len(args) > 1 else f"{name}: a CPU storage (describe its layout and what it trades)."
    if not re.fullmatch(r"[a-z][a-z0-9_]*", name) or name in KEYWORDS:
        die("NAME: lower-case letters, digits, _; not a Rust keyword")
    if (C / "storage" / name).exists() or f'name: "{name}"' in (C / "lib.rs").read_text():
        die(f"`{name}` already exists")
    ty = camel(name)
    # Fail before touching anything if an anchor is missing.
    plan = sites(name, ty, "x")
    for e, _ in plan:
        e.where(e.path.read_text())
    d = C / "storage" / name
    d.mkdir()
    src = (C / "storage/bricks/cpu.rs").read_text()
    src = src.replace("`bricks` storage", f"`{name}` storage").replace("U16", ty)
    src = src.replace("cpu-simd's representation: 512-bit", "The starting point, a copy of `bricks`: 512-bit")
    (d / "cpu.rs").write_text(src)
    (d / "mod.rs").write_text(f"//! Storage `{name}`: {desc} CPU only.\n\npub mod cpu;\n")
    touched = [d / "cpu.rs", d / "mod.rs"]
    for (e, text), kind in zip(plan, range(len(plan))):
        if e.label == "registry":
            text = registry_block(name, ty, desc)
        if e.label == "module list":
            # keep `pub mod` lines sorted: insert before the first greater one
            p = e.path
            lines = p.read_text().splitlines(keepends=True)
            mods = [i for i, l in enumerate(lines) if re.match(r"pub mod \w+;", l)]
            at = next((i for i in mods if lines[i].split()[2].rstrip(";") > name), mods[-1] + 1)
            lines.insert(at, f"pub mod {name};\n")
            p.write_text("".join(lines))
        else:
            e.insert(text)
        touched.append(e.path)
    for p in dict.fromkeys(touched):
        print(p.relative_to(ROOT))
    print(f"\nnext: edit crates/mr-components/src/storage/{name}/cpu.rs (build/probe/occupied/block, LEVELS, CAPS,"
          f" bytes), then `scripts/new.py try {name} --sweep`")


def cmd_remove(args):
    if len(args) != 2 or args[0] != "storage":
        die("remove storage NAME")
    name = args[1]
    d = C / "storage" / name
    if not d.exists():
        die(f"no storage dir {d.relative_to(ROOT)}")
    for e, _ in sites(name, camel(name), "x"):
        text = e.path.read_text()
        out = text
        if e.label == "registry":
            out = re.sub(rf'    ComponentInfo \{{\n        role: Role::Storage,\n        name: "{re.escape(name)}",\n.*?\n    \}},\n', "", text, flags=re.S)
        elif e.label == "module list":
            out = re.sub(rf"^pub mod {re.escape(name)};\n", "", text, flags=re.M)
        else:
            pat = rf'^.*(?:"{re.escape(name)}" =>|::<crate::storage::{re.escape(name)}::cpu::|store::<s::{re.escape(name)}::).*\n'
            out = re.sub(pat, "", text, flags=re.M)
            out = re.sub(rf'^.*{re.escape(name)}::cpu::{camel(name)}.*\n', "", out, flags=re.M)
        if out != text:
            e.path.write_text(out)
            print("edited", e.path.relative_to(ROOT))
    for f in d.iterdir():
        f.unlink()
    d.rmdir()
    print("removed", d.relative_to(ROOT))


def run(cmd):
    print("$", " ".join(cmd))
    return subprocess.run(cmd, cwd=ROOT).returncode == 0


def cmd_try(args):
    sweep = "--sweep" in args
    names = [a for a in args if not a.startswith("-")]
    if len(names) != 1:
        die("try NAME [--sweep]")
    name = names[0]
    if f'name: "{name}"' not in (C / "lib.rs").read_text():
        die(f"`{name}` is not registered")
    ok = run(["cargo", "test", "-q", "-p", "mr-components"])
    ok = run(["cargo", "test", "-q", "--workspace", "registry"]) and ok
    ok = run([sys.executable, str(ROOT / "scripts/check.py")]) and ok
    if sweep and ok:
        ok = run([sys.executable, str(ROOT / "scripts/run1.py"),
                  '[{graph="raycast-cpu",edits=[{node="storage",component="%s"}]}]' % name])
    print("try:", "passed" if ok else "FAILED")
    if not ok:
        print("if only the sweep-size tests failed (the new component joins every sweep), `scripts/new.py accept-sweep` rewrites the size")
    sys.exit(0 if ok else 1)


def cmd_accept_sweep(_):
    r = subprocess.run(["cargo", "test", "-q", "--release", "pair_covering_sweeps"], cwd=ROOT, capture_output=True, text=True)
    m = re.search(r"left: (\d+)\s+right: (\d+)", r.stdout + r.stderr)
    if not m:
        print("accept-sweep: the sweep size test passes, nothing to accept" if r.returncode == 0 else "accept-sweep: failed some other way:\n" + (r.stdout + r.stderr)[-1500:])
        return
    new, old = m.group(1), m.group(2)
    for path in [ROOT / "src/plan/tests.rs"]:
        text = path.read_text()
        if old in text:
            path.write_text(re.sub(rf"\b{old}\b", new, text))
            print(f"{path.relative_to(ROOT)}: {old} -> {new}")


def cmd_sites(_):
    for e, _t in sites("NAME", "Name", "x"):
        print(f"{e.label:22} {e.path.relative_to(ROOT)}")


def main():
    cmds = {"storage": cmd_storage, "remove": cmd_remove, "try": cmd_try, "sites": cmd_sites, "accept-sweep": cmd_accept_sweep}
    if len(sys.argv) < 2 or sys.argv[1] not in cmds:
        print(__doc__)
        sys.exit(2)
    cmds[sys.argv[1]](sys.argv[2:])


main()
