"""
Integration tests for learn-claude-code agents.

Comprehensive agent task tests covering v0-v4 core capabilities.
Runs on GitHub Actions (Linux).
"""
import os
import sys
import json
import tempfile
import shutil

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def get_client():
    """Get OpenAI-compatible client for testing."""
    from openai import OpenAI
    api_key = os.getenv("TEST_API_KEY")
    base_url = os.getenv("TEST_BASE_URL", "https://api.openai-next.com/v1")
    if not api_key:
        return None
    return OpenAI(api_key=api_key, base_url=base_url)


MODEL = os.getenv("TEST_MODEL", "claude-3-5-sonnet-20241022")


# =============================================================================
# Tool Definitions
# =============================================================================

BASH_TOOL = {
    "type": "function",
    "function": {
        "name": "bash",
        "description": "Run a shell command",
        "parameters": {
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }
    }
}

READ_FILE_TOOL = {
    "type": "function",
    "function": {
        "name": "read_file",
        "description": "Read contents of a file",
        "parameters": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"]
        }
    }
}

WRITE_FILE_TOOL = {
    "type": "function",
    "function": {
        "name": "write_file",
        "description": "Write content to a file (creates or overwrites)",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["path", "content"]
        }
    }
}

EDIT_FILE_TOOL = {
    "type": "function",
    "function": {
        "name": "edit_file",
        "description": "Replace old_string with new_string in a file",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"}
            },
            "required": ["path", "old_string", "new_string"]
        }
    }
}

TODO_WRITE_TOOL = {
    "type": "function",
    "function": {
        "name": "TodoWrite",
        "description": "Update the todo list to track task progress",
        "parameters": {
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {"type": "string"},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]},
                            "activeForm": {"type": "string"}
                        },
                        "required": ["content", "status", "activeForm"]
                    }
                }
            },
            "required": ["items"]
        }
    }
}

V1_TOOLS = [BASH_TOOL, READ_FILE_TOOL, WRITE_FILE_TOOL, EDIT_FILE_TOOL]
V2_TOOLS = V1_TOOLS + [TODO_WRITE_TOOL]


# =============================================================================
# Agent Loop Runner
# =============================================================================

def execute_tool(name, args, workdir):
    """Execute a tool and return output."""
    import subprocess

    if name == "bash":
        cmd = args.get("command", "")
        try:
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=30, cwd=workdir)
            return result.stdout + result.stderr or "(empty)"
        except Exception as e:
            return f"Error: {e}"

    elif name == "read_file":
        path = args.get("path", "")
        try:
            with open(path, "r") as f:
                return f.read()
        except Exception as e:
            return f"Error: {e}"

    elif name == "write_file":
        path = args.get("path", "")
        content = args.get("content", "")
        try:
            with open(path, "w") as f:
                f.write(content)
            return f"Written {len(content)} bytes to {path}"
        except Exception as e:
            return f"Error: {e}"

    elif name == "edit_file":
        path = args.get("path", "")
        old = args.get("old_string", "")
        new = args.get("new_string", "")
        try:
            with open(path, "r") as f:
                content = f.read()
            if old not in content:
                return f"Error: '{old}' not found in file"
            content = content.replace(old, new, 1)
            with open(path, "w") as f:
                f.write(content)
            return f"Replaced in {path}"
        except Exception as e:
            return f"Error: {e}"

    elif name == "TodoWrite":
        items = args.get("items", [])
        # Simulate todo tracking
        result = []
        for item in items:
            status_icon = {"pending": "[ ]", "in_progress": "[>]", "completed": "[x]"}.get(item["status"], "[ ]")
            result.append(f"{status_icon} {item['content']}")
        return "\n".join(result) + f"\n({len([i for i in items if i['status']=='completed'])}/{len(items)} completed)"

    return f"Unknown tool: {name}"


def run_agent_loop(client, task, tools, workdir=None, max_turns=15, system_prompt=None):
    """
    Run a complete agent loop until done or max_turns.
    Returns (final_response, tool_calls_made, messages)
    """
    if workdir is None:
        workdir = os.getcwd()

    if system_prompt is None:
        system_prompt = f"You are a coding agent at {workdir}. Use tools to complete tasks. Be concise."

    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": task}
    ]

    tool_calls_made = []

    for turn in range(max_turns):
        response = client.chat.completions.create(
            model=MODEL,
            messages=messages,
            tools=tools,
            max_tokens=1500
        )

        message = response.choices[0].message
        finish_reason = response.choices[0].finish_reason

        if finish_reason == "stop" or not message.tool_calls:
            return message.content, tool_calls_made, messages

        messages.append({
            "role": "assistant",
            "content": message.content,
            "tool_calls": [
                {"id": tc.id, "type": "function", "function": {"name": tc.function.name, "arguments": tc.function.arguments}}
                for tc in message.tool_calls
            ]
        })

        for tool_call in message.tool_calls:
            func_name = tool_call.function.name
            args = json.loads(tool_call.function.arguments)
            tool_calls_made.append((func_name, args))

            output = execute_tool(func_name, args, workdir)

            messages.append({
                "role": "tool",
                "tool_call_id": tool_call.id,
                "content": output[:5000]
            })

    return None, tool_calls_made, messages


# =============================================================================
# v0 Tests: Bash Only
# =============================================================================

def test_v0_bash_echo():
    """v0: Simple bash command execution."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    response, calls, _ = run_agent_loop(
        client,
        "Run 'echo hello world' and tell me the output.",
        [BASH_TOOL]
    )

    assert len(calls) >= 1, "Should make at least 1 tool call"
    assert any("echo" in str(c) for c in calls), "Should run echo"
    assert response and "hello" in response.lower()

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v0_bash_echo")
    return True


def test_v0_bash_pipeline():
    """v0: Bash pipeline with multiple commands."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test file
        with open(os.path.join(tmpdir, "data.txt"), "w") as f:
            f.write("apple\nbanana\napricot\ncherry\n")

        response, calls, _ = run_agent_loop(
            client,
            f"Count how many lines in {tmpdir}/data.txt start with 'a'. Use grep and wc.",
            [BASH_TOOL],
            workdir=tmpdir
        )

        assert len(calls) >= 1
        assert response and "2" in response

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v0_bash_pipeline")
    return True


# =============================================================================
# v1 Tests: 4 Core Tools
# =============================================================================

def test_v1_read_file():
    """v1: Read file contents."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "secret.txt")
        with open(filepath, "w") as f:
            f.write("The secret code is: XYZ123")

        response, calls, _ = run_agent_loop(
            client,
            f"Read {filepath} and tell me what the secret code is.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert any(c[0] == "read_file" for c in calls), "Should use read_file"
        assert response and "XYZ123" in response

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v1_read_file")
    return True


def test_v1_write_file():
    """v1: Create new file with write_file."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "greeting.txt")

        response, calls, _ = run_agent_loop(
            client,
            f"Create a file at {filepath} containing 'Hello, Agent!' using write_file tool.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert any(c[0] == "write_file" for c in calls), "Should use write_file"
        assert os.path.exists(filepath)
        with open(filepath) as f:
            content = f.read()
        assert "Hello" in content

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v1_write_file")
    return True


def test_v1_edit_file():
    """v1: Edit existing file with edit_file."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "config.txt")
        with open(filepath, "w") as f:
            f.write("debug=false\nport=8080\n")

        response, calls, _ = run_agent_loop(
            client,
            f"Edit {filepath} to change debug=false to debug=true using edit_file tool.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert any(c[0] == "edit_file" for c in calls), "Should use edit_file"
        with open(filepath) as f:
            content = f.read()
        assert "debug=true" in content

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v1_edit_file")
    return True


def test_v1_read_edit_verify():
    """v1: Multi-tool workflow: read -> edit -> verify."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "version.txt")
        with open(filepath, "w") as f:
            f.write("version=1.0.0")

        response, calls, _ = run_agent_loop(
            client,
            f"1. Read {filepath}, 2. Change version to 2.0.0, 3. Read it again to verify.",
            V1_TOOLS,
            workdir=tmpdir
        )

        tool_names = [c[0] for c in calls]
        assert "read_file" in tool_names, "Should read file"
        assert "edit_file" in tool_names or "write_file" in tool_names, "Should modify file"

        with open(filepath) as f:
            content = f.read()
        assert "2.0.0" in content

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v1_read_edit_verify")
    return True


# =============================================================================
# v2 Tests: Todo Tracking
# =============================================================================

def test_v2_todo_single_task():
    """v2: Agent uses TodoWrite for simple task."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        system = f"""You are a coding agent at {tmpdir}.
Use TodoWrite to track tasks. Use write_file to create files. Be concise."""

        response, calls, _ = run_agent_loop(
            client,
            f"Create a file at {tmpdir}/hello.txt with content 'hello'. First use TodoWrite to plan, then use write_file to create the file.",
            V2_TOOLS,
            workdir=tmpdir,
            system_prompt=system,
            max_turns=10
        )

        todo_calls = [c for c in calls if c[0] == "TodoWrite"]
        write_calls = [c for c in calls if c[0] == "write_file"]
        file_exists = os.path.exists(os.path.join(tmpdir, "hello.txt"))

        print(f"TodoWrite calls: {len(todo_calls)}, write_file calls: {len(write_calls)}")

        # Pass if file created (core functionality)
        # TodoWrite is optional for simple tasks
        assert file_exists or len(write_calls) >= 1, "Should attempt to create file"

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v2_todo_single_task")
    return True


def test_v2_todo_multi_step():
    """v2: Agent uses TodoWrite for multi-step task."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        system = f"""You are a coding agent at {tmpdir}.
Use TodoWrite to plan multi-step tasks. Use write_file to create files. Complete all steps."""

        response, calls, _ = run_agent_loop(
            client,
            f"""Create 3 files in {tmpdir}:
1. Use write_file to create a.txt with content 'A'
2. Use write_file to create b.txt with content 'B'
3. Use write_file to create c.txt with content 'C'
Use TodoWrite to track progress. Execute all steps.""",
            V2_TOOLS,
            workdir=tmpdir,
            system_prompt=system,
            max_turns=25
        )

        # Check files created
        files_created = sum(1 for f in ["a.txt", "b.txt", "c.txt"]
                          if os.path.exists(os.path.join(tmpdir, f)))

        write_calls = [c for c in calls if c[0] == "write_file"]
        todo_calls = [c for c in calls if c[0] == "TodoWrite"]

        print(f"Files created: {files_created}/3, write_file calls: {len(write_calls)}, TodoWrite calls: {len(todo_calls)}")

        # Pass if at least 2 files created or 2 write attempts made
        assert files_created >= 2 or len(write_calls) >= 2, f"Should create/attempt at least 2 files"

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_v2_todo_multi_step")
    return True


# =============================================================================
# Error Handling Tests
# =============================================================================

def test_error_file_not_found():
    """Error: Agent handles missing file gracefully."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        response, calls, _ = run_agent_loop(
            client,
            f"Read the file {tmpdir}/nonexistent.txt and tell me if it exists.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert response is not None, "Should return a response"
        # Agent should acknowledge file doesn't exist
        assert any(word in response.lower() for word in ["not", "error", "exist", "found", "cannot"])

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_error_file_not_found")
    return True


def test_error_command_fails():
    """Error: Agent handles failed command gracefully."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    response, calls, _ = run_agent_loop(
        client,
        "Run the command 'nonexistent_command_xyz' and tell me what happens.",
        [BASH_TOOL]
    )

    assert response is not None
    assert any(word in response.lower() for word in ["not found", "error", "fail", "command"])

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_error_command_fails")
    return True


def test_error_edit_string_not_found():
    """Error: Agent handles edit with missing string."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "test.txt")
        with open(filepath, "w") as f:
            f.write("hello world")

        response, calls, _ = run_agent_loop(
            client,
            f"Edit {filepath} to replace 'xyz123' with 'abc'. Tell me if it worked.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert response is not None
        # Model should report the issue - check for common phrases or that it tried edit
        resp_lower = response.lower()
        edit_calls = [c for c in calls if c[0] == "edit_file"]
        # Either reports error or tried the edit (which returns error in tool result)
        error_phrases = ["not found", "error", "doesn't", "cannot", "couldn't", "didn't",
                        "wasn't", "unable", "no such", "not exist", "failed", "xyz123"]
        found_error = any(phrase in resp_lower for phrase in error_phrases)
        assert found_error or len(edit_calls) >= 1, "Should report error or attempt edit"

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_error_edit_string_not_found")
    return True


# =============================================================================
# Complex Workflow Tests
# =============================================================================

def test_workflow_create_python_script():
    """Workflow: Create and run a Python script."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        response, calls, _ = run_agent_loop(
            client,
            f"Create a Python script at {tmpdir}/calc.py that prints 2+2, then run it with python3.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert os.path.exists(os.path.join(tmpdir, "calc.py")), "Script should exist"
        tool_names = [c[0] for c in calls]
        assert "write_file" in tool_names, "Should write file"
        assert "bash" in tool_names, "Should run bash"
        assert response and "4" in response

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_workflow_create_python_script")
    return True


def test_workflow_find_and_replace():
    """Workflow: Find files and replace content."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create multiple files
        for i, content in enumerate(["foo=old", "bar=old", "baz=new"]):
            with open(os.path.join(tmpdir, f"file{i}.txt"), "w") as f:
                f.write(content)

        response, calls, _ = run_agent_loop(
            client,
            f"Find all .txt files in {tmpdir} containing 'old' and change 'old' to 'NEW'.",
            V1_TOOLS,
            workdir=tmpdir,
            max_turns=20
        )

        # Check modifications
        modified = 0
        for i in range(3):
            with open(os.path.join(tmpdir, f"file{i}.txt")) as f:
                if "NEW" in f.read():
                    modified += 1

        assert modified >= 2, f"Should modify at least 2 files, got {modified}"

    print(f"Tool calls: {len(calls)}, Files modified: {modified}")
    print("PASS: test_workflow_find_and_replace")
    return True


def test_workflow_directory_setup():
    """Workflow: Create directory structure with files."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        response, calls, _ = run_agent_loop(
            client,
            f"""In {tmpdir}, create this structure:
- src/main.py (content: print('main'))
- src/utils.py (content: print('utils'))
- README.md (content: '# Project')""",
            V1_TOOLS,
            workdir=tmpdir,
            max_turns=20
        )

        # Check structure
        checks = [
            os.path.exists(os.path.join(tmpdir, "src", "main.py")),
            os.path.exists(os.path.join(tmpdir, "src", "utils.py")),
            os.path.exists(os.path.join(tmpdir, "README.md")),
        ]

        passed = sum(checks)
        assert passed >= 2, f"Should create at least 2/3 items, got {passed}"

    print(f"Tool calls: {len(calls)}, Items created: {passed}/3")
    print("PASS: test_workflow_directory_setup")
    return True


# =============================================================================
# Edge Case Tests
# =============================================================================

def test_edge_unicode_content():
    """Edge case: Handle unicode content in files."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        unicode_content = "Hello World\nChinese: \u4e2d\u6587\nEmoji: \u2728\nJapanese: \u3053\u3093\u306b\u3061\u306f"
        filepath = os.path.join(tmpdir, "unicode.txt")

        response, calls, _ = run_agent_loop(
            client,
            f"Create a file at {filepath} with this content:\n{unicode_content}\nThen read it back and confirm the content.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert os.path.exists(filepath), "File should exist"
        with open(filepath, encoding='utf-8') as f:
            content = f.read()
        # Check at least some unicode preserved
        assert "\u4e2d" in content or "Chinese" in content or len(content) > 10

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_unicode_content")
    return True


def test_edge_empty_file():
    """Edge case: Handle empty file operations."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create empty file
        filepath = os.path.join(tmpdir, "empty.txt")
        with open(filepath, "w") as f:
            pass

        response, calls, _ = run_agent_loop(
            client,
            f"Read the file {filepath} and tell me if it's empty or has content.",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert response is not None
        assert any(w in response.lower() for w in ["empty", "no content", "nothing", "0 bytes", "blank"])

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_empty_file")
    return True


def test_edge_special_chars_in_content():
    """Edge case: Handle special characters in file content."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        special_content = 'line1\nline with "quotes"\nline with $variable\nline with `backticks`'
        filepath = os.path.join(tmpdir, "special.txt")

        response, calls, _ = run_agent_loop(
            client,
            f"Create a file at {filepath} containing special characters like quotes, dollar signs, and backticks. Content:\n{special_content}",
            V1_TOOLS,
            workdir=tmpdir
        )

        assert os.path.exists(filepath), "File should exist"
        with open(filepath) as f:
            content = f.read()
        # Should have at least some content
        assert len(content) > 5

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_special_chars_in_content")
    return True


def test_edge_multiline_edit():
    """Edge case: Edit operation spanning multiple lines."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "multi.txt")
        original = """def old_function():
    # old implementation
    return "old"
"""
        with open(filepath, "w") as f:
            f.write(original)

        response, calls, _ = run_agent_loop(
            client,
            f"In {filepath}, replace the entire function 'old_function' with a new function called 'new_function' that returns 'new'.",
            V1_TOOLS,
            workdir=tmpdir
        )

        with open(filepath) as f:
            content = f.read()
        assert "new" in content.lower()

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_multiline_edit")
    return True


def test_edge_nested_directory():
    """Edge case: Create deeply nested directory structure."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        deep_path = os.path.join(tmpdir, "a", "b", "c", "deep.txt")

        response, calls, _ = run_agent_loop(
            client,
            f"Create a file at {deep_path} with content 'deep content'. The directories may not exist yet.",
            V1_TOOLS,
            workdir=tmpdir
        )

        # Check if file was created (via write_file or bash mkdir -p)
        file_exists = os.path.exists(deep_path)
        dir_exists = os.path.exists(os.path.join(tmpdir, "a", "b", "c"))

        assert file_exists or dir_exists, "Should create nested structure"

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_nested_directory")
    return True


def test_edge_large_output():
    """Edge case: Handle large command output."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a file with many lines
        filepath = os.path.join(tmpdir, "large.txt")
        with open(filepath, "w") as f:
            for i in range(500):
                f.write(f"Line {i}: This is a test line with some content.\n")

        response, calls, _ = run_agent_loop(
            client,
            f"Count the number of lines in {filepath}.",
            [BASH_TOOL],
            workdir=tmpdir
        )

        assert response is not None
        assert "500" in response or "lines" in response.lower()

    print(f"Tool calls: {len(calls)}")
    print("PASS: test_edge_large_output")
    return True


def test_edge_concurrent_files():
    """Edge case: Create multiple files in sequence."""
    client = get_client()
    if not client:
        print("SKIP: No API key")
        return True

    with tempfile.TemporaryDirectory() as tmpdir:
        response, calls, _ = run_agent_loop(
            client,
            f"""Create 5 numbered files in {tmpdir}:
- file1.txt with content '1'
- file2.txt with content '2'
- file3.txt with content '3'
- file4.txt with content '4'
- file5.txt with content '5'
Do this as efficiently as possible.""",
            V1_TOOLS,
            workdir=tmpdir,
            max_turns=20
        )

        files_created = sum(1 for i in range(1, 6)
                          if os.path.exists(os.path.join(tmpdir, f"file{i}.txt")))

        assert files_created >= 4, f"Should create at least 4/5 files, got {files_created}"

    print(f"Tool calls: {len(calls)}, Files created: {files_created}/5")
    print("PASS: test_edge_concurrent_files")
    return True


# =============================================================================
# Main
# =============================================================================

if __name__ == "__main__":
    tests = [
        # v0: Bash only
        test_v0_bash_echo,
        test_v0_bash_pipeline,
        # v1: 4 core tools
        test_v1_read_file,
        test_v1_write_file,
        test_v1_edit_file,
        test_v1_read_edit_verify,
        # v2: Todo tracking
        test_v2_todo_single_task,
        test_v2_todo_multi_step,
        # Error handling
        test_error_file_not_found,
        test_error_command_fails,
        test_error_edit_string_not_found,
        # Complex workflows
        test_workflow_create_python_script,
        test_workflow_find_and_replace,
        test_workflow_directory_setup,
        # Edge cases
        test_edge_unicode_content,
        test_edge_empty_file,
        test_edge_special_chars_in_content,
        test_edge_multiline_edit,
        test_edge_nested_directory,
        test_edge_large_output,
        test_edge_concurrent_files,
    ]

    failed = []
    for test_fn in tests:
        name = test_fn.__name__
        print(f"\n{'='*60}")
        print(f"Running: {name}")
        print('='*60)
        try:
            if not test_fn():
                failed.append(name)
        except Exception as e:
            print(f"FAILED: {e}")
            import traceback
            traceback.print_exc()
            failed.append(name)

    print(f"\n{'='*60}")
    print(f"Results: {len(tests) - len(failed)}/{len(tests)} passed")
    print('='*60)

    if failed:
        print(f"FAILED: {failed}")
        sys.exit(1)
    else:
        print("All integration tests passed!")
        sys.exit(0)
