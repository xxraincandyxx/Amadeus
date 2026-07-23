from solution import clamp
assert clamp(5, 0, 10) == 5
assert clamp(-3, 0, 10) == 0
assert clamp(15, 0, 10) == 10
assert clamp(0, 0, 10) == 0
assert clamp(10, 0, 10) == 10
print("OK")
