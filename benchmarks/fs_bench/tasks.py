# @amadeus-header
# summary: Task definitions for the fs-coding mini-suite (single-file bug fixes).
# layer: benchmark
# status: active
# feature_flags: none
# provides:
# - module: fs_bench.tasks
# - type: fs_bench.tasks.TASKS
# uses:
# - runtime: python3
# invariants:
# - Each task's buggy `solution` fails its `tests`; the obvious one-line fix makes them pass.
# - Tests import the function from `solution` and run plain asserts, exiting non-zero on failure.
# side_effects: none
# tests:
# - cmd: python -c "import fs_bench.tasks as t; assert len(t.TASKS)==15"
# @end-amadeus-header

"""Single-file Python bug-fix tasks for the fs-coding mini-suite.

Each task is a tiny "repo": one buggy ``solution.py`` plus a ``test_solution.py``
that the correct implementation passes. The agent must read the files, find the
bug, edit ``solution.py``, and verify by running the tests — exercising its
native read/edit/bash tools and the ReAct loop.
"""

TASKS = [
    {
        "name": "sum_list_off_by_one",
        "blurb": "Sum should include every element; the loop skips the first.",
        "solution": """def sum_list(xs):
    total = 0
    for i in range(1, len(xs)):
        total += xs[i]
    return total
""",
        "tests": """from solution import sum_list
assert sum_list([1, 2, 3]) == 6
assert sum_list([10]) == 10
assert sum_list([]) == 0
assert sum_list([5, 5, 5, 5]) == 20
print("OK")
""",
    },
    {
        "name": "max_wrong_operator",
        "blurb": "Track the max, not the min.",
        "solution": """def my_max(xs):
    best = xs[0]
    for x in xs[1:]:
        if x < best:
            best = x
    return best
""",
        "tests": """from solution import my_max
assert my_max([1, 2, 3]) == 3
assert my_max([-5, -1, -10]) == -1
assert my_max([7]) == 7
assert my_max([3, 3, 3]) == 3
print("OK")
""",
    },
    {
        "name": "is_even_modulo",
        "blurb": "Evenness is modulo 2, not modulo 3.",
        "solution": """def is_even(n):
    return n % 3 == 0
""",
        "tests": """from solution import is_even
assert is_even(2) is True
assert is_even(3) is False
assert is_even(0) is True
assert is_even(-4) is True
assert is_even(7) is False
print("OK")
""",
    },
    {
        "name": "reverse_returns_original",
        "blurb": "The result is returned reversed.",
        "solution": """def reverse_string(s):
    return s
""",
        "tests": """from solution import reverse_string
assert reverse_string("abc") == "cba"
assert reverse_string("") == ""
assert reverse_string("a") == "a"
assert reverse_string("ab") == "ba"
print("OK")
""",
    },
    {
        "name": "count_vowels_missing_uppercase",
        "blurb": "Vowels can be uppercase too.",
        "solution": """def count_vowels(s):
    vowels = set("aeiou")
    return sum(1 for ch in s if ch in vowels)
""",
        "tests": """from solution import count_vowels
assert count_vowels("hello") == 2
assert count_vowels("AEIOU") == 5
assert count_vowels("rhythm") == 0
assert count_vowels("Apple") == 2
print("OK")
""",
    },
    {
        "name": "average_integer_division",
        "blurb": "Average should be a real quotient, not floor division.",
        "solution": """def average(xs):
    return sum(xs) // len(xs)
""",
        "tests": """from solution import average
assert average([1, 2]) == 1.5
assert average([4, 4, 4]) == 4.0
assert average([0, 3]) == 1.5
assert average([10]) == 10.0
print("OK")
""",
    },
    {
        "name": "contains_wrong_compare",
        "blurb": "Membership uses `in`, not equality with the first char.",
        "solution": """def contains(haystack, needle):
    return haystack[0] == needle
""",
        "tests": """from solution import contains
assert contains("hello", "e") is True
assert contains("hello", "z") is False
assert contains("abc", "c") is True
assert contains("abc", "a") is True
print("OK")
""",
    },
    {
        "name": "fencepost_join",
        "blurb": "Join should not add a trailing separator.",
        "solution": """def join_with(words, sep):
    out = ""
    for w in words:
        out += w + sep
    return out
""",
        "tests": """from solution import join_with
assert join_with(["a", "b", "c"], "-") == "a-b-c"
assert join_with([], ",") == ""
assert join_with(["solo"], "-") == "solo"
assert join_with(["x", "y"], ", ") == "x, y"
print("OK")
""",
    },
    {
        "name": "absolute_value_missing",
        "blurb": "Distance is absolute; sign should be dropped.",
        "solution": """def distance(a, b):
    return a - b
""",
        "tests": """from solution import distance
assert distance(5, 2) == 3
assert distance(2, 5) == 3
assert distance(-1, -1) == 0
assert distance(0, 4) == 4
print("OK")
""",
    },
    {
        "name": "leap_year_off_by_modulo",
        "blurb": "Century years need the 400 rule.",
        "solution": """def is_leap(year):
    return year % 4 == 0
""",
        "tests": """from solution import is_leap
assert is_leap(2000) is True
assert is_leap(1900) is False
assert is_leap(2024) is True
assert is_leap(2023) is False
assert is_leap(2100) is False
print("OK")
""",
    },
    {
        "name": "string_repeat_times_three",
        "blurb": "Repeat count is off.",
        "solution": """def repeat(s, n):
    return s * (n + 1)
""",
        "tests": """from solution import repeat
assert repeat("ab", 3) == "ababab"
assert repeat("x", 0) == ""
assert repeat("x", 1) == "x"
assert repeat("hi", 2) == "hihi"
print("OK")
""",
    },
    {
        "name": "swapped_min_max",
        "blurb": "clamp should push into range, not out of it.",
        "solution": """def clamp(x, lo, hi):
    if x < hi:
        return hi
    if x > lo:
        return lo
    return x
""",
        "tests": """from solution import clamp
assert clamp(5, 0, 10) == 5
assert clamp(-3, 0, 10) == 0
assert clamp(15, 0, 10) == 10
assert clamp(0, 0, 10) == 0
assert clamp(10, 0, 10) == 10
print("OK")
""",
    },
    {
        "name": "merge_dedupe_missing",
        "blurb": "Set union, but duplicates are kept.",
        "solution": """def unique(xs):
    out = []
    for x in xs:
        out.append(x)
    return out
""",
        "tests": """from solution import unique
assert unique([1, 1, 2, 3, 3]) == [1, 2, 3]
assert unique([]) == []
assert unique(["a", "a", "b"]) == ["a", "b"]
assert unique([5]) == [5]
print("OK")
""",
    },
    {
        "name": "power_wrong_base",
        "blurb": "Exponent loop multiplies the wrong accumulator seed.",
        "solution": """def power(base, exp):
    result = 0
    for _ in range(exp):
        result *= base
    return result
""",
        "tests": """from solution import power
assert power(2, 3) == 8
assert power(5, 0) == 1
assert power(3, 2) == 9
assert power(10, 1) == 10
print("OK")
""",
    },
    {
        "name": "index_of_returns_wrong",
        "blurb": "Found index should be returned, not a constant.",
        "solution": """def index_of(xs, target):
    for i, x in enumerate(xs):
        if x == target:
            return -1
    return -1
""",
        "tests": """from solution import index_of
assert index_of([10, 20, 30], 20) == 1
assert index_of([10, 20, 30], 30) == 2
assert index_of([10, 20, 30], 99) == -1
assert index_of([1, 2], 1) == 0
print("OK")
""",
    },
]

assert len(TASKS) == 15, f"expected 15 tasks, got {len(TASKS)}"
