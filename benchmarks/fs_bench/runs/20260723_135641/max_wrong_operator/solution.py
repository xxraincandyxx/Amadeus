def my_max(xs):
    best = xs[0]
    for x in xs[1:]:
        if x > best:
            best = x
    return best
