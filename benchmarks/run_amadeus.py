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


class AmadeusPolicy:
    """An intercode policy backed by the Amadeus HTTP ``/chat`` endpoint."""

    def summarize_obs(self, agent_obs) -> str:
        """Render intercode's AGENT_OBS dict (possibly rpyc netrefs) as a string.

        AGENT_OBS maps each test string to ``{"output": str, "error": str}``. rpyc
        returns netref objects that are not JSON-serializable, so coerce to text.
        """
        try:
            items = list(agent_obs.items())
        except AttributeError:
            return str(agent_obs)
        if not items:
            return "No test output"
        lines = []
        for test, res in items:
            err = ""
            if isinstance(res, dict):
                err = str(res.get("error", "") or "")
            else:
                err = str(res)
            mark = "FAIL" if err else "PASS"
            lines.append(f"  [{mark}] {test}")
        return "\n".join(lines)

    def __init__(self, base_url: str, timeout: float = 180.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.query = None

    def reset(self):
        self.query = None

    def _chat(self, message: str) -> str:
        url = f"{self.base_url}/chat"
        body = json.dumps({"message": message}).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                data = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Amadeus /chat HTTP {exc.code}: {detail}") from exc
        return data.get("content", "") or ""

    def forward(self, query, observation, reward, available_actions):
        if observation is None:
            user = (
                f"{INIT_MSG}\n\nQuery: \"{query}\"\n\n"
                "Write the Python function. Respond with a single ```python block "
                "that defines the function."
            )
        else:
            obs_str = observation if isinstance(observation, str) else str(observation)
            user = (
                f"Previous attempt result for query \"{self.query}\":\n"
                f"Output: {obs_str}\n"
                f"Reward: {reward}\n\n"
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
    observation, reward = None, None
    history = {"actions": [], "observations": [], "rewards": [], "raw_replies": []}

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
                observation = policy.summarize_obs(info.get(AGENT_OBS, {}))
                # Stop early on a perfect score.
                if reward >= 1.0:
                    history["rewards"].append(reward)
                    break
            else:
                _, reward, _, info = env.step(action)
                observation = policy.summarize_obs(info.get(AGENT_OBS, {}))
                if not observation or observation == "No test output":
                    observation = str(info.get("observation", action))
        except Exception as exc:  # noqa: BLE001
            observation = f"Environment error: {exc}"
            reward = 0
        history["rewards"].append(reward if reward is not None else 0.0)

    final_reward = max(history["rewards"]) if history["rewards"] else 0.0
    return {
        "task_id": record.get("task_id"),
        "query": env.query,
        "reward": final_reward,
        "turns": len(history["actions"]),
        "history": history,
    }


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
    per_problem = []
    try:
        for idx in indices:
            print(f"--- problem {idx} ---", flush=True)
            t0 = time.time()
            result = run_problem(env, policy, idx, cli.max_turns)
            result["duration_s"] = round(time.time() - t0, 2)
            print(
                f"task_id={result['task_id']} reward={result['reward']} "
                f"turns={result['turns']} time={result['duration_s']}s",
                flush=True,
            )
            per_problem.append(result)
            with open(os.path.join(out_dir, "per_problem.jsonl"), "a") as fh:
                fh.write(json.dumps(result) + "\n")
    finally:
        env.close()

    rewards = [r["reward"] for r in per_problem]
    summary = {
        "run_id": run_id,
        "timestamp": now_iso(),
        "amadeus_url": cli.amadeus_url,
        "data_path": cli.data_path,
        "image_name": cli.image_name,
        "n_problems": len(per_problem),
        "mean_reward": round(sum(rewards) / len(rewards), 4) if rewards else 0,
        "solved": sum(1 for r in rewards if r >= 1.0),
        "results": per_problem,
    }
    with open(os.path.join(out_dir, "summary.json"), "w") as fh:
        json.dump(summary, fh, indent=2)
    print(f"\nmean_reward={summary['mean_reward']} solved={summary['solved']}/{len(per_problem)}")
    print(f"results written to {out_dir}")


if __name__ == "__main__":
    main()
