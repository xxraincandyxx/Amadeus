#!/usr/bin/env python3
"""fs-coding mini-suite: the Amadeus agent fixes single-file Python bugs.

Unlike the intercode/MBPP harness (which tests the backbone by emitting one
function), this exercises the *agent*: each task is a tiny working directory
with a buggy ``solution.py`` and a failing ``test_solution.py``. A fresh Amadeus
agent is given the paths and must use its native read/edit/bash tools to locate
the bug, edit the file, and verify by running the tests. Scoring is objective:
``test_solution.py`` exits 0 after the agent's turn.

Run against the Amadeus HTTP server (start it with --llm-trace so every tool
call and LLM turn is recorded for diagnosis).
"""
# @amadeus-header
# summary: Runs the Amadeus agent on the fs-coding bug-fix mini-suite over HTTP.
# layer: benchmark
# status: active
# feature_flags: none
# provides:
# - cmd: fs_bench/run_fs_bench.py
# uses:
# - module: fs_bench.tasks.TASKS
# - route: POST /agents, POST /agents/:id/chat, DELETE /agents/:id (Amadeus API)
# - runtime: python3 (for test execution)
# - artifact: runs/<run_id>/{summary.json, per_task.jsonl}
# invariants:
# - A task is solved iff its test_solution.py exits 0 after the agent's turn.
# - Each task gets a fresh working directory and a fresh agent (no history leak).
# side_effects:
# - Creates per-task working directories with buggy source + tests.
# - Sends HTTP requests to the Amadeus server; creates/deletes agents.
# - Runs python3 to score tests.
# tests:
# - cmd: python fs_bench/run_fs_bench.py --smoke
# @end-amadeus-header

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from tasks import TASKS  # noqa: E402

PY = sys.executable


def now_iso():
    return datetime.now(timezone.utc).isoformat()


def run_tests(task_dir):
    """Run test_solution.py in task_dir. Returns (passed: bool, output: str)."""
    try:
        r = subprocess.run(
            [PY, "test_solution.py"],
            cwd=task_dir,
            capture_output=True,
            text=True,
            timeout=30,
        )
        return r.returncode == 0, (r.stdout + r.stderr).strip()
    except subprocess.TimeoutExpired:
        return False, "test timed out"


class AmadeusClient:
    """Minimal HTTP client for /agents with retry (mirrors run_amadeus)."""

    def __init__(self, base_url, timeout=180.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.agent_id = None

    def _req(self, req, tries=5):
        last = None
        for attempt in range(tries):
            try:
                with urllib.request.urlopen(req, timeout=self.timeout + 30) as resp:
                    return json.loads(resp.read().decode("utf-8"))
            except Exception as exc:  # noqa: BLE001
                last = exc
                time.sleep(3 + attempt * 5)
        raise RuntimeError(f"request failed after {tries} tries: {last}")

    def start(self, tag):
        body = json.dumps({"name": tag, "profile": "default"}).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}/agents", data=body,
            headers={"Content-Type": "application/json"}, method="POST",
        )
        self.agent_id = self._req(req)["agent"]["id"]

    def stop(self):
        if not self.agent_id:
            return
        aid, self.agent_id = self.agent_id, None
        req = urllib.request.Request(
            f"{self.base_url}/agents/{aid}", data=b"{}",
            headers={"Content-Type": "application/json"}, method="DELETE",
        )
        try:
            urllib.request.urlopen(req, timeout=self.timeout).read()
        except Exception:  # noqa: BLE001
            pass

    def chat(self, message):
        body = json.dumps(
            {"message": message, "timeout_secs": int(self.timeout)}
        ).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}/agents/{self.agent_id}/chat", data=body,
            headers={"Content-Type": "application/json"}, method="POST",
        )
        return self._req(req).get("content", "") or ""


def run_task(client, task, work_root, max_turns_note=""):
    name = task["name"]
    task_dir = os.path.join(work_root, name)
    os.makedirs(task_dir, exist_ok=True)
    with open(os.path.join(task_dir, "solution.py"), "w") as fh:
        fh.write(task["solution"])
    with open(os.path.join(task_dir, "test_solution.py"), "w") as fh:
        fh.write(task["tests"])

    passed_before, out_before = run_tests(task_dir)
    sol_path = os.path.join(task_dir, "solution.py")
    test_path = os.path.join(task_dir, "test_solution.py")

    prompt = (
        f"You are fixing a bug in a Python module at:\n  {sol_path}\n"
        f"Its failing tests are at:\n  {test_path}\n\n"
        f"The tests currently FAIL with this output:\n"
        f"---\n{out_before}\n---\n\n"
        "Steps:\n"
        "1. Read both files.\n"
        "2. Find the bug in solution.py.\n"
        "3. Edit solution.py to fix it (minimal change).\n"
        "4. Run the tests to verify: "
        f"`cd {task_dir} && {PY} test_solution.py`.\n"
        "5. Iterate until the tests print OK. Use your file and shell tools.\n\n"
        "Reply with a one-line summary when the tests pass."
    )

    client.start(tag=name)
    reply = ""
    err = None
    try:
        reply = client.chat(prompt)
    except Exception as exc:  # noqa: BLE001
        err = str(exc)
    finally:
        client.stop()

    passed_after, out_after = run_tests(task_dir)
    return {
        "name": name,
        "blurb": task["blurb"],
        "passed_before": passed_before,
        "passed_after": passed_after,
        "solved": bool(passed_after and not passed_before),
        "reply": reply[:500],
        "test_output_after": out_after[:400],
        "error": err,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--amadeus_url", default="http://127.0.0.1:3000")
    p.add_argument("--runs_dir", default=os.path.join(HERE, "runs"))
    p.add_argument("--smoke", action="store_true", help="run only the first 2 tasks")
    p.add_argument("--timeout", type=float, default=180.0)
    cli = p.parse_args()

    tasks = TASKS[:2] if cli.smoke else TASKS
    run_id = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    work_root = os.path.join(cli.runs_dir, run_id)
    os.makedirs(work_root, exist_ok=True)
    summary_path = os.path.join(work_root, "summary.json")
    per_task_path = os.path.join(work_root, "per_task.jsonl")

    client = AmadeusClient(cli.amadeus_url, timeout=cli.timeout)
    results = []
    print(f"fs-coding mini-suite: {len(tasks)} tasks  (run {run_id})", flush=True)
    for i, task in enumerate(tasks, start=1):
        t0 = time.time()
        try:
            res = run_task(client, task, work_root)
        except Exception as exc:  # noqa: BLE001
            res = {"name": task["name"], "solved": False, "error": str(exc),
                   "passed_before": None, "passed_after": None, "reply": "", "test_output_after": "", "blurb": task["blurb"]}
        res["duration_s"] = round(time.time() - t0, 2)
        results.append(res)
        with open(per_task_path, "a") as fh:
            fh.write(json.dumps(res) + "\n")
        solved = sum(1 for r in results if r["solved"])
        mark = "SOLVED" if res["solved"] else "FAILED"
        print(
            f"[{i}/{len(tasks)}] {mark:6} {res['name']} "
            f"({res['duration_s']}s) | running {solved}/{i}",
            flush=True,
        )
        json.dump({"run_id": run_id, "done": i, "total": len(tasks),
                   "solved": solved, "results": results}, open(summary_path, "w"), indent=2)

    solved = sum(1 for r in results if r["solved"])
    print(f"\nDONE: solved {solved}/{len(results)} = {100*solved/len(results):.1f}%")
    print(f"results: {work_root}")


if __name__ == "__main__":
    main()
