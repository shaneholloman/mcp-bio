from __future__ import annotations

import os
import shlex
import subprocess
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SETUP_SCRIPT = REPO_ROOT / "spec" / "fixtures" / "setup-mychem-empty-spec-fixture.sh"
CLEANUP_SCRIPT = (
    REPO_ROOT / "spec" / "fixtures" / "cleanup-mychem-empty-spec-fixture.sh"
)


def parse_export_env(env_file: Path) -> dict[str, str]:
    env: dict[str, str] = {}
    for raw_line in env_file.read_text(encoding="utf-8").splitlines():
        if not raw_line.startswith("export "):
            continue
        name, raw_value = raw_line[len("export ") :].split("=", 1)
        env[name] = shlex.split(raw_value)[0]
    return env


def wait_for_process_exit(pid: int, timeout: float = 5.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if not Path(f"/proc/{pid}").exists():
            return
        time.sleep(0.1)
    raise AssertionError(f"process {pid} should have exited")


def test_mychem_empty_fixture_setup_and_cleanup_round_trip(tmp_path: Path) -> None:
    workspace = tmp_path
    subprocess.run(
        ["bash", str(SETUP_SCRIPT), str(workspace)],
        check=True,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )

    env_file = workspace / ".cache" / "spec-mychem-empty-env"
    env = parse_export_env(env_file)
    pid = int(env["BIOMCP_MYCHEM_EMPTY_PID"])
    fixture_root = Path(env["BIOMCP_MYCHEM_EMPTY_ROOT"])
    ready_file = Path(env["BIOMCP_MYCHEM_EMPTY_READY_FILE"])

    assert fixture_root.is_dir()
    assert ready_file.is_file()
    os.kill(pid, 0)

    subprocess.run(
        ["bash", str(CLEANUP_SCRIPT), str(workspace)],
        check=True,
        cwd=REPO_ROOT,
    )

    wait_for_process_exit(pid)
    assert not env_file.exists()
    assert not fixture_root.exists()


def test_mychem_empty_cleanup_ignores_unowned_pid_and_root(tmp_path: Path) -> None:
    workspace = tmp_path
    cache_dir = workspace / ".cache"
    cache_dir.mkdir()
    env_file = cache_dir / "spec-mychem-empty-env"
    foreign_root = tmp_path / "foreign-root"
    foreign_root.mkdir()
    foreign_ready_file = tmp_path / "foreign-ready-file"
    foreign_ready_file.write_text("http://127.0.0.1:0\n", encoding="utf-8")
    sleeper = subprocess.Popen(["sleep", "60"])

    try:
        env_file.write_text(
            "\n".join(
                [
                    f"export BIOMCP_MYCHEM_EMPTY_PID={sleeper.pid}",
                    f"export BIOMCP_MYCHEM_EMPTY_READY_FILE={shlex.quote(str(foreign_ready_file))}",
                    f"export BIOMCP_MYCHEM_EMPTY_ROOT={shlex.quote(str(foreign_root))}",
                ]
            )
            + "\n",
            encoding="utf-8",
        )

        subprocess.run(
            ["bash", str(CLEANUP_SCRIPT), str(workspace)],
            check=True,
            cwd=REPO_ROOT,
        )

        assert sleeper.poll() is None
        assert foreign_root.exists()
        assert not env_file.exists()
    finally:
        sleeper.terminate()
        sleeper.wait(timeout=5)
