#!/usr/bin/env python3
"""X11 testing without installing Xorg beside the host's Wayland session: a podman container
(scripts/x11/Containerfile, image `mr-x11`) runs a real X server, Xorg with the dummy driver
(default) or Xvfb, and openbox as its window manager; the repository is mounted at its own path,
so the host's build runs inside (same Fedora release, same libraries).

    scripts/x11.py run [opts] -- CMD...   CMD under DISPLAY; after --secs a shot of the screen,
                                          then CMD is stopped (unless it exited before)
    scripts/x11.py smoke [opts]           the GUI and the view window (the presenters' window
                                          code) on each Vulkan driver, a shot each
    scripts/x11.py shell [opts]           an interactive shell in the container

Options: --server xorg|xvfb, --gpu nvidia|radv|lavapipe (one Vulkan driver per run, chosen by
VK_DRIVER_FILES; smoke: all three unless given), --secs N (default 12), --shot PATH.
Shots and logs go to target/x11/. NVIDIA needs a CDI spec matching the running driver: kept in
~/.config/cdi/nvidia.yaml and regenerated (nvidia-ctk, no root) when the driver changes, because
/etc/cdi's copy goes stale on driver updates. The NVIDIA Vulkan ICD file is mounted by hand
(the generated spec leaves it out).

The Xwayland bridge is the other X11 path: the GUI test `gui_device_lost_hands_over` already
runs the GUI as an X11 client of Xwayland inside a headless kwin. Xwayland is a real X server
(the X protocol, winit's X11 back end, Vulkan's X11 surfaces), but it presents through Wayland;
this container is Xorg's own presentation path and window management.
"""
import argparse
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMAGE = "mr-x11"
CDI_DIR = Path.home() / ".config" / "cdi"
OUT = ROOT / "target" / "x11"
ICD = {
    "nvidia": "/usr/share/vulkan/icd.d/nvidia_icd.x86_64.json",
    "radv": "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
    "lavapipe": "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json",
}


def sh(*cmd, **kw):
    return subprocess.run([str(c) for c in cmd], **kw)


def ensure_image():
    if sh("podman", "image", "exists", IMAGE).returncode:
        print(f"building the {IMAGE} image", flush=True)
        sh("podman", "build", "-q", "-t", IMAGE, ROOT / "scripts" / "x11", check=True)


def nvidia_driver():
    r = sh("nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader", capture_output=True, text=True)
    return r.stdout.strip().splitlines()[0] if r.returncode == 0 and r.stdout.strip() else None


def ensure_cdi():
    """The user's CDI spec for the running NVIDIA driver; None when there is no NVIDIA GPU."""
    driver = nvidia_driver()
    if not driver:
        return None
    spec = CDI_DIR / "nvidia.yaml"
    if not spec.exists() or f".so.{driver}" not in spec.read_text():
        CDI_DIR.mkdir(parents=True, exist_ok=True)
        print(f"generating {spec} for NVIDIA driver {driver}", flush=True)
        sh("nvidia-ctk", "cdi", "generate", f"--output={spec}", check=True, capture_output=True)
    return spec


def podman_args(gpu, interactive=False):
    args = ["podman"]
    if gpu == "nvidia":
        if not ensure_cdi():
            sys.exit("no NVIDIA driver running (nvidia-smi)")
        args += ["--cdi-spec-dir", str(CDI_DIR)]
    args += ["run", "--rm", "--security-opt", "label=disable", "--device", "/dev/dri",
             "-v", f"{ROOT}:{ROOT}", "-w", str(ROOT), "-e", "HOME=/tmp/home", "-e", "XDG_RUNTIME_DIR=/tmp/run",
             "-e", "RUST_BACKTRACE=1"]
    if interactive:
        args += ["-it"]
    if gpu == "nvidia":
        args += ["--device", "nvidia.com/gpu=all", "-v", f"{ICD['nvidia']}:{ICD['nvidia']}:ro"]
    if gpu:
        args += ["-e", f"VK_DRIVER_FILES={ICD[gpu]}"]
    return args + [IMAGE]


def server_cmd(server):
    if server == "xvfb":
        return "Xvfb :1 -screen 0 1920x1080x24 -nolisten tcp >/tmp/x.log 2>&1 &"
    return "Xorg :1 -config /etc/X11/xorg-dummy.conf -noreset -nolisten tcp -logfile /tmp/x.log >/dev/null 2>&1 &"


def inner(server, body):
    """The script run in the container: the server, a wait for it, the window manager, body."""
    return (f"mkdir -p /tmp/home /tmp/run && chmod 700 /tmp/run; {server_cmd(server)} export DISPLAY=:1; "
            "for i in $(seq 100); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done; "
            "xdpyinfo >/dev/null 2>&1 || { echo 'X server did not start'; cat /tmp/x.log; exit 90; }; "
            "openbox >/dev/null 2>&1 & sleep 0.5; "
            f"echo \"x11: $(xdpyinfo | sed -n 's/^vendor string: *//p') $(xdpyinfo | sed -n 's/^X.Org version: *//p')\"; {body}")


def run_one(cmd, server, gpu, secs, shot):
    """Runs cmd (a list) under X; returns (exit status or None when stopped, output)."""
    shot = Path(shot).resolve()
    shot.parent.mkdir(parents=True, exist_ok=True)
    q = " ".join(shlex.quote(c) for c in cmd)
    body = (f"({q}) > /tmp/cmd.log 2>&1 & pid=$!; "
            f"for i in $(seq {int(secs * 10)}); do kill -0 $pid 2>/dev/null || break; sleep 0.1; done; "
            f"windows=$(xdotool search --onlyvisible --name . 2>/dev/null | while read w; do xdotool getwindowname $w; done | grep -v '^$' | paste -sd '|'); "
            f"import -window root {shlex.quote(str(shot))} 2>/dev/null; "
            "if kill -0 $pid 2>/dev/null; then kill $pid; sleep 1; kill -9 $pid 2>/dev/null; st=stopped; else wait $pid; st=$?; fi; "
            "cat /tmp/cmd.log; echo \"windows: $windows\"; echo \"status: $st\"")
    r = sh(*podman_args(gpu), "sh", "-c", inner(server, body), capture_output=True, text=True)
    return r.stdout + r.stderr


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["run", "smoke", "shell"])
    ap.add_argument("--server", choices=["xorg", "xvfb"], default="xorg")
    ap.add_argument("--gpu", choices=list(ICD))
    ap.add_argument("--secs", type=float, default=12)
    ap.add_argument("--shot")
    argv = sys.argv[1:]
    cmd = argv[argv.index("--") + 1:] if "--" in argv else []
    a = ap.parse_args(argv[:argv.index("--")] if "--" in argv else argv)
    ensure_image()
    if a.command == "shell":
        os.execvp("podman", podman_args(a.gpu, True) + ["sh", "-c", inner(a.server, "exec bash")])
    if a.command == "run":
        if not cmd:
            sys.exit("run: give a command after --")
        print(run_one(cmd, a.server, a.gpu, a.secs, a.shot or OUT / f"shot-{a.server}-{a.gpu or 'any'}.png"))
        return
    exe = ROOT / "target" / "release" / "multirenderer"
    if not exe.exists():
        sys.exit(f"build first: {exe} is missing (cargo build --release)")
    plan = OUT / "smoke-plan.toml"
    OUT.mkdir(parents=True, exist_ok=True)
    plan.write_text('[scene]\ngenerator = { type = "showcase" }\n'
                    'camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }\n'
                    '[run]\nresolution = [640, 360]\nview_radius = 3\n'
                    '[[renderer]]\ngraph = "raycast-gpu"\n[[renderer]]\ngraph = "raycast-cpu"\n')
    failed = []
    for gpu in [a.gpu] if a.gpu else list(ICD):
        if gpu == "nvidia" and not nvidia_driver():
            print("nvidia: skipped (no NVIDIA driver running)")
            continue
        for name, args in [("gui", ["gui", str(plan)]), ("view", ["view", str(plan)])]:
            shot = OUT / f"{name}-{a.server}-{gpu}.png"
            t = time.time()
            out = run_one([str(exe), *args], a.server, gpu, a.secs, shot)
            lines = out.strip().splitlines()
            status = next((l for l in reversed(lines) if l.startswith("status: ")), "status: ?")[8:]
            windows = next((l for l in reversed(lines) if l.startswith("windows: ")), "windows: ")[9:]
            ok = status == "stopped" and "multirenderer" in windows.lower()
            (OUT / f"{name}-{a.server}-{gpu}.log").write_text(out)
            print(f"{'ok  ' if ok else 'FAIL'} {name:5} {a.server} {gpu:9} {time.time() - t:5.1f} s  windows: {windows or '-'}  {status}  shot {shot.relative_to(ROOT)}")
            if not ok:
                failed.append(f"{name}/{gpu}")
                print("     " + "\n     ".join(lines[-15:]))
    if failed:
        sys.exit(f"failed: {', '.join(failed)} (logs in {OUT.relative_to(ROOT)})")


if __name__ == "__main__":
    main()
