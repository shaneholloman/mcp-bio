from __future__ import annotations

import re
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"
RELEASE_WORKFLOW = WORKFLOWS / "release.yml"

EXPECTED_NEEDS = {
    "version-check": [],
    "create-draft": ["version-check"],
    "build": ["version-check", "create-draft"],
    "pypi-build": ["version-check"],
    "wheel-smoke": ["pypi-build"],
    "docs-live": ["version-check"],
    "pypi-publish": ["pypi-build", "wheel-smoke", "docs-live"],
    "homebrew-tap": ["build", "docs-live", "wheel-smoke"],
    "container-publish": [
        "build",
        "docs-live",
        "version-check",
        "wheel-smoke",
        "create-draft",
    ],
    "publish-release": [
        "build",
        "pypi-publish",
        "homebrew-tap",
        "container-publish",
        "docs-live",
    ],
}


def _job_block(workflow: str, job: str) -> str:
    match = re.search(
        rf"^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
        re.MULTILINE | re.DOTALL,
    )
    assert match is not None, f"missing workflow job {job}"
    return match.group(1)


def _needs(block: str) -> list[str]:
    match = re.search(r"^    needs: \[([^]]*)\]$", block, re.MULTILINE)
    if match is None:
        return []
    return [part.strip() for part in match.group(1).split(",") if part.strip()]


def _assert_release_contract(workflow: str) -> None:
    assert "push:\n    tags: ['v*']" in workflow
    assert "permissions: {}" in workflow
    assert "group: release-${{ inputs.tag || github.ref_name }}" in workflow
    assert "github.event.release.tag_name" not in workflow
    for job, needs in EXPECTED_NEEDS.items():
        assert _needs(_job_block(workflow, job)) == needs, job

    expected_ifs = {
        "create-draft": "if: github.event_name == 'push'",
        "build": "if: github.event_name == 'push'",
        "pypi-build": "if: github.event_name == 'push'",
        "wheel-smoke": "if: github.event_name == 'push'",
        "pypi-publish": "if: github.event_name == 'push'",
        "homebrew-tap": "if: github.event_name == 'push'",
        "publish-release": "if: github.event_name == 'push'",
    }
    for job, condition in expected_ifs.items():
        assert condition in _job_block(workflow, job), job
    assert "\n    if:" not in _job_block(workflow, "version-check")
    assert "\n    if:" not in _job_block(workflow, "docs-live")

    container = _job_block(workflow, "container-publish")
    for clause in (
        "!cancelled()",
        "github.event_name == 'push' && success()",
        "inputs.container_only == true",
        "needs.version-check.result == 'success'",
        "needs.docs-live.result == 'success'",
        "needs.create-draft.result == 'skipped'",
        "needs.build.result == 'skipped'",
        "needs['wheel-smoke'].result == 'skipped'",
    ):
        assert clause in container
    assert "always()" not in workflow

    version = _job_block(workflow, "version-check")
    assert "fetch-depth: 0" in version
    assert "check-release-versions.py" in version
    assert "check-changelog-coverage.py" in version
    assert "continue-on-error" not in version
    for escape in ("|| true", "|| :", "; exit 0", "if: false", "if: ${{ false }}"):
        assert escape not in version, escape

    permissions = {
        "version-check": "contents: read",
        "create-draft": "contents: write",
        "build": "contents: write",
        "pypi-build": "contents: read",
        "docs-live": "contents: read",
        "homebrew-tap": "contents: read",
        "pypi-publish": "id-token: write",
        "container-publish": "packages: write",
        "publish-release": "contents: write",
    }
    for job, grant in permissions.items():
        assert grant in _job_block(workflow, job), job
    assert "permissions:" not in _job_block(workflow, "wheel-smoke")

    draft = _job_block(workflow, "create-draft")
    assert 'gh release create "$TAG" --draft --verify-tag' in draft
    build = _job_block(workflow, "build")
    assert 'gh release upload "$TAG"' in build and "--clobber" in build
    assert "skip-existing" not in workflow and "skip_existing" not in workflow

    smoke = _job_block(workflow, "wheel-smoke")
    for artifact in (
        "wheel-x86_64-unknown-linux-gnu",
        "wheel-aarch64-unknown-linux-gnu",
        "wheel-aarch64-apple-darwin",
        "wheel-x86_64-apple-darwin",
        "wheel-x86_64-pc-windows-msvc",
    ):
        assert artifact in smoke
    assert "Scripts/biomcp.exe" in smoke and "bin/biomcp" in smoke
    assert '"$status" -eq 101' in smoke and '"$status" -ge 128' in smoke
    assert "require_exit 0 drug interactions apixaban" in smoke
    assert "Drug not found in FAERS" in smoke
    assert "skill asset" in smoke

    final = _job_block(workflow, "publish-release")
    assert 'gh release edit "$TAG" --draft=false' in final
    assert "gh release view" in final
    assert "docker buildx imagetools create --tag" in final
    assert "gh release view" not in container
    assert "imagetools create --tag" not in container


def _assert_linux_floor_contract(workflow: str) -> None:
    """Pin the glibc 2.28 floor plumbing for Linux artifacts.

    Both Linux artifact families (tarballs and wheels) build inside
    manylinux_2_28 containers with checksum-pinned rustup, the wheel
    platform tag is asserted before the scan runs, and the runtime
    floor smoke exercises a deep offline command.
    """
    build = _job_block(workflow, "build")
    pypi = _job_block(workflow, "pypi-build")
    smoke = _job_block(workflow, "wheel-smoke")

    # Both Linux tarball legs build inside the manylinux containers.
    assert (
        "- os: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n"
        "            artifact: biomcp-linux-x86_64.tar.gz\n"
        "            container: quay.io/pypa/manylinux_2_28_x86_64\n" in build
    )
    assert (
        "- os: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n"
        "            artifact: biomcp-linux-arm64.tar.gz\n"
        "            container: quay.io/pypa/manylinux_2_28_aarch64\n" in build
    )
    assert "cross: true" not in build
    assert "Install cross-compilation toolchain" not in build
    assert "Build and package inside the manylinux 2_28 container" in build
    # The pre-tar binary carries the same floor as the wheel scan.
    assert (
        "scripts/check-wheel-glibc-floor.py --elf target/${{ matrix.target }}/release/biomcp 2.28"
        in build
    )
    # Host toolchain steps stay off the container legs.
    assert build.count("if: runner.os != 'Linux'") >= 2
    assert "- name: Package (macOS)\n        if: runner.os == 'macOS'" in build
    assert "Package (Unix)" not in build

    # rustup is pinned by version and checksum in both container jobs;
    # matrix.target is the rustup dist triple, and the matrix asserts
    # below pin both Linux legs (x86_64 and aarch64 on their native
    # containers), so the templated URL covers both archives.
    rustup_base = (
        "https://static.rust-lang.org/rustup/archive/1.28.2/${{ matrix.target }}"
    )
    for block in (build, pypi):
        assert f"{rustup_base}/rustup-init\n" in block
        assert f"{rustup_base}/rustup-init.sha256\n" in block
        assert "sha256sum -c rustup-init.sha256" in block
    assert (
        "- os: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n"
        "            container: quay.io/pypa/manylinux_2_28_x86_64\n" in pypi
    )
    assert (
        "- os: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n"
        "            container: quay.io/pypa/manylinux_2_28_aarch64\n" in pypi
    )
    # No remote script is piped into a shell anywhere in the release flow.
    assert "sh.rustup.rs" not in workflow
    assert "| sh -s" not in workflow

    # The wheel platform tag is pinned and asserted before the scan.
    assert "--compatibility manylinux_2_28" in pypi
    assert "wheel must end in $tag.whl: $wheel" in pypi

    # The runtime floor smoke runs a deep offline command, not only
    # the version banner, and the JSON guard is pinned exactly so a
    # quoting typo cannot ship.
    assert "cache stats --json" in smoke
    assert 'case "$stats" in' in smoke
    assert '"{"*) ;;' in smoke


def test_release_workflow_contract() -> None:
    _assert_release_contract(RELEASE_WORKFLOW.read_text(encoding="utf-8"))


def test_linux_floor_contract() -> None:
    _assert_linux_floor_contract(RELEASE_WORKFLOW.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# Data-driven contract over the parsed workflow for the pipeline jobs the
# issue's mutations target: pypi-build, wheel-smoke, and docs-live. The
# version-check no-escape assertions above stay text-based; these cover
# every step of the three jobs plus the trigger block, so a new step in
# any of them is guarded without a new hand-written assertion.
# ---------------------------------------------------------------------------

PIPELINE_JOBS = ("pypi-build", "wheel-smoke", "docs-live")

# A closed allowlist: any other step-level condition in the three
# pipeline jobs is a way to switch a check off, whatever syntax it
# uses (false, ${{ false }}, ${{ false && true }}, ${{true}}, ...).
# Only the conditions actually used by steps in the guarded jobs. A
# new legitimate condition requires an explicit, reviewable edit here.
ALLOWED_STEP_IFS = {
    "runner.os == 'Linux'",
    "runner.os != 'Linux'",
}

# Exit-code swallows are not banned by phrase: every step of the
# pinned jobs below is hash-pinned, so `|| echo skip`,
# `set +o errexit`, `trap -- 'exit 0' EXIT`, `|| exit $((0))`, or any
# other spelling changes the step's text and fails the hash. The
# patterns are gone on purpose — structure closed the class.
#
# Hash pinning: each load-bearing step is serialized from the parsed
# workflow (so YAML folding cannot hide a change), its `run` body is
# normalized to trivial whitespace (per-line trailing space stripped,
# blank runs collapsed to one blank line), and the SHA-256 of the
# canonical JSON is recorded below. ANY edit to a pinned step — run
# text, `if:`, `shell:`, `with:`, `continue-on-error`, `env:` — fails
# with the step's name and the new hash; deliberate changes update
# the hash here in the same commit.
    # Steps of the three pipeline jobs plus version-check's gate
    # steps, pinned 2026-09-27 (ticket 1258). Update a hash only in
    # the same commit as the deliberate workflow edit it records.
PINNED_STEPS: dict[tuple[str, str], str] = {
    ("pypi-build", "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683"): "435261111ae8c13c6efd4c0122d30af2df68e5cbc57a7e92ccd1568897ae4024",
    ("pypi-build", "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065"): "004630710366bff7851b00b676ce5044490a91b7220ce97a1f1e48fa16679424",
    ("pypi-build", "dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c"): "e5cf6427d1b42e2b776f2589adc36320874f109751f02494b68ee9068c79b0d5",
    ("pypi-build", "arduino/setup-protoc@c65c819552d16ad3c9b72d9dfd5ba5237b9c906b"): "4e605dbe89d4dcacdf70d170853634c9f4c518e1a41f5d2c5e2bd27cf4c89e7c",
    ("pypi-build", "Install pinned maturin"): "996e8a6336fff6d3d0b3a74230bb8b24021e34e7d06390412ac88bd2308173e0",
    ("pypi-build", "Build wheels"): "a6e876869ef10c81eb4b505376e37e1a370b4ca36b809a68bc6ea20d52b87d3e",
    ("pypi-build", "Build wheels inside the manylinux 2_28 container"): "21a93b7950bc2976defc9c23424265ac57d9cd4c2daf4a40940ad5ee683a8563",
    ("pypi-build", "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"): "b9b6cca0e685928f03d3f444bac8c0825b2ee3510663d6503c24525218de1e23",
    ("wheel-smoke", "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065"): "53e861877de3017c6e648667b07f052d923c6c50c065f7b9b60d72589c0887e0",
    ("wheel-smoke", "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"): "7da61a5393486e202557314e916ce6b47390f55bd62478135e9c7911337da88e",
    ("wheel-smoke", "Install the wheel into a clean venv"): "362a4ba663ff744574ec1f229c6ecd32e1f0f308ae95f0000f79efbf089bf6f9",
    ("wheel-smoke", "Run the wheel inside the manylinux 2_28 container"): "d5495643fdb712f52fbaa636b5198909e890806d92a309d073dfc1bb328cffc4",
    ("wheel-smoke", "Smoke the installed wheel on every shipped platform"): "a80bfec3bd118fc0be28eedcc5f76f48c1ef734fea95d49f8aa9d926a359aa9f",
    ("docs-live", "Check out the gate helper"): "afce43fafcab696d9cef03f29b0c43b6c9849baf126b749e198bdb9d83555430",
    ("docs-live", "Resolve the tag commit"): "b52e25a4026bed9172a0eff4b90f6a706ec984875307d7e89f9d450878a96e44",
    ("docs-live", "Require the live documentation revision to equal or descend from the tag"): "d8e88f95d2e890e14d314242bcce698767ddd834b4ccbab56469d5fd684eb54d",
    ("version-check", "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683"): "3ffb87228f0c77aecb87c34ee83d661a80bff50e397e911e917de5891db2a5fb",
    ("version-check", "Require the tag and committed versions to agree"): "8bdcc74fd872a7a77d8341bd5049e3a3845a89825f02875f4258e5d665243b8b",
    ("version-check", "Require the changelog to cover merged tickets"): "8dcf47fdb31ff44771a511058fa05d8e17d1a551744dcc1e8631fa7f8979118e",
}


def _normalize_step_text(text: str) -> str:
    lines = [line.rstrip() for line in text.splitlines()]
    out: list[str] = []
    blank = 0
    for line in lines:
        if line == "":
            blank += 1
            if blank <= 1:
                out.append(line)
        else:
            blank = 0
            out.append(line)
    return "\n".join(out).strip("\n")


def _canonical_step(step: dict) -> str:
    import copy
    import json as _json

    normalized = copy.deepcopy(step)
    if isinstance(normalized.get("run"), str):
        normalized["run"] = _normalize_step_text(normalized["run"])
    return _json.dumps(normalized, sort_keys=True, ensure_ascii=False, indent=1)


def _step_sha(step: dict) -> str:
    import hashlib

    return hashlib.sha256(_canonical_step(step).encode("utf-8")).hexdigest()

EXPECTED_MATRICES = {
    "pypi-build": [
        {
            "os": "ubuntu-24.04",
            "target": "x86_64-unknown-linux-gnu",
            "container": "quay.io/pypa/manylinux_2_28_x86_64",
        },
        {
            "os": "ubuntu-24.04-arm",
            "target": "aarch64-unknown-linux-gnu",
            "container": "quay.io/pypa/manylinux_2_28_aarch64",
        },
        {"os": "macos-14", "target": "aarch64-apple-darwin"},
        {"os": "macos-latest", "target": "x86_64-apple-darwin"},
        {"os": "windows-latest", "target": "x86_64-pc-windows-msvc"},
    ],
    "wheel-smoke": [
        {
            "os": "ubuntu-24.04",
            "artifact": "wheel-x86_64-unknown-linux-gnu",
            "executable": "bin/biomcp",
            "python": "bin/python",
            "container": "quay.io/pypa/manylinux_2_28_x86_64",
        },
        {
            "os": "ubuntu-24.04-arm",
            "artifact": "wheel-aarch64-unknown-linux-gnu",
            "executable": "bin/biomcp",
            "python": "bin/python",
            "container": "quay.io/pypa/manylinux_2_28_aarch64",
        },
        {
            "os": "macos-14",
            "artifact": "wheel-aarch64-apple-darwin",
            "executable": "bin/biomcp",
            "python": "bin/python",
        },
        {
            "os": "macos-latest",
            "artifact": "wheel-x86_64-apple-darwin",
            "executable": "bin/biomcp",
            "python": "bin/python",
        },
        {
            "os": "windows-latest",
            "artifact": "wheel-x86_64-pc-windows-msvc",
            "executable": "Scripts/biomcp.exe",
            "python": "Scripts/python.exe",
        },
    ],
}

# The only step in the release flow allowed to tolerate failure: the
# optional protoc installer, whose generated code is committed.
CONTINUE_ON_ERROR_ALLOW = ("arduino/setup-protoc",)


def _load_release_pipeline() -> dict:
    return yaml.safe_load(RELEASE_WORKFLOW.read_text(encoding="utf-8"))


def _pipeline_steps(parsed: dict, job: str) -> list[dict]:
    return parsed["jobs"][job]["steps"]


def _step_by_name(parsed: dict, job: str, needle: str) -> dict:
    for step in _pipeline_steps(parsed, job):
        if needle in (step.get("name") or step.get("uses") or ""):
            return step
    raise AssertionError(f"no step matching {needle!r} in {job}")


def _assert_step_hashes(parsed: dict) -> None:
    """Every pinned step matches its recorded SHA-256 exactly."""
    for (job, needle), expected in PINNED_STEPS.items():
        step = _step_by_name(parsed, job, needle)
        actual = _step_sha(step)
        assert actual == expected, (
            f"{job}/{needle}: step text drifted from its pinned hash.\n"
            f"  expected sha256: {expected}\n"
            f"  actual   sha256: {actual}\n"
            f"If the edit is deliberate, update PINNED_STEPS in this file "
            f"in the same commit (the workflow comment names the pins)."
        )


def _assert_no_defaults_shell(parsed: dict) -> None:
    """`shell:` may exist only inside a step.

    A job- or workflow-level `defaults.run.shell: bash {0}` would
    swallow step failures for every step at once, so no `defaults`
    key may exist anywhere in the release workflow.
    """
    assert "defaults" not in parsed, (
        "workflow-level defaults are forbidden (a defaults.run.shell "
        "would apply to every step at once)"
    )
    for job, spec in parsed["jobs"].items():
        assert "defaults" not in spec, f"{job}: job-level defaults are forbidden"


def _assert_pipeline_contract(parsed: dict) -> None:
    # The trigger block: tag pushes only, never a release publication
    # event (the YAML 1.1 `on:` key parses as boolean True).
    triggers = parsed[True]
    assert triggers["push"] == {"tags": ["v*"]}, "trigger block must be tag pushes only"
    assert "release" not in triggers, "trigger block must be tag pushes only"
    assert "workflow_dispatch" in triggers, (
        "trigger block must keep the manual dispatch"
    )

    for job in PIPELINE_JOBS:
        job_spec = parsed["jobs"][job]
        assert "continue-on-error" not in job_spec, (
            f"{job}: job-level continue-on-error is forbidden"
        )
        steps = _pipeline_steps(parsed, job)
        assert steps, f"{job}: job must have steps"
        for step in steps:
            uses = step.get("uses", "")
            # Only the optional protoc installer may tolerate failure.
            # Forbidding the key outright catches every spelling of
            # true: the boolean, "true", "${{ true }}", and numbers.
            if "continue-on-error" in step:
                assert any(allow in uses for allow in CONTINUE_ON_ERROR_ALLOW), (
                    f"{job}: continue-on-error is forbidden outside the protoc installer"
                )
            step_if = step.get("if")
            if step_if is not None:
                assert step_if in ALLOWED_STEP_IFS, (
                    f"{job}: step if {step_if!r} is not in the allowlist"
                )
            shell = step.get("shell")
            assert shell in (None, "bash"), (
                f"{job}: custom shell {shell!r} is forbidden; declare shell: bash"
            )

    # Matrix entries are pinned exactly, so a swapped runner, a new
    # leg, or a dropped leg (including the ARM wheel build and smoke)
    # breaks the contract.
    for job, expected in EXPECTED_MATRICES.items():
        matrix = parsed["jobs"][job]["strategy"]["matrix"]["include"]
        assert matrix == expected, f"{job}: matrix entries must be pinned exactly"
    assert parsed["jobs"]["docs-live"].get("strategy") is None
    assert parsed["jobs"]["docs-live"]["runs-on"] == "ubuntu-24.04"

    # The floor check runs in both wheel legs: the two Linux matrix
    # entries above carry their containers, and the container build
    # step scans the artifact that ships.
    wheel_build = _step_by_name(
        parsed, "pypi-build", "Build wheels inside the manylinux 2_28 container"
    )
    assert "check-wheel-glibc-floor.py" in wheel_build["run"], (
        "pypi-build: the wheel build step must run the floor check"
    )

    smoke = _step_by_name(parsed, "wheel-smoke", "Smoke the installed wheel")
    # The panic guard must return failure, not just print.
    assert re.search(
        r'echo "panic or crash \(exit \$status\) from: biomcp \$\*" >&2\s*\n\s*return 1',
        smoke["run"],
    ), "wheel-smoke: the panic guard must return failure"
    # The not-found exit check runs after the adverse-events probe.
    run = smoke["run"]
    probe = run.index("drug adverse-events qwertyzzznonexistent999")
    assert '[ "$status" -eq 0 ]' in run[probe:], (
        "wheel-smoke: the not-found exit check must follow the adverse-events probe"
    )
    assert 'grep -q "Drug not found in FAERS"' in run[probe:], (
        "wheel-smoke: the not-found exit check must follow the adverse-events probe"
    )

    # The runtime floor smoke stays on the Linux legs only.
    floor_smoke = _step_by_name(
        parsed, "wheel-smoke", "Run the wheel inside the manylinux 2_28 container"
    )
    assert floor_smoke["if"] == "runner.os == 'Linux'", (
        "wheel-smoke: the runtime floor smoke must stay on the Linux legs"
    )

    # docs-live: the tag lookup must assert the SHA it resolved, and
    # the revision timeout branch must fail the step.
    resolve = _step_by_name(parsed, "docs-live", "Resolve the tag commit")
    assert 'test -n "$TAG_SHA"' in resolve["run"], (
        "docs-live: the tag lookup must assert the resolved SHA"
    )
    revision = _step_by_name(
        parsed, "docs-live", "Require the live documentation revision"
    )
    assert re.search(
        r"did not reach \$\{TAG_SHA\}[^\"\n]*\" >&2\s*\n\s*exit 1\b",
        revision["run"],
    ), "docs-live: the revision timeout branch must end in exit 1"


def test_pipeline_jobs_contract() -> None:
    parsed = _load_release_pipeline()
    _assert_pipeline_contract(parsed)
    _assert_publish_gating(parsed)
    _assert_no_defaults_shell(parsed)
    _assert_step_hashes(parsed)


# Every job's condition, asserted exactly. A suffix like
# `&& !cancelled()` would re-run a publish after an upstream failure
# and must break the contract.
EXPECTED_JOB_IFS = {
    "create-draft": "github.event_name == 'push'",
    "build": "github.event_name == 'push'",
    "pypi-build": "github.event_name == 'push'",
    "wheel-smoke": "github.event_name == 'push'",
    "pypi-publish": "github.event_name == 'push'",
    "homebrew-tap": "github.event_name == 'push'",
    "publish-release": "github.event_name == 'push'",
    "container-publish": (
        "!cancelled() && ((github.event_name == 'push' && success()) || "
        "(inputs.container_only == true && needs.version-check.result == 'success' && "
        "needs.docs-live.result == 'success' && needs.create-draft.result == 'skipped' && "
        "needs.build.result == 'skipped' && needs['wheel-smoke'].result == 'skipped'))"
    ),
}


def _assert_publish_gating(parsed: dict) -> None:
    for job in ("version-check", "docs-live"):
        assert "if" not in parsed["jobs"][job], f"{job}: must run unconditionally"
    for job, expected in EXPECTED_JOB_IFS.items():
        actual = parsed["jobs"][job].get("if")
        assert actual == expected, (
            f"{job}: job if must be exactly {expected!r}, got {actual!r}"
        )
    # `!cancelled()` re-runs a job after an upstream failure. Only the
    # container rebuild path may use it; a publish path never may.
    for job, spec in parsed["jobs"].items():
        if job == "container-publish":
            continue
        assert "!cancelled()" not in (spec.get("if") or ""), (
            f"{job}: !cancelled() is forbidden"
        )
        for step in spec.get("steps", []):
            assert "!cancelled()" not in (step.get("if") or ""), (
                f"{job}: !cancelled() is forbidden in a step if"
            )


def _set_job_flag(job: str, key: str, value):
    def mutate(parsed: dict) -> None:
        parsed["jobs"][job][key] = value

    return mutate


def _mutated_pipeline(mutate) -> dict:
    parsed = _load_release_pipeline()
    mutate(parsed)
    return parsed


def _set_step_flag(job: str, needle: str, key: str, value):
    def mutate(parsed: dict) -> None:
        _step_by_name(parsed, job, needle)[key] = value

    return mutate


def _mutate_run(job: str, needle: str, transform):
    def mutate(parsed: dict) -> None:
        step = _step_by_name(parsed, job, needle)
        step["run"] = transform(step["run"])

    return mutate


def _drop_matrix_entry(job: str, target: str):
    def mutate(parsed: dict) -> None:
        include = parsed["jobs"][job]["strategy"]["matrix"]["include"]
        kept = [entry for entry in include if entry.get("target") != target]
        assert len(kept) < len(include), target
        parsed["jobs"][job]["strategy"]["matrix"]["include"] = kept

    return mutate


def _retarget_matrix_os(job: str, key: str, value: str, os_name: str):
    def mutate(parsed: dict) -> None:
        include = parsed["jobs"][job]["strategy"]["matrix"]["include"]
        for entry in include:
            if entry.get(key) == value:
                entry["os"] = os_name
                return
        raise AssertionError(value)

    return mutate


HASH_DRIFT = "step text drifted from its pinned hash"

PIPELINE_MUTATIONS = {
    # --- Leak spellings the old phrase bans never named (ticket
    # --- 1258): each edits a pinned step, so the hash fails.
    "or_echo_skip_on_the_venv_smoke": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: run.replace("set -euo pipefail", "set -euo pipefail || echo skip", 1)
            if "set -euo pipefail" in run
            else run + "\ntrue || echo skip\n",
        ),
        HASH_DRIFT,
    ),
    "set_plus_o_errexit_in_the_venv_smoke": (
        _mutate_run(
            "wheel-smoke", "Smoke the installed wheel", lambda run: "set +o errexit\n" + run
        ),
        HASH_DRIFT,
    ),
    "trap_dash_dash_exit_zero_in_the_venv_smoke": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: run + "\ntrap -- 'exit 0' EXIT\n",
        ),
        HASH_DRIFT,
    ),
    "arithmetic_exit_zero_on_the_floor_check": (
        _mutate_run(
            "pypi-build",
            "Build wheels inside the manylinux 2_28 container",
            lambda run: run.replace(
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28",
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28 || exit $((0))",
            ),
        ),
        HASH_DRIFT,
    ),
    "one_character_edit_in_a_pinned_step": (
        # A single character of pinned prose fails the hash: the pin
        # covers everything, not a list of known escapes. (The step's
        # exit-1 branch has its own content assertion, so the mutated
        # character is prose, not the exit code.)
        _mutate_run(
            "docs-live",
            "Require the live documentation revision",
            lambda run: run.replace("revision", "revixion", 1),
        ),
        HASH_DRIFT,
    ),
    "runner_os_linux_if_on_the_main_smoke_step": (
        # Would turn off the macOS and Windows smoke legs entirely.
        _set_step_flag(
            "wheel-smoke", "Smoke the installed wheel", "if", "runner.os == 'Linux'"
        ),
        HASH_DRIFT,
    ),
    "workflow_level_defaults_shell_bash_brace_zero": (
        lambda parsed: parsed.update(
            {"defaults": {"run": {"shell": "bash {0}"}}}
        ),
        "workflow-level defaults are forbidden",
    ),
    "job_level_defaults_shell_bash_brace_zero": (
        lambda parsed: parsed["jobs"]["wheel-smoke"].update(
            {"defaults": {"run": {"shell": "bash {0}"}}}
        ),
        "job-level defaults are forbidden",
    ),
    "continue_on_error_on_venv_smoke": (
        _set_step_flag(
            "wheel-smoke", "Smoke the installed wheel", "continue-on-error", True
        ),
        "continue-on-error is forbidden outside the protoc installer",
    ),
    "continue_on_error_template_true_on_venv_smoke": (
        _set_step_flag(
            "wheel-smoke",
            "Smoke the installed wheel",
            "continue-on-error",
            "${{ true }}",
        ),
        "continue-on-error is forbidden outside the protoc installer",
    ),
    "continue_on_error_quoted_true_on_wheel_smoke_job": (
        _set_job_flag("wheel-smoke", "continue-on-error", "true"),
        "job-level continue-on-error is forbidden",
    ),
    "continue_on_error_on_docs_live": (
        _set_step_flag(
            "docs-live",
            "Require the live documentation revision",
            "continue-on-error",
            True,
        ),
        "continue-on-error is forbidden outside the protoc installer",
    ),
    "continue_on_error_on_a_non_protoc_uses_step": (
        _set_step_flag(
            "wheel-smoke", "actions/download-artifact", "continue-on-error", True
        ),
        "continue-on-error is forbidden outside the protoc installer",
    ),
    "or_true_on_the_floor_check": (
        _mutate_run(
            "pypi-build",
            "Build wheels inside the manylinux 2_28 container",
            lambda run: run.replace(
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28",
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28 || true",
            ),
        ),
        "step text drifted from its pinned hash",
    ),
    "or_exit_zero_on_the_floor_check": (
        _mutate_run(
            "pypi-build",
            "Build wheels inside the manylinux 2_28 container",
            lambda run: run.replace(
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28",
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28||exit 0",
            ),
        ),
        "step text drifted from its pinned hash",
    ),
    "set_plus_e_in_the_venv_smoke": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: "set +e\n" + run,
        ),
        "step text drifted from its pinned hash",
    ),
    "trap_exit_zero_in_the_venv_smoke": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: run + "\ntrap 'exit 0' EXIT\n",
        ),
        "step text drifted from its pinned hash",
    ),
    "tag_lookup_or_exit_zero": (
        _mutate_run(
            "docs-live",
            "Resolve the tag commit",
            lambda run: run.replace(
                'test -n "$TAG_SHA"', 'test -n "$TAG_SHA" || exit 0'
            ),
        ),
        "step text drifted from its pinned hash",
    ),
    "removing_the_floor_check": (
        _mutate_run(
            "pypi-build",
            "Build wheels inside the manylinux 2_28 container",
            lambda run: run.replace(
                "check-wheel-glibc-floor.py target/wheels/*.whl 2.28", ":"
            ),
        ),
        "must run the floor check",
    ),
    "arm_wheel_build_dropped": (
        _drop_matrix_entry("pypi-build", "aarch64-unknown-linux-gnu"),
        "matrix entries must be pinned exactly",
    ),
    "arm_smoke_on_an_x86_runner": (
        _retarget_matrix_os(
            "wheel-smoke", "artifact", "wheel-aarch64-unknown-linux-gnu", "ubuntu-24.04"
        ),
        "matrix entries must be pinned exactly",
    ),
    "if_false_on_the_runtime_floor_smoke": (
        _set_step_flag(
            "wheel-smoke",
            "Run the wheel inside the manylinux 2_28 container",
            "if",
            False,
        ),
        "is not in the allowlist",
    ),
    "if_template_false_on_the_runtime_floor_smoke": (
        _set_step_flag(
            "wheel-smoke",
            "Run the wheel inside the manylinux 2_28 container",
            "if",
            "${{ false }}",
        ),
        "is not in the allowlist",
    ),
    "if_template_false_and_true_on_docs_live_revision": (
        _set_step_flag(
            "docs-live",
            "Require the live documentation revision",
            "if",
            "${{ false && true }}",
        ),
        "is not in the allowlist",
    ),
    "custom_shell_template_on_the_venv_smoke": (
        _set_step_flag("wheel-smoke", "Smoke the installed wheel", "shell", "bash {0}"),
        "custom shell",
    ),
    "docs_live_timeout_exit_zero": (
        _mutate_run(
            "docs-live",
            "Require the live documentation revision",
            lambda run: run.replace("exit 1", "exit 0"),
        ),
        "revision timeout branch must end in exit 1",
    ),
    "not_cancelled_on_pypi_publish": (
        _set_job_flag(
            "pypi-publish", "if", "github.event_name == 'push' && !cancelled()"
        ),
        "job if must be exactly",
    ),
    "deleting_the_return_1_after_the_panic_message": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: re.sub(
                r'(echo "panic or crash \(exit \$status\) from: biomcp \$\*" >&2)\s*\n\s*return 1',
                r"\1\n              :",
                run,
            ),
        ),
        "panic guard must return failure",
    ),
    "deleting_the_not_found_exit_check": (
        _mutate_run(
            "wheel-smoke",
            "Smoke the installed wheel",
            lambda run: run.replace('grep -q "Drug not found in FAERS"', ":"),
        ),
        "not-found exit check must follow",
    ),
    "restoring_a_release_published_trigger": (
        lambda parsed: parsed[True].update({"release": {"types": ["published"]}}),
        "trigger block must be tag pushes only",
    ),
    "no_space_template_true_on_a_smoke_step_if": (
        lambda parsed: _step_by_name(
            parsed, "wheel-smoke", "Smoke the installed wheel"
        ).update({"if": "${{true}}"}),
        "is not in the allowlist",
    ),
    "double_quoted_trap_exit_zero": (
        lambda parsed: _step_by_name(
            parsed, "wheel-smoke", "Smoke the installed wheel"
        ).update(
            {
                "run": 'trap "exit 0" EXIT\n'
                + _step_by_name(parsed, "wheel-smoke", "Smoke the installed wheel")[
                    "run"
                ]
            }
        ),
        "step text drifted from its pinned hash",
    ),
}


@pytest.mark.parametrize("name", sorted(PIPELINE_MUTATIONS))
def test_pipeline_mutations_break_the_contract(name: str) -> None:
    mutate, expected_message = PIPELINE_MUTATIONS[name]
    mutated = _mutated_pipeline(mutate)
    with pytest.raises(AssertionError, match=re.escape(expected_message)):
        _assert_pipeline_contract(mutated)
        _assert_publish_gating(mutated)
        _assert_no_defaults_shell(mutated)
        _assert_step_hashes(mutated)


@pytest.mark.parametrize(
    "job,edge", [(job, edge) for job, edges in EXPECTED_NEEDS.items() for edge in edges]
)
def test_removing_each_needs_edge_breaks_the_contract(
    tmp_path: Path, job: str, edge: str
) -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    block = _job_block(workflow, job)
    mutated = (
        block.replace(edge + ", ", "", 1)
        .replace(", " + edge, "", 1)
        .replace("[" + edge + "]", "[]", 1)
    )
    assert mutated != block
    scratch = tmp_path / "release.yml"
    scratch.write_text(workflow.replace(block, mutated, 1), encoding="utf-8")
    with pytest.raises(AssertionError):
        _assert_release_contract(scratch.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "needle,replacement",
    [
        (
            'run: python3 scripts/check-release-versions.py --tag "$TAG"',
            'run: python3 scripts/check-release-versions.py --tag "$TAG" || true',
        ),
        (
            'run: python3 scripts/check-release-versions.py --tag "$TAG"',
            'run: python3 scripts/check-release-versions.py --tag "$TAG" || :',
        ),
        (
            'run: python3 scripts/check-changelog-coverage.py --tag "$TAG"',
            'run: python3 scripts/check-changelog-coverage.py --tag "$TAG"; exit 0',
        ),
        (
            "      - name: Require the changelog to cover merged tickets\n",
            "      - name: Require the changelog to cover merged tickets\n        if: ${{ false }}\n",
        ),
        ("    steps:\n", "    continue-on-error: true\n    steps:\n"),
        (
            "      - name: Require the tag and committed versions to agree\n",
            "      - name: Require the tag and committed versions to agree\n        if: false\n",
        ),
    ],
)
def test_neutering_a_gate_breaks_the_contract(
    tmp_path: Path, needle: str, replacement: str
) -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert needle in workflow
    scratch = tmp_path / "release.yml"
    scratch.write_text(workflow.replace(needle, replacement, 1), encoding="utf-8")
    with pytest.raises(AssertionError):
        _assert_release_contract(scratch.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "clause",
    [
        "success()",
        "inputs.container_only == true",
        "needs.version-check.result == 'success'",
        "needs.docs-live.result == 'success'",
        "needs.create-draft.result == 'skipped'",
        "needs.build.result == 'skipped'",
        "needs['wheel-smoke'].result == 'skipped'",
    ],
)
def test_removing_a_container_condition_clause_breaks_the_contract(
    tmp_path: Path, clause: str
) -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    scratch = tmp_path / "release.yml"
    scratch.write_text(workflow.replace(clause, "true", 1), encoding="utf-8")
    with pytest.raises(AssertionError):
        _assert_release_contract(scratch.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "needle,replacement",
    [
        (
            "            container: quay.io/pypa/manylinux_2_28_x86_64\n",
            "            container: quay.io/pypa/manylinux_2_28_x86_64-alpine\n",
        ),
        (
            "- os: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu",
            "- os: ubuntu-24.04\n            target: aarch64-unknown-linux-gnu",
        ),
        (
            "https://static.rust-lang.org/rustup/archive/1.28.2/${{ matrix.target }}/rustup-init\n",
            "https://static.rust-lang.org/rustup/archive/1.27.1/${{ matrix.target }}/rustup-init\n",
        ),
        (
            "            sha256sum -c rustup-init.sha256\n",
            "            true\n",
        ),
        (
            "--compatibility manylinux_2_28",
            "",
        ),
        (
            '                *) echo "wheel must end in $tag.whl: $wheel" >&2; exit 1 ;;',
            "                :",
        ),
        (
            "scripts/check-wheel-glibc-floor.py --elf target/${{ matrix.target }}/release/biomcp 2.28",
            "true",
        ),
        (
            '          stats="$(/opt/python/cp312-cp312/bin/biomcp cache stats --json)"',
            '          stats="$(/opt/python/cp312-cp312/bin/biomcp --version)"',
        ),
    ],
)
def test_neutering_the_linux_floor_breaks_the_contract(
    tmp_path: Path, needle: str, replacement: str
) -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert needle in workflow, needle
    scratch = tmp_path / "release.yml"
    scratch.write_text(workflow.replace(needle, replacement), encoding="utf-8")
    with pytest.raises(AssertionError):
        _assert_linux_floor_contract(scratch.read_text(encoding="utf-8"))


def test_restoring_the_unpinned_rustup_pipe_breaks_the_contract(tmp_path: Path) -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    pinned = (
        '            curl --proto "=https" --tlsv1.2 -sSfO '
        "https://static.rust-lang.org/rustup/archive/1.28.2/${{ matrix.target }}/rustup-init\n"
        '            curl --proto "=https" --tlsv1.2 -sSfO '
        "https://static.rust-lang.org/rustup/archive/1.28.2/${{ matrix.target }}/rustup-init.sha256\n"
        "            sha256sum -c rustup-init.sha256\n"
        "            chmod +x rustup-init\n"
        "            ./rustup-init -y --default-toolchain 1.93.1 --profile minimal"
    )
    assert pinned in workflow
    scratch = tmp_path / "release.yml"
    scratch.write_text(
        workflow.replace(
            pinned,
            '            curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs '
            "| sh -s -- -y --default-toolchain 1.93.1 --profile minimal",
        ),
        encoding="utf-8",
    )
    with pytest.raises(AssertionError):
        _assert_linux_floor_contract(scratch.read_text(encoding="utf-8"))


def test_actions_are_pinned_and_pypi_uses_trusted_publishing() -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert not re.findall(r"uses: [^\s]+@(?![0-9a-f]{40}\b)[^\s]+", workflow)
    pypi = _job_block(workflow, "pypi-publish")
    assert "environment: pypi" in pypi and "id-token: write" in pypi
    assert "uv publish --trusted-publishing always dist/*" in pypi


def test_no_other_workflow_exposes_release_publication() -> None:
    routes = (
        "gh release create",
        "uv publish",
        "skopeo copy",
        "git push",
        "docker push",
        "imagetools create",
    )
    for path in WORKFLOWS.glob("*.yml"):
        if path != RELEASE_WORKFLOW:
            text = path.read_text(encoding="utf-8")
            assert not any(route in text for route in routes), path.name
