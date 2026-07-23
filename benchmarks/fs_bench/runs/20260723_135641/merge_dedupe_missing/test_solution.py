from solution import unique
assert unique([1, 1, 2, 3, 3]) == [1, 2, 3]
assert unique([]) == []
assert unique(["a", "a", "b"]) == ["a", "b"]
assert unique([5]) == [5]
print("OK")
