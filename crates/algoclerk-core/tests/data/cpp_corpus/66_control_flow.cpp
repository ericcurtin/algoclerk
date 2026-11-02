#include <vector>

int sumDigits(int n) {
    int s = 0;
    do {
        s += n % 10;
        n /= 10;
    } while (n > 0);
    return s;
}

int classify(int x, int y) {
    if (x > 0)
        if (y > 0)
            return 1;
        else
            return 2;
    else if (x < 0)
        return 3;
    return 4;
}

int dangling(int a, int b) {
    int r = 0;
    if (a)
        if (b) r = 1;
        else r = 2;
    for (;;) {
        if (r > 5) break;
        r += 2;
    }
    while (true) {
        r--;
        if (r < 3) break;
    }
    return r;
}

int collatz(long long n) {
    int steps = 0;
    for (; n != 1; ++steps) {
        n = n % 2 == 0 ? n / 2 : 3 * n + 1;
    }
    return steps;
}

int fallthrough(int day) {
    int score = 0;
    switch (day) {
        case 1:
            score += 1;
        case 2:
            score += 2;
            break;
        case 3: {
            int bonus = 7;
            score += bonus;
            break;
        }
        case 4:
        case 5:
            return 9;
        default:
            score = -1;
    }
    return score;
}

int nestedLoops(const std::vector<std::vector<int>>& g) {
    int found = 0;
    for (size_t i = 0; i < g.size(); ++i) {
        for (size_t j = 0; j < g[i].size(); ++j) {
            if (g[i][j] < 0) continue;
            if (g[i][j] == 0) break;
            found += g[i][j];
        }
    }
    {
        int scoped = found;
        found = scoped + 1;
    }
    ;
    return found;
}
