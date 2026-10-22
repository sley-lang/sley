pub struct Point {
    pub x: i64,
    pub y: i64,
}

pub fn manhattan(a: &Point, b: &Point) -> i64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
