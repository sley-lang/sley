import sys
sys.setrecursionlimit(100000)
name, n = sys.argv[1], int(sys.argv[2])
ns = {}
exec(compile(open(f"{sys.path[0]}/{name}.py").read(), f"{name}.py", "exec"), ns)
print(ns[name](n))
