from dataclasses import dataclass


@dataclass
class Point:
    x: int
    y: int


def manhattan(a: Point, b: Point) -> int:
    return abs(a.x - b.x) + abs(a.y - b.y)
