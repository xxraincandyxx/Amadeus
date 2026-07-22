#!/usr/bin/env python3
"""Intercode (python/mbpp) evaluation harness for the Amadeus agent.

Reuses intercode's canonical ``PythonEnv`` (docker container ``intercode-python``,
rpyc execution server, ``get_reward_mbpp`` scorer) and substitutes the Amadeus
agent as the decision-making policy via the HTTP ``/chat`` endpoint.

Per problem the harness runs intercode's standard n-turn loop:
  reset(idx) -> for each turn: policy.forward(query, obs, reward, actions)
                              -> if action starts with ``def``: step(def) then
                                 auto ``submit <func>`` to compute the mbpp reward
                                 else: step(action) and feed observation back.

Every Amadeus turn is traced server-side (start the server with ``--llm-trace``)
so the resulting ``llm_trace_*.jsonl`` records exactly what the model saw and
said on each turn for post-hoc diagnosis.
"""
# @amadeus-header
# summary: Drives the Amadeus agent through intercode's canonical python/mbpp docker eval.
# layer: benchmark
# status: active
# feature_flags: none
# provides:
# - cmd: benchmarks/run_amadeus.py
# - type: AmadeusPolicy
# uses:
# - module: intercode.envs.PythonEnv
# - route: POST /chat (Amadeus HTTP API)
# - artifact: results/<run_id>/{summary.json, per_problem.jsonl}
# invariants:
# - Scoring uses intercode's get_reward_mbpp inside the canonical intercode-python docker container.
# - The Amadeus server must be running with --llm-trace for full per-turn diagnosis.
# side_effects:
# - Starts and stops intercode docker containers.
# - Sends HTTP requests to the Amadeus server.
# - Writes results under benchmarks/results/<run_id>/.
# tests:
# - cmd: python benchmarks/run_amadeus.py --problems 5
# @end-amadeus-header

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
INTERCODE = os.path.join(HERE, "intercode")
sys.path.insert(0, INTERCODE)

from intercode.envs import PythonEnv, AGENT_OBS  # noqa: E402

INIT_MSG = """## TASK DESCRIPTION
You are a Python code generator. You will be given a natural-language programming
query and must write Python 3 code to satisfy it. You interact with a Python 3
Interpreter by submitting a single Python code block each turn.

## RESPONSE FORMAT
Respond with EXACTLY ONE fenced code block and nothing else, formatted as:
```python
<your python code here>
```
Do NOT execute code, do NOT explain, do NOT add prose outside the block.

## HOW YOU ARE SCORED
For MBPP problems, define the requested function with a `def` statement. The
harness extracts your function, runs the hidden unit tests, and reports the
fraction that pass as a reward in [0, 1]. A reward of 1 means every test passed.
If a turn fails, you will see the output and reward and may try again.
"""


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def parse_python_action(reply: str):
    """Extract the last ```python``` block from the reply.

    Returns (action, is_code). Falls back to the raw text if no block is found.
    """
    blocks = re.findall(r"```(?:python)?\s*\n(.*?)```", reply, re.DOTALL)
    if blocks:
        return blocks[-1].strip(), True
    stripped = reply.strip()
    return stripped, bool(stripped)


def extract_signature(gold: str):
    """Pull the canonical `def name(params):` line from the gold solution.

    Standard MBPP gives the model this signature; without it the model guesses
    the function name and arity, which is the dominant failure mode here.
    Returns the def line (normalized) or None if the gold has no def.
    """
    match = re.search(r"(def\s+\w+\s*\([^)]*\)\s*:)", gold)
    if not match:
        return None
    # Normalize "def name (params)" -> "def name(params)".
    return re.sub(r"(\w)\s+\(", r"\1(", match.group(1))


class AmadeusPolicy:
    """An intercode policy backed by the Amadeus HTTP ``/chat`` endpoint."""

    def summarize_obs(self, agent_obs) -> str:
        """Render intercode's AGENT_OBS dict (possibly rpyc netrefs) as a string.

        AGENT_OBS maps each test string to ``{"output": str, "error": str}``. rpyc
        returns netref objects on which ``.get()`` is blocked (raises
        AttributeError), so we coerce via ``str()`` and never let this method
        raise — a raise here would be caught by the turn loop's outer handler and
        clobber the real reward with 0.
        """
        try:
            items = list(agent_obs.items())
        except Exception:  # noqa: BLE001
            return str(agent_obs)
        if not items:
            return "No test output"
        lines = []
        for test, res in items:
            # str() on the netref dict yields e.g. "{'output': '', 'error': '...'}";
            # a passing test has an empty error value.
            text = str(res)
            err_empty = "'error': ''" in text or '"error": ""' in text
            mark = "PASS" if err_empty else "FAIL"
            lines.append(f"  [{mark}] {test}")
        return "\n".join(lines)

    def __init__(self, base_url: str, timeout: float = 180.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.query = None
        self.agent_id = None
        self.signature = None

    def reset(self):
        self.query = None

    def _request(self, req, tries: int = 5):
        """HTTP request with retry/backoff so a transient server hiccup or
        auto-restart doesn't kill the whole run."""
        last = None
        for attempt in range(tries):
            try:
                with urllib.request.urlopen(req, timeout=self.timeout + 30) as resp:
                    return json.loads(resp.read().decode("utf-8"))
            except Exception as exc:  # noqa: BLE001
                last = exc
                time.sleep(3 + attempt * 5)
        raise RuntimeError(f"request failed after {tries} tries: {last}")

    def start(self, tag: str = "intercode"):
        """Create a fresh Amadeus agent for one problem (clean history)."""
        body = json.dumps({"name": tag, "profile": "default"}).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}/agents",
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        self.agent_id = self._request(req)["agent"]["id"]

    def stop(self):
        """Delete the per-problem agent so history never leaks across problems."""
        if not self.agent_id:
            return
        aid = self.agent_id
        self.agent_id = None
        req = urllib.request.Request(
            f"{self.base_url}/agents/{aid}",
            data=b"{}",
            headers={"Content-Type": "application/json"},
            method="DELETE",
        )
        try:
            urllib.request.urlopen(req, timeout=self.timeout).read()
        except Exception:  # noqa: BLE001
            pass

    def _chat(self, message: str) -> str:
        url = f"{self.base_url}/agents/{self.agent_id}/chat"
        body = json.dumps({"message": message, "timeout_secs": int(self.timeout)}).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        data = self._request(req)
        return data.get("content", "") or ""

    def forward(self, query, observation, reward, available_actions):
        if observation is None:
            sig_clause = ""
            if self.signature:
                sig_clause = (
                    f"\n\nYou MUST use exactly this function signature (same name and "
                    f"parameters, no extras):\n```python\n{self.signature}\n```\n"
                    f"Complete the body of that function."
                )
            user = (
                f"{INIT_MSG}\n\nQuery: \"{query}\"{sig_clause}\n\n"
                "Respond with a single ```python block that defines the function."
            )
        else:
            obs_str = observation if isinstance(observation, str) else str(observation)
            sig_clause = f"\nKeep the signature `{self.signature}`." if self.signature else ""
            user = (
                f"Previous attempt result for query \"{self.query}\":\n"
                f"Output: {obs_str}\n"
                f"Reward: {reward}{sig_clause}\n\n"
                "The tests did not fully pass. Rewrite the function. Respond with a "
                "single ```python block that defines the corrected function."
            )
        reply = self._chat(user)
        action, is_code = parse_python_action(reply)
        return action, is_code, reply


def run_problem(env: PythonEnv, policy: AmadeusPolicy, idx: int, max_turns: int):
    env.reset(idx)
    record = env.data_loader.get(idx)
    policy.query = env.query
    policy.signature = extract_signature(env.gold)
    observation, reward = None, None
    history = {"actions": [], "observations": [], "rewards": [], "raw_replies": []}
    policy.start(tag=f"intercode-{record.get('task_id', idx)}")
    try:
        for turn in range(max_turns):
            try:
                action, is_code, reply = policy.forward(
                    env.query, observation, reward, env.get_available_actions()
                )
            except Exception as exc:  # noqa: BLE001
                history["actions"].append(f"<policy-error: {exc}>")
                history["rewards"].append(0.0)
                break

            history["raw_replies"].append(reply)
            if not is_code:
                history["actions"].append("<no-code>")
                history["rewards"].append(0.0)
                observation = "No valid code block found. Respond with a ```python block."
                reward = 0
                continue

            history["actions"].append(action)
            try:
                if action.lstrip().startswith("def "):
                    func_match = re.match(r"\s*def (\w+)\(", action)
                    if not func_match:
                        observation = "Could not parse function name from your `def`. Retry."
                        reward = 0
                        history["rewards"].append(0.0)
                        continue
                    func_name = func_match.group(1)
                    env.step(action)
                    _, reward, _, info = env.step(f"submit {func_name}")
                    # Capture the true reward FIRST, before any observation
                    # processing that could raise and clobber it.
                    turn_reward = reward if reward is not None else 0.0
                    history["rewards"].append(turn_reward)
                    if os.environ.get("IC_DEBUG"):
                        print(
                            f"    [debug] task={record.get('task_id')} submit={func_name} "
                            f"reward={turn_reward}",
                            flush=True,
                        )
                    try:
                        observation = policy.summarize_obs(info.get(AGENT_OBS, {}))
                    except Exception:  # noqa: BLE001
                        observation = ""
                    # Stop early on a perfect score.
                    if turn_reward >= 1.0:
                        break
                    # Reward already recorded above; skip the shared append below.
                    continue
                else:
                    _, reward, _, info = env.step(action)
                    observation = policy.summarize_obs(info.get(AGENT_OBS, {}))
                    if not observation or observation == "No test output":
                        observation = str(info.get("observation", action))
            except Exception as exc:  # noqa: BLE001
                observation = f"Environment error: {exc}"
                reward = 0
            history["rewards"].append(reward if reward is not None else 0.0)
    finally:
        policy.stop()

    final_reward = max(history["rewards"]) if history["rewards"] else 0.0
    return {
        "task_id": record.get("task_id"),
        "query": env.query,
        "reward": final_reward,
        "turns": len(history["actions"]),
        "history": history,
    }


def bounce_server(amadeus_url: str) -> bool:
    """Kill the Amadeus server listener so the auto-restart wrapper re-spawns it.

    Used when sustained 'result expired' / streaming errors indicate the server's
    in-process state (session bridge / connection pool) has degraded. Killing only
    the port listener leaves the wrapper bash loop alive to restart. Returns True
    once /health responds again.
    """
    from urllib.parse import urlparse

    port = urlparse(amadeus_url).port or 3000
    print(f"  [bounce] restarting amadeus server on port {port}", flush=True)
    os.system(f"lsof -ti tcp:{port} | xargs kill 2>/dev/null")
    for _ in range(60):
        time.sleep(2)
        try:
            with urllib.request.urlopen(f"{amadeus_url}/health", timeout=3) as resp:
                if resp.status == 200:
                    print("  [bounce] server healthy again", flush=True)
                    return True
        except Exception:  # noqa: BLE001
            pass
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--data_path",
        default=os.path.join(INTERCODE, "data/python/mbpp/ic_mbpp.json"),
    )
    parser.add_argument("--image_name", default="intercode-python")
    parser.add_argument("--amadeus_url", default="http://127.0.0.1:3000")
    parser.add_argument("--problems", type=int, default=5, help="number of problems")
    parser.add_argument("--start", type=int, default=0, help="starting problem index")
    parser.add_argument("--max_turns", type=int, default=4)
    parser.add_argument("--results_dir", default=os.path.join(HERE, "results"))
    cli = parser.parse_args()

    run_id = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    out_dir = os.path.join(cli.results_dir, run_id)
    os.makedirs(out_dir, exist_ok=True)

    env = PythonEnv(
        image_name=cli.image_name,
        data_path=cli.data_path,
        is_agent=True,
        verbosity=False,
    )
    policy = AmadeusPolicy(base_url=cli.amadeus_url)

    indices = list(range(cli.start, cli.start + cli.problems))
    summary_path = os.path.join(out_dir, "summary.json")
    per_problem_path = os.path.join(out_dir, "per_problem.jsonl")

    def write_summary(done, total):
        rewards = [r["reward"] for r in per_problem]
        summary = {
            "run_id": run_id,
            "timestamp": now_iso(),
            "amadeus_url": cli.amadeus_url,
            "data_path": cli.data_path,
            "image_name": cli.image_name,
            "max_turns": cli.max_turns,
            "done": done,
            "total": total,
            "n_problems": len(per_problem),
            "mean_reward": round(sum(rewards) / len(rewards), 4) if rewards else 0,
            "solved": sum(1 for r in rewards if r >= 1.0),
            # Compact per-problem view; full detail lives in per_problem.jsonl.
            "rewards": [r["reward"] for r in per_problem],
        }
        with open(summary_path, "w") as fh:
            json.dump(summary, fh, indent=2)

    per_problem = []
    consec_errors = 0
    try:
        for done, idx in enumerate(indices, start=1):
            t0 = time.time()
            try:
                result = run_problem(env, policy, idx, cli.max_turns)
            except Exception as exc:  # noqa: BLE001
                # Never let one problem (or a server blip) abort the whole run.
                result = {
                    "task_id": idx,
                    "query": "",
                    "reward": 0.0,
                    "turns": 0,
                    "history": {"actions": [f"<run-error: {exc}>"], "observations": [], "rewards": [0.0], "raw_replies": []},
                }
                policy.agent_id = None
            result["duration_s"] = round(time.time() - t0, 2)
            per_problem.append(result)
            with open(per_problem_path, "a") as fh:
                fh.write(json.dumps(result) + "\n")
            # Self-heal: if the server's streaming path has degraded (sustained
            # run-errors), bounce it so the wrapper re-spawns a fresh one.
            if result["turns"] == 0:
                consec_errors += 1
                if consec_errors >= 5:
                    bounce_server(cli.amadeus_url)
                    consec_errors = 0
            else:
                consec_errors = 0
            rewards_so_far = [r["reward"] for r in per_problem]
            solved_so_far = sum(1 for r in rewards_so_far if r >= 1.0)
            print(
                f"[{done}/{len(indices)}] idx={idx} task={result['task_id']} "
                f"reward={result['reward']} turns={result['turns']} "
                f"time={result['duration_s']}s | "
                f"running solved={solved_so_far}/{done} "
                f"mean={sum(rewards_so_far)/len(rewards_so_far):.3f}",
                flush=True,
            )
            write_summary(done, len(indices))
    finally:
        env.close()

    rewards = [r["reward"] for r in per_problem]
    print(
        f"\nDONE: mean_reward={sum(rewards)/len(rewards):.4f} "
        f"solved={sum(1 for r in rewards if r >= 1.0)}/{len(per_problem)}"
    )
    print(f"results written to {out_dir}")


if __name__ == "__main__":
    main()
