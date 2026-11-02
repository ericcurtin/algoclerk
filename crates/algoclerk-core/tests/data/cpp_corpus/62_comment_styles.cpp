// File header comment
// spanning two lines
#include <vector>  // vectors
#include <string>

/* block comment above a function
   that runs over several lines */
int sum(const std::vector<int>& v) {
    // accumulate everything
    int total = 0;  // running total
    for (int x : v) {
        total += x;  // add one
        // done with x
    }
    /* before return */
    return total;
    // after return, last thing in the block
}

void emptyBody() {
    // nothing to see here
}

void nested(int n) {
    if (n > 0) {  // positive
        // inner comment
        n--;
    } else {  // not positive
        n++;
    }
    while (n < 10) {  // loop head
        n += 2;
    }
    switch (n) {  // switch head
        case 1:  // one
            break;
        default:  // everything else
            n = 0;
    }
    {
        // lonely block
        n = 3;
    }
}
