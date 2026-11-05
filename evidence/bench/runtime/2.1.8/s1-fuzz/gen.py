import random,sys
def expr(r,names,d=0):
    if d>2 or r.random()<0.3:
        return r.choice(names+[str(r.randint(0,9)),str(r.randint(-9,9))])
    op=r.choice(['+','-','*','//','%','//','%'])
    a=expr(r,names,d+1)
    if op in('//','%'):
        dv=r.choice([str(r.randint(1,7)),str(-r.randint(1,7)),'('+r.choice(names)+' * 0 + %d)'%r.choice([3,-3,5])])
        return f"({a} {op} {dv})"
    return f"({a} {op} {expr(r,names,d+1)})"
def prog(seed):
    r=random.Random(seed)
    lines=["import pytest",""]
    fns=[]
    for k in range(5):
        lo=r.randint(0,2) if k%2 else r.randint(-3,2); hi=lo+r.randint(1,8)
        body=[f"def f{k}(a: int, b: int) -> int:","    t = 0", f"    for i in range({lo}, {hi}):",
              f"        x = {expr(r,['i','a','b'])}",
              f"        y = {expr(r,['i','x'])}",
              f"        t += x // {r.choice([2,3,10,-4])} + y % {r.choice([2,7,10,-3])}",
              f"        u = i * {r.randint(0,5)} + {r.randint(0,9)}",
              f"        t += u // {r.randint(1,9)} + u % {r.randint(1,9)}",
              f"        u = u - {r.randint(0,12)}",
              f"        t += u // {r.choice([3,-3,4])} + u % {r.choice([4,-4,5])}",
              f"        while x > 0:", f"            t += x % 10", f"            x //= 10",
              "    return t"]
        lines+=body+[""]
        fns.append(body)
    ns={}
    exec("\n".join(lines[2:]),ns)
    tests=["def test_all():"]
    for k in range(5):
        for _ in range(8):
            a,b=r.randint(-20,20),r.randint(-20,20)
            tests.append(f"    assert f{k}({a}, {b}) == {ns['f%d'%k](a,b)}")
    return "\n".join(lines+tests)+"\n"
seed=int(sys.argv[1]);open(f"p{seed}.py","w").write(prog(seed))
