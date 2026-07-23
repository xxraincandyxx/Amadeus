from solution import join_with
assert join_with(["a", "b", "c"], "-") == "a-b-c"
assert join_with([], ",") == ""
assert join_with(["solo"], "-") == "solo"
assert join_with(["x", "y"], ", ") == "x, y"
print("OK")
