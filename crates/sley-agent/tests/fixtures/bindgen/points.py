@dataclass
class Point:
    x: int
    y: int

def shift(ps: list[Point], dx: int) -> list[Point]:
    out: list[Point] = []
    for p in ps:
        out.append(Point(p.x + dx, p.y))
    return out

def norm1(p: Point) -> int:
    return abs(p.x) + abs(p.y)
