#!/usr/bin/env python3
"""MultiRenderer installer for Linux and Windows (Python 3.9+, standard library only).

    python3 install.py                 a wizard: shows the state, asks what to do
    python3 install.py install         prerequisites, sources, build, shortcuts
    python3 install.py update          new sources, rebuild; your edited plans are kept
    python3 install.py uninstall       removes everything it put down
    python3 install.py status

Options: --yes (no questions; defaults), --source URL|PATH (default: the checkout this file is
in, else GitHub), --no-shortcut, --purge (uninstall: also your plans and runs, and the
prerequisites this installer installed).

What it puts down (and removes again):
- the sources and their build under the data directory (`~/.local/share/multirenderer`,
  Windows `%LOCALAPPDATA%\\multirenderer`): compiled pipelines build against these sources
  with the Rust toolchain at run time, so both stay installed;
- the executable in `<data>/bin`, linked as `~/.local/bin/multirenderer` (Windows: `<data>\\bin`
  on the user's PATH);
- the shipped plans in `<data>/plans`; the GUI starts in `<data>` (plans, runs);
- a desktop entry (Windows: a Start menu shortcut);
- the compiled-pipeline cache (`~/.cache/multirenderer`; Windows: inside `<data>`);
- prerequisites when missing, each only after asking: the Rust toolchain (rustup, with
  llvm-tools for profile-guided builds), a C compiler (Linux: the distribution's package;
  Windows: Visual Studio Build Tools with C++ and ATL, which the linked-in DXC needs).
"""
import argparse
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

GITHUB = "https://github.com/ElipticalDrive/voxel-bench"
WINDOWS = os.name == "nt"
EXE = "multirenderer.exe" if WINDOWS else "multirenderer"
HERE = Path(__file__).resolve().parent
VSWHERE = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
VS_PARTS = ["Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "Microsoft.VisualStudio.Component.VC.ATL"]
LINUX_CC = {"dnf": ["gcc"], "apt-get": ["build-essential"], "pacman": ["base-devel"], "zypper": ["gcc"]}


def env_dir(var, default):
    v = os.environ.get(var)
    return Path(v) if v else default


if WINDOWS:
    DATA = env_dir("LOCALAPPDATA", Path.home() / "AppData" / "Local") / "multirenderer"
    CACHE = DATA / "pipelines"
    SHORTCUT = env_dir("APPDATA", Path.home() / "AppData" / "Roaming") / "Microsoft" / "Windows" / "Start Menu" / "Programs" / "MultiRenderer.lnk"
    LINK = None
else:
    DATA = env_dir("XDG_DATA_HOME", Path.home() / ".local" / "share") / "multirenderer"
    CACHE = env_dir("XDG_CACHE_HOME", Path.home() / ".cache") / "multirenderer"
    SHORTCUT = env_dir("XDG_DATA_HOME", Path.home() / ".local" / "share") / "applications" / "multirenderer.desktop"
    LINK = Path.home() / ".local" / "bin" / "multirenderer"
SRC, BIN, PLANS, MANIFEST = DATA / "src", DATA / "bin", DATA / "plans", DATA / "install.json"

YES = False


def say(msg):
    print(f"==> {msg}", flush=True)


def ask(question, default=True):
    if YES or not sys.stdin.isatty():
        return default
    hint = "[Y/n]" if default else "[y/N]"
    while True:
        a = input(f"{question} {hint} ").strip().lower()
        if not a:
            return default
        if a in ("y", "yes", "n", "no"):
            return a.startswith("y")


def run(*cmd, cwd=None, env=None, check=True):
    print("   $", " ".join(str(c) for c in cmd), flush=True)
    r = subprocess.run([str(c) for c in cmd], cwd=cwd, env=env)
    if check and r.returncode:
        sys.exit(f"failed (exit {r.returncode}): {cmd[0]}")
    return r.returncode


def download(url, dest=None):
    """The bytes at `url` (into `dest` if given), retried a few times."""
    say(f"downloading {url}")
    for attempt in range(4):
        try:
            with urllib.request.urlopen(url, timeout=60) as r:
                data = r.read()
            break
        except OSError as e:
            if attempt == 3:
                sys.exit(f"download failed: {url}: {e}")
            say(f"retrying ({e})")
    if dest:
        Path(dest).write_bytes(data)
    return data


def load_manifest():
    try:
        return json.loads(MANIFEST.read_text())
    except (OSError, ValueError):
        return None


def save_manifest(m):
    MANIFEST.write_text(json.dumps(m, indent=1))


# --- the Rust toolchain ---------------------------------------------------------------------

def cargo_home():
    return env_dir("CARGO_HOME", Path.home() / ".cargo")


def tool(name):
    """A Rust tool: on PATH, else in the rustup install's bin directory."""
    found = shutil.which(name)
    if found:
        return Path(found)
    p = cargo_home() / "bin" / (name + (".exe" if WINDOWS else ""))
    return p if p.exists() else None


def host_triple():
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(machine, machine)
    return f"{arch}-pc-windows-msvc" if WINDOWS else f"{arch}-unknown-linux-gnu"


def ensure_rust(m):
    if tool("cargo") is None:
        if not ask("No Rust toolchain found. Install it with rustup (into ~/.cargo, ~/.rustup)?"):
            sys.exit("a Rust toolchain is needed: https://rustup.rs")
        with tempfile.TemporaryDirectory() as t:
            init = Path(t) / ("rustup-init.exe" if WINDOWS else "rustup-init")
            download(f"https://static.rust-lang.org/rustup/dist/{host_triple()}/rustup-init{'.exe' if WINDOWS else ''}", init)
            init.chmod(0o755)
            run(init, "-y", "--profile", "minimal", "--default-toolchain", "stable")
        m["rustup"] = True
    rustup = tool("rustup")
    if rustup:
        # Edition 2024 needs 1.85; the tree tracks recent stable.
        run(rustup, "update", "stable", "--no-self-update", check=False)
        run(rustup, "component", "add", "llvm-tools")
    else:
        say("Rust is not managed by rustup: make sure it is recent and has llvm-tools (profile-guided builds)")


# --- a C compiler --------------------------------------------------------------------------

def vs_ready():
    if not VSWHERE.exists():
        return None, False
    out = subprocess.run([str(VSWHERE), "-products", "*", "-latest", "-property", "installationPath"], capture_output=True, text=True).stdout.strip()
    if not out:
        return None, False
    full = subprocess.run([str(VSWHERE), "-products", "*", "-latest", "-requires", *VS_PARTS, "-property", "installationPath"], capture_output=True, text=True).stdout.strip()
    return out, bool(full)


def ensure_cc(m):
    if WINDOWS:
        path, ready = vs_ready()
        if ready:
            return
        setup = VSWHERE.parent / "setup.exe"
        adds = sum((["--add", p] for p in VS_PARTS), [])
        if path:
            if not ask(f"Visual Studio at {path} lacks C++ or ATL (DXC is linked in and needs ATL). Add them?"):
                sys.exit("C++ build tools with ATL are needed")
            run(setup, "modify", "--installPath", path, *adds, "--quiet", "--norestart")
            m.setdefault("vs_added", []).extend(VS_PARTS)
            m["vs_path"] = path
        else:
            if not ask("No C++ build tools found. Install Visual Studio Build Tools (C++, ATL; several GB, asks for administrator)?"):
                sys.exit("C++ build tools are needed: https://visualstudio.microsoft.com/visual-cpp-build-tools/")
            with tempfile.TemporaryDirectory() as t:
                boot = Path(t) / "vs_buildtools.exe"
                download("https://aka.ms/vs/17/release/vs_buildtools.exe", boot)
                rc = run(boot, "--quiet", "--wait", "--norestart", "--nocache", "--add", "Microsoft.VisualStudio.Workload.VCTools", *adds, "--includeRecommended", check=False)
                if rc not in (0, 3010):
                    sys.exit(f"Build Tools installer failed (exit {rc})")
            m["vs_installed"] = vs_ready()[0]
        if not vs_ready()[1]:
            sys.exit("the C++ build tools are still incomplete; see the Visual Studio Installer")
    elif not (shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")):
        pm = next((p for p in LINUX_CC if shutil.which(p)), None)
        if pm is None:
            sys.exit("no C compiler found; install gcc or clang with your package manager")
        pkgs = LINUX_CC[pm]
        verb = {"pacman": ["-S", "--needed", "--noconfirm"], "zypper": ["install", "-y"]}.get(pm, ["install", "-y"])
        if not ask(f"No C compiler found. Install {' '.join(pkgs)} with sudo {pm}?"):
            sys.exit("a C compiler is needed")
        run("sudo", pm, *verb, *pkgs)
        m["packages"] = {"manager": pm, "names": pkgs}


# --- sources ---------------------------------------------------------------------------------

def default_source():
    return str(HERE) if (HERE / "Cargo.toml").exists() and (HERE / "src" / "main.rs").exists() else GITHUB


def is_url(s):
    return "://" in s or s.startswith("git@")


def fetch(m):
    """Puts the sources into SRC (keeping SRC/target for incremental rebuilds)."""
    source = m["source"]
    if is_url(source):
        if shutil.which("git") and (SRC / ".git").exists():
            run("git", "-C", SRC, "pull", "--ff-only")
        elif shutil.which("git") and not SRC.exists():
            run("git", "clone", "--depth", "1", source, SRC)
        else:
            if "github.com" not in source:
                sys.exit("without git only GitHub sources can be fetched (as an archive)")
            url = source.removesuffix(".git").rstrip("/") + "/archive/refs/heads/main.zip"
            z = zipfile.ZipFile(io.BytesIO(download(url)))
            clear_sources()
            top = z.namelist()[0].split("/")[0] + "/"
            for info in z.infolist():
                rel = info.filename[len(top):]
                if rel and not info.is_dir():
                    dest = SRC / rel
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    dest.write_bytes(z.read(info))
    else:
        src = Path(source)
        if shutil.which("git") and (src / ".git").exists():
            # Tracked and new files of the working tree, as they are now (uncommitted edits too).
            out = subprocess.run(["git", "-C", src, "ls-files", "-z", "--cached", "--others", "--exclude-standard"], capture_output=True, check=True).stdout
            files = [f for f in out.decode().split("\0") if f and (src / f).is_file()]
        else:
            files = [str(p.relative_to(src)) for p in src.rglob("*") if p.is_file() and p.relative_to(src).parts[0] not in ("target", ".git")]
        say(f"copying {len(files)} files from {src}")
        clear_sources()
        for f in files:
            dest = SRC / f
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src / f, dest)


def clear_sources():
    SRC.mkdir(parents=True, exist_ok=True)
    for p in SRC.iterdir():
        if p.name != "target":
            shutil.rmtree(p) if p.is_dir() and not p.is_symlink() else p.unlink()


def build(m):
    cargo = tool("cargo")
    env = dict(os.environ, PATH=f"{cargo.parent}{os.pathsep}{os.environ.get('PATH', '')}")
    run(cargo, "build", "--release", "--locked", cwd=SRC, env=env)
    BIN.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copy2(SRC / "target" / "release" / EXE, BIN / EXE)
    except PermissionError:
        sys.exit(f"cannot replace {BIN / EXE}: close MultiRenderer and run this again")
    if shutil.which("git") and (SRC / ".git").exists():
        m["commit"] = subprocess.run(["git", "-C", SRC, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()


def sync_plans(m):
    """Copies the shipped plans; on update replaces only those you have not edited."""
    PLANS.mkdir(parents=True, exist_ok=True)
    shipped = m.setdefault("plans", {})
    for p in sorted((SRC / "plans").glob("*.toml")):
        dest, new = PLANS / p.name, digest(p)
        if dest.exists() and digest(dest) not in (shipped.get(p.name), new):
            say(f"keeping your edited plans/{p.name}")
            continue
        shutil.copy2(p, dest)
        shipped[p.name] = new


def digest(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


# --- shortcuts -------------------------------------------------------------------------------

def shortcuts(m):
    exe = BIN / EXE
    SHORTCUT.parent.mkdir(parents=True, exist_ok=True)
    if WINDOWS:
        add_to_path(BIN)
        ps = (f"$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{SHORTCUT}'); $s.TargetPath = '{exe}'; "
              f"$s.WorkingDirectory = '{DATA}'; $s.Description = 'Voxel renderer test bench'; $s.Save()")
        run("powershell", "-NoProfile", "-Command", ps)
        return
    LINK.parent.mkdir(parents=True, exist_ok=True)
    if LINK.is_symlink() or LINK.exists():
        LINK.unlink()
    LINK.symlink_to(exe)
    if str(LINK.parent) not in os.environ.get("PATH", "").split(os.pathsep):
        say(f"{LINK.parent} is not on your PATH; add it to run `multirenderer` from a shell")
    # A desktop session may not have rustup's PATH: name the toolchain for compiled pipelines.
    cargo, rustc = tool("cargo"), tool("rustc")
    SHORTCUT.write_text(
        "[Desktop Entry]\nType=Application\nName=MultiRenderer\nComment=Voxel renderer test bench\n"
        f"Exec=env CARGO={cargo} RUSTC={rustc} {exe}\nPath={DATA}\nIcon=applications-graphics\n"
        "Terminal=false\nCategories=Graphics;Development;\n")
    if shutil.which("update-desktop-database"):
        subprocess.run(["update-desktop-database", str(SHORTCUT.parent)], capture_output=True)


def user_path(edit):
    """Reads and optionally rewrites the user's PATH (Windows registry), telling running programs."""
    import ctypes
    import winreg
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment", 0, winreg.KEY_READ | winreg.KEY_WRITE) as k:
        try:
            value, kind = winreg.QueryValueEx(k, "Path")
        except FileNotFoundError:
            value, kind = "", winreg.REG_EXPAND_SZ
        parts = [p for p in value.split(";") if p]
        new = edit(parts)
        if new != parts:
            winreg.SetValueEx(k, "Path", 0, kind, ";".join(new))
            ctypes.windll.user32.SendMessageTimeoutW(0xFFFF, 0x1A, 0, "Environment", 2, 5000, None)


def add_to_path(d):
    user_path(lambda parts: parts if str(d) in parts else parts + [str(d)])


def remove_from_path(d):
    user_path(lambda parts: [p for p in parts if p != str(d)])


# --- commands --------------------------------------------------------------------------------

def install(args):
    m = load_manifest()
    if m:
        say(f"already installed (from {m['source']}); updating instead")
        return update(args)
    m = {"version": 1, "source": args.source or default_source()}
    if not is_url(m["source"]):
        m["source"] = str(Path(m["source"]).resolve())
    say(f"installing MultiRenderer from {m['source']} into {DATA}")
    DATA.mkdir(parents=True, exist_ok=True)
    save_manifest(m)  # from here on uninstall can clean up whatever follows
    ensure_cc(m)
    save_manifest(m)
    ensure_rust(m)
    save_manifest(m)
    fetch(m)
    build(m)
    sync_plans(m)
    m["shortcut"] = not args.no_shortcut
    save_manifest(m)
    if m["shortcut"]:
        shortcuts(m)
    save_manifest(m)
    say(f"installed: {BIN / EXE}" + (" (open a new terminal for the PATH)" if WINDOWS else ""))


def update(args):
    m = load_manifest()
    if not m:
        sys.exit("not installed")
    if args.source:
        m["source"] = args.source if is_url(args.source) else str(Path(args.source).resolve())
    say(f"updating from {m['source']}")
    ensure_cc(m)
    ensure_rust(m)
    save_manifest(m)
    fetch(m)
    build(m)
    sync_plans(m)
    if m.get("shortcut"):
        shortcuts(m)
    save_manifest(m)
    say("updated")


def uninstall(args):
    m = load_manifest()
    if not m:
        sys.exit(f"not installed (no {MANIFEST})")
    keep_user = not args.purge and not ask("Also remove your plans and benchmark runs?", default=False)
    say("removing MultiRenderer")
    if WINDOWS:
        remove_from_path(BIN)
    elif LINK.is_symlink() and LINK.resolve() == (BIN / EXE).resolve():
        LINK.unlink()
    if SHORTCUT.exists():
        SHORTCUT.unlink()
        if not WINDOWS and shutil.which("update-desktop-database"):
            subprocess.run(["update-desktop-database", str(SHORTCUT.parent)], capture_output=True)
    for d in (SRC, BIN, CACHE):
        if d.exists():
            shutil.rmtree(d, ignore_errors=WINDOWS)
            if d.exists():
                say(f"could not remove {d} entirely (in use?)")
    shipped = m.get("plans", {})
    for p in PLANS.glob("*.toml") if PLANS.exists() else []:
        if not keep_user or shipped.get(p.name) == digest(p):
            p.unlink()
    if not keep_user:
        shutil.rmtree(DATA / "runs", ignore_errors=True)
    MANIFEST.unlink()
    remove_prerequisites(m, args.purge)
    for d in (PLANS, DATA):
        try:
            d.rmdir()
        except OSError:
            pass
    if DATA.exists():
        say(f"kept your files in {DATA}")
    say("uninstalled")


def remove_prerequisites(m, purge):
    if m.get("rustup") and tool("rustup") and (purge or ask("The Rust toolchain was installed for MultiRenderer. Remove it (rustup self uninstall)?", default=False)):
        run(tool("rustup"), "self", "uninstall", "-y", check=False)
    if WINDOWS:
        setup = VSWHERE.parent / "setup.exe"
        if m.get("vs_installed") and (purge or ask("Visual Studio Build Tools were installed for MultiRenderer. Remove them?", default=False)):
            run(setup, "uninstall", "--installPath", m["vs_installed"], "--quiet", "--norestart", check=False)
        elif m.get("vs_added") and (purge or ask("C++ / ATL were added to Visual Studio for MultiRenderer. Remove them?", default=False)):
            run(setup, "modify", "--installPath", m["vs_path"], *sum((["--remove", p] for p in m["vs_added"]), []), "--quiet", "--norestart", check=False)
    elif m.get("packages"):
        pm, names = m["packages"]["manager"], m["packages"]["names"]
        if purge or ask(f"{' '.join(names)} was installed for MultiRenderer. Remove it with sudo {pm}?", default=False):
            verb = {"pacman": ["-Rs", "--noconfirm"], "zypper": ["remove", "-y"], "apt-get": ["remove", "-y"]}.get(pm, ["remove", "-y"])
            run("sudo", pm, *verb, *names, check=False)


def status(_args=None):
    m = load_manifest()
    print(f"MultiRenderer: {'installed' if m else 'not installed'}" + (f" from {m['source']}" + (f" at {m['commit']}" if m.get("commit") else "") if m else ""))
    print(f"  data      {DATA}")
    print(f"  cache     {CACHE}")
    cargo = tool("cargo")
    print(f"  cargo     {cargo or 'missing'}")
    if WINDOWS:
        path, ready = vs_ready()
        print(f"  C++/ATL   {'ok' if ready else ('incomplete at ' + path if path else 'missing')}  {path or ''}")
    else:
        print(f"  cc        {shutil.which('cc') or shutil.which('gcc') or shutil.which('clang') or 'missing'}")
    return m


def wizard(args):
    m = status()
    choices = ["update", "uninstall", "quit"] if m else ["install", "quit"]
    while True:
        a = input(f"What now? ({' / '.join(choices)}) ").strip().lower()
        hit = [c for c in choices if a and c.startswith(a)]
        if len(hit) == 1:
            break
    {"install": install, "update": update, "uninstall": uninstall, "quit": lambda _a: None}[hit[0]](args)


def main():
    global YES
    p = argparse.ArgumentParser(description="Install, update or remove MultiRenderer.")
    p.add_argument("command", nargs="?", choices=["install", "update", "uninstall", "status"])
    p.add_argument("--yes", "-y", action="store_true", help="no questions; take the defaults")
    p.add_argument("--source", help=f"git URL or local directory (default: this checkout, else {GITHUB})")
    p.add_argument("--no-shortcut", action="store_true", help="no desktop entry / Start menu shortcut or PATH entry")
    p.add_argument("--purge", action="store_true", help="uninstall: also your plans and runs, and prerequisites this installed")
    args = p.parse_args()
    YES = args.yes
    if args.command is None:
        if sys.stdin.isatty() and not YES:
            return wizard(args)
        args.command = "status"
    {"install": install, "update": update, "uninstall": uninstall, "status": status}[args.command](args)


if __name__ == "__main__":
    main()
