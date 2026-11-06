"""Prepare a standalone Sevi commit without changing the current worktree/index.

Used only for this checkout's initial Rust/ export. In a standalone clone, use
normal git add/commit/push instead. Outputs a local sevi-main branch for review.
"""
from pathlib import Path
import os
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]


def run(args, cwd, env=None, text=True):
    return subprocess.check_output(args, cwd=cwd, env=env, text=text).strip()


def main():
    repository = Path(run(["git", "rev-parse", "--show-toplevel"], ROOT)).resolve()
    if repository == ROOT or ROOT.parent != repository or ROOT.name != "Rust":
        raise SystemExit("This export is only for the original checkout's Rust/ folder. Use normal Git in the standalone repository.")
    scratch = ROOT.joinpath("target/github-export")
    scratch.mkdir(parents=True, exist_ok=True)
    index = scratch.joinpath("index-" + uuid.uuid4().hex).resolve()
    assert index.is_relative_to(repository)
    environment = dict(os.environ, GIT_INDEX_FILE=str(index))
    subprocess.run(["git", "read-tree", "--empty"], cwd=repository, env=environment, check=True)
    subprocess.run(["git", "add", "-A", "--", "Rust"], cwd=repository, env=environment, check=True)
    tree = run(["git", "write-tree"], repository, environment)
    subtree = run(["git", "rev-parse", tree + ":Rust"], repository)
    files = run(["git", "ls-tree", "-r", "--name-only", subtree], repository).splitlines()
    for name in files:
        parts = Path(name).parts
        if "target" in parts or ".secrets" in parts or name.endswith((".pem", ".key", ".p12", ".p8")):
            raise SystemExit("Unexpected generated/private file in export: " + name)
    expected = {"Cargo.toml", "Cargo.lock", "LICENSE", ".github/workflows/check.yml", ".github/workflows/release.yml"}
    if not expected.issubset(files): raise SystemExit("Standalone export is missing required files")
    parent = subprocess.run(["git", "rev-parse", "--verify", "refs/heads/sevi-main"], cwd=repository, capture_output=True, text=True)
    args = ["git", "commit-tree", subtree]
    if parent.returncode == 0:
        if run(["git", "rev-parse", parent.stdout.strip() + "^{tree}"], repository) == subtree:
            print("sevi-main already matches the Rust workspace")
            return
        args.extend(["-p", parent.stdout.strip()])
    commit = subprocess.check_output(args, cwd=repository, input="Prepare standalone Sevi Rust port and signed GitHub updates\n", text=True).strip()
    subprocess.run(["git", "update-ref", "refs/heads/sevi-main", commit, parent.stdout.strip() if parent.returncode == 0 else "0" * 40], cwd=repository, check=True)
    scratch.joinpath("files.txt").write_text("\n".join(files) + "\n")
    print("Prepared sevi-main:", commit, "with", len(files), "files at repository root")
    print("Push with: git push sevi sevi-main:main")


if __name__ == "__main__": main()
