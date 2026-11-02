#include <algorithm>
#include <vector>

struct Point {
    int x = 0;
    int y = 0;
    Point() {}
    Point(int px, int py) : x(px), y(py) {}
};

struct Segment {
    Point a;
    Point b;
    bool vertical = false;
    Segment(Point p, Point q) : a(p), b(q) { vertical = p.x == q.x; }
};

int manhattan(const Point& p, const Point& q) {
    return std::abs(p.x - q.x) + std::abs(p.y - q.y);
}

int farthest(const std::vector<Point>& pts) {
    Point origin;
    Point first{1, 2};
    Point second = {3, 4};
    Point third(5, 6);
    int best = manhattan(origin, first);
    for (const Point& p : pts) {
        best = std::max(best, manhattan(origin, p));
    }
    std::vector<Point> more = {Point(1, 1), {2, 2}};
    return best + manhattan(second, third) + (int)more.size();
}
