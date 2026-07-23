def index_of(xs, target):
    for i, x in enumerate(xs):
        if x == target:
            return i
    return -1
