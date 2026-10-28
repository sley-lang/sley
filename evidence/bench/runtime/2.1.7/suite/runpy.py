import sys, json
sys.setrecursionlimit(100000)
name = sys.argv[1]
args = [json.load(open(a)) if a.endswith(".json") else int(a) for a in sys.argv[2:]]
ns = {}
exec(compile(open(f"{sys.path[0]}/{name}.py").read(), f"{name}.py", "exec"), ns)
print(ns[name](*args))
