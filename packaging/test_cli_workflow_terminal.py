"""Opt-in Windows ConPTY regression. Uses only isolated mock servers and fake secrets.

Build: cargo test -p oxvif-cli --bin oxvif --no-run
Run: python packaging/test_cli_workflow_terminal.py --binary <printed test executable>
Requires pywinpty and pyte (optional --deps directory). No native secret-store writes.
"""
import argparse
import os
from pathlib import Path
import select
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--binary", required=True, type=Path)
parser.add_argument("--deps", type=Path, default=ROOT / "target/terminal-deps")
parser.add_argument("--mode", choices=["discover", "manage", "all"], default="all")
args = parser.parse_args()
sys.path.insert(0, str(args.deps))
from winpty import PtyProcess
import pyte


class Terminal:
    def __init__(self, mode):
        self.directory = Path(tempfile.mkdtemp(prefix=f"workflow-{mode}-", dir=ROOT / "target"))
        env = {k: v for k, v in os.environ.items()
               if not k.upper().startswith(("OXVIF_", "ONVIF_"))
               and k.upper() not in {"HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"}}
        env.update(OXVIF_TERMINAL_FIXTURE_DIR=str(self.directory),
                   OXVIF_TERMINAL_FIXTURE_MODE=mode,
                   OXVIF_CONFIG_DIR=str(self.directory), NO_PROXY="127.0.0.1,localhost")
        self.proc = PtyProcess.spawn(
            [str(args.binary.resolve()), "--exact",
             "terminal_fixture::workflow_terminal_fixture", "--ignored", "--nocapture"],
            env=env, cwd=str(ROOT), dimensions=(28, 160), backend=0)
        self.screen = pyte.Screen(160, 28)
        self.stream = pyte.Stream(self.screen)
        self.raw = ""

    def pump(self, seconds=0.12):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if select.select([self.proc.fileobj], [], [], 0.03)[0]:
                try:
                    part = self.proc.read(65536)
                except EOFError:
                    return
                self.raw += part
                self.stream.feed(part)

    def shown(self):
        return "\n".join(self.screen.display)

    def expect(self, text, timeout=12):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            self.pump()
            if text in self.shown():
                return
        raise AssertionError(f"Missing screen: {text}")

    def key(self, value):
        self.proc.write(value)
        self.pump()

    def resize(self, rows, columns):
        self.screen.resize(rows, columns)
        self.proc.setwinsize(rows, columns)
        self.pump()

    def menu(self, ordinal):
        self.key(f"{ordinal}G\r")

    def credentials(self, password="admin"):
        self.menu(8)
        self.expect("ONVIF username")
        self.key("admin\r")
        self.expect("ONVIF password")
        self.key(password + "\r")
        self.expect("Credentials updated")
        self.key("\r")

    def add(self, device_id, password="admin"):
        self.key("a")
        self.expect("onboard selected device")
        self.key("\x15" + device_id + "\radmin\r" + password + "\r")

    def finish(self):
        end = time.monotonic() + 8
        while self.proc.isalive() and time.monotonic() < end:
            self.pump()
        assert not self.proc.isalive(), "fixture did not exit"
        assert self.proc.exitstatus == 0 and "TERMINAL_FIXTURE_PASS" in self.raw, "fixture assertions failed"
        assert "wrong-password-fixture" not in self.raw, "secret leaked into terminal"

    def close(self):
        # Artifacts contain fake fixtures only; do not print complete terminal records.
        (self.directory / "terminal.vt").write_text(self.raw, encoding="utf8")
        (self.directory / "last-screen.txt").write_text(self.shown(), encoding="utf8")
        if self.proc.isalive():
            self.proc.write("\x03")
            self.pump(0.3)
        self.proc.close()


def discover(t):
    t.expect("oxvif discovery")
    t.add("added-one", "wrong-password-fixture")
    t.expect("Setup failed")
    t.key("\r")
    t.expect("password cleared")
    assert "added-one" in t.shown() and "admin" in t.shown()
    t.key("admin\r")
    t.expect("Camera saved")
    t.key("\r")
    t.expect("1 saved")
    t.key("n")
    t.add("added-two")
    t.expect("Camera saved")
    t.key("\r")
    t.expect("2 saved")
    assert "1 shown" in t.shown(), "NEW filter did not update after registration"
    t.key("a")
    t.expect("onboard selected device")
    t.key("\x1b")
    t.expect("oxvif discovery")
    t.key("q")
    t.finish()


def manage(t):
    t.expect("Choose camera")
    t.key("/cam255\r\r")
    t.expect("cam255 | Profile: not selected")
    t.credentials()
    t.menu(2)
    t.expect("Select profile")
    t.menu(2)
    t.expect("cam255 | Profile:")
    chosen_title = t.screen.display[0].split(" | ")[1]
    assert "not selected" not in chosen_title
    t.menu(7)
    t.expect("Operation finished")
    t.key("\r")
    t.menu(4)
    t.expect("New output path")
    output = str(t.directory / "inventory.json")
    t.key(output + "\r")
    t.expect("Operation finished")
    t.key("\r")
    t.menu(6)
    t.expect("Last completed result")
    t.key("/device\rG")
    report = t.shown()
    t.key("q")
    t.menu(6)
    t.expect("Last completed result")
    assert t.shown() == report, "report query/position not retained"
    t.key("q")
    t.menu(5)
    t.expect("Choose comparison baseline")
    t.key("\r")
    t.expect("Existing inventory path")
    assert "inventory.json" in t.shown()
    t.key("\r")
    t.expect("Operation finished")
    t.key("\r")
    original_inventory = (t.directory / "inventory.json").read_bytes()
    (t.directory / "inventory.json").unlink()
    t.menu(5)
    t.expect("Choose comparison baseline")
    t.key("\r\r")
    t.expect("Invalid baseline; correct the path")
    t.key("\r")
    t.expect("Existing inventory path")
    assert "inventory.json" in t.shown()
    (t.directory / "inventory.json").write_bytes(original_inventory)
    t.key("\x1b")
    t.menu(4)
    t.expect("New output path")
    t.key("\r")
    t.expect("Choose another destination")
    assert (t.directory / "inventory.json").read_bytes() == original_inventory, "existing export overwritten"
    t.key("\r\x1b")
    t.credentials("wrong-password-fixture")
    t.menu(7)
    t.expect("Operation failed")
    t.key("\r")
    t.menu(11)
    t.expect("Latest failure / cancellation (historical)")
    assert "No failure" not in t.shown()
    t.key("q")
    t.credentials()
    t.key("q")
    t.expect("Choose camera")
    assert "Search: cam255" in t.shown()
    t.key("/\x15cam254\r\r")
    t.expect("cam254 | Profile: not selected")
    t.menu(7)
    t.expect("Operation failed")  # A's temporary credentials must not carry to B.
    t.key("\rq")
    t.expect("Choose camera")
    t.key("/\x15cam255\r\r")
    t.expect("cam255 | Profile:")
    assert chosen_title in t.screen.display[0], "profile lost across A-B-A"
    t.menu(7)
    t.expect("Operation finished")  # A's own temporary credentials still work.
    t.key("\r")
    (t.directory / "clear-profiles").write_text("fixture only", encoding="utf8")
    end = time.monotonic() + 4
    while not (t.directory / "profiles-cleared").exists() and time.monotonic() < end:
        t.pump()
    assert (t.directory / "profiles-cleared").is_file(), "fixture profile mutation not acknowledged"
    t.menu(2)
    t.expect("No media profiles")
    t.key("\r")
    t.expect("cam255 | Profile: not selected")
    t.key("q")
    t.expect("Choose camera")
    t.key("/\x15cam253\r\r")
    t.expect("cam253 | Profile:")
    t.menu(7)
    t.expect("Working...")
    t.key("\x1b")
    t.expect("Operation cancelled")
    t.key("\r")
    t.menu(11)
    t.expect("Latest failure / cancellation")
    assert "Operation cancelled" in t.shown()
    t.key("qq")
    t.expect("Choose camera")
    t.key("/\x15no-such-fixture\r")
    assert "0 matches" in t.shown()
    assert "Return to search results" in t.shown() and "Enter device address" in t.shown()
    t.resize(10, 60)
    t.expect("Choose camera")
    t.resize(28, 160)
    t.expect("0 matches")
    t.key("cG\r")
    t.expect("IP address, hostname")
    t.key("http://[bad\r")
    t.expect("Invalid device address")
    t.key("\r")
    t.expect("IP address, hostname")
    assert "http://[bad" in t.shown(), "invalid input was lost"
    t.key("\x15127.0.0.1:9\x1b")
    t.expect("Choose camera")
    t.key("\r")
    t.expect("IP address, hostname")
    assert "127.0.0.1:9" in t.shown(), "cancelled draft was lost"
    t.key("\x1b")
    t.expect("Choose camera")
    t.key("Gk\r")
    t.expect("oxvif manage discovery")
    t.key("n")
    t.add("added-three")
    t.expect("Camera saved")
    t.key("\r")
    t.expect("0 shown")
    t.key("A")
    t.key("G\r")
    t.expect("added-three | Profile:")
    t.menu(7)
    t.expect("Operation finished")  # Explicitly saved credentials resolve for the new ID.
    t.key("\rq")
    t.expect("oxvif manage discovery")
    t.key("qq")
    t.finish()


for mode in (["discover", "manage"] if args.mode == "all" else [args.mode]):
    terminal = Terminal(mode)
    try:
        globals()[mode](terminal)
        print(f"PASS: {mode} isolated terminal workflow")
    finally:
        terminal.close()
