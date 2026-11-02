# ClerkScript: the language of the game

ClerkScript is a small, typed, high-level language. Each level is **a function with a typed signature**
and you write its body, with the same ideas you would use in a real interview answer: strings, vectors,
maps, sets, stacks, queues, heaps, linked lists and trees. There is no assembly and no memory addressing.

```text
fn two_sum(nums: vec<int>, target: int) -> vec<int> {
    let seen = map<int, int>();
    for i in range(len(nums)) {
        let want = target - nums[i];
        if seen.contains(want) {
            return [seen[want], i];
        }
        seen[nums[i]] = i;
    }
    return [];
}
```

The language is deterministic and sandboxed (no I/O, no randomness, no `unsafe`, no panics on any input),
has local type inference, friendly clerk-flavoured error messages and an abstract **cost model** that
mirrors real complexity. It is edited in the game as **cards** (blocks) or as **text**; both views are
the same program (the text is always the printed syntax tree, comments excepted).

## Types

| Type | Meaning | Notes |
| --- | --- | --- |
| `int` | 64-bit signed integer | arithmetic is checked: overflow is a runtime error, never wrap-around |
| `bool` | `true` / `false` | |
| `char` | one ASCII character | `'a'`, escapes `\n \t \r \0 \\ \' \"` |
| `string` | ASCII text | immutable-style value; `s += c` appends in place |
| `vec<T>` | growable array | grids are `vec<vec<T>>` |
| `set<T>` | hash set | `T` must be `int`, `bool`, `char`, `string`, or a tuple / small vec of those |
| `map<K,V>` | hash map | same key rule |
| `(A, B, ...)` | tuple (2 or more) | `p.0`, `p.1`, destructuring in `let` and `for` |
| `option<T>` | maybe a value | `none`, `some(x)`, `m.get(k)` |
| `stack<T>` | last in, first out | |
| `queue<T>`, `deque<T>` | FIFO / double ended | |
| `heap<T>` | **max**-heap | store negated numbers or `(-priority, item)` for a min-heap |
| `lnode` | linked-list node handle | fields `val`, `next`; `null` ends a list; `lnode(v, next)` makes one |
| `tnode` | binary-tree node handle | fields `val`, `left`, `right`; `tnode(v, l, r)` |

Comparison (`<`, sorting, heaps) works on `int`, `bool`, `char`, `string`, tuples and vecs of those.
Values behave like values: `let b = a` copies (the cost model charges the copy). `lnode` and `tnode`
are handles to shared nodes, as in C++ or Java.

## Statements

```text
let x = 0;                      let (a, b) = pair;          let v: vec<int> = [];
x = x + 1;      x += 2;         v[i] = 3;       m[key] = 1; node.next = other;
if cond { ... } else if cond { ... } else { ... }
while cond { ... }
for i in range(n) { ... }       range(a, b)     range(a, b, -1)   // the step is a number literal
for x in v { ... }              for c in "text" { ... }
for (i, x) in enumerate(v) { ... }     for (a, b) in zip(v, w) { ... }
break;   continue;   return x;   return;   expr;
```

Rules the checker enforces (with friendly messages): sets and maps have no order, so iterate
`sorted(s)`, `m.keys()`, `m.values()` or `m.items()` (these return a shelf sorted by key); do not change a
shelf you are walking over; do not assign to a `range` counter; every path of a function that returns a
value must end in `return`; helper functions are optional per level.

### Functions

```text
fn helper(grid: vec<vec<char>>, row: int, seen: &mut set<int>) -> int { ... }
```

Parameters are passed by value (read-only unless the callee assigns, in which case the caller's value is
copied). A `&mut` parameter lets the callee change the caller's variable (pass a plain variable). Recursion
is allowed where the level unlocks it (depth limit 200).

## Expressions

Precedence, loosest to tightest: `or`, `and`, `not`, comparisons (`== != < <= > >=`), `|`, `^`, `&`,
`<< >>`, `+ -`, `* / %`, unary `- ~`, then postfix (`.field`, `.method(..)`, `[i]`, `[a..b]`, `.0`).
`&&`, `||` and `!` are accepted as aliases of `and`, `or`, `not`. Comparison is below the bit operators
(like Python, unlike C): `n & (n - 1) == 0` means `(n & (n - 1)) == 0`.
Division truncates toward zero; `%` has the sign of the left operand.

Literals: `42`, `0xff`, `0b101`, `true`, `'a'`, `"text"`, `[1, 2, 3]`, `[0; n]` (repeat), `(a, b)`,
`null`, `none`, and empty containers `vec<int>()`, `map<string, int>()`, `set<char>()`, `stack<int>()`,
`queue<int>()`, `deque<int>()`, `heap<int>()`.

## Built-ins

Each level lists which are **unlocked**; the checker rejects a locked one with a message that names the
reason (for example `sorted` in the "sort it yourself" level). Method calls are written `x.name(..)`.

| Receiver | Tools | Cost |
| --- | --- | --- |
| any | `len(x)` | 1 |
| ints | `min(a,b)`, `max(a,b)`, `abs(x)`, `to_string(x)`, `chr(x)` | 1 |
| vec / string | `min(v)`, `max(v)`, `sum(v)`, `sorted(x)`, `reversed(x)`, `x[a..b]` | n, n, n, n log n, n, slice length |
| vec | `push`, `pop`, `insert(i,x)`, `remove(i)`, `contains`, `index_of`, `count`, `reverse`, `sort`, `swap(i,j)`, `clear`, `is_empty`, `extend` | 1, 1, n-i, n-i, n, n, n, n, n log n, 1, 1, 1, m |
| string | `upper`, `lower`, `trim`, `find`, `contains`, `starts_with`, `ends_with`, `split`, `chars`, `push`, `count`, `replace`, `repeat`, `join(v, sep)`, `from_chars`, `to_int` | n (push is 1) |
| char | `is_digit`, `is_alpha`, `is_alnum`, `is_upper`, `is_lower`, `is_space`, `upper`, `lower`, `ord(c)` | 1 |
| set | `add`, `remove`, `contains`, `union`, `intersection`, `difference`, `is_empty`, `clear` | 1, 1, 1, n+m |
| map | `m[k]`, `m[k] = v`, `get(k)`, `get(k, default)`, `contains`, `remove`, `keys`, `values`, `items`, `is_empty`, `clear` | 1; keys / values / items cost n |
| option | `is_some`, `is_none`, `unwrap`, `unwrap_or` | 1 |
| stack | `push`, `pop`, `peek`, `is_empty`, `clear` | 1 |
| queue / deque | `push`, `pop`, `front` / `push_front`, `push_back`, `pop_front`, `pop_back`, `front`, `back` | 1 |
| heap | `push`, `pop` (log n + 1), `peek` (1) | log n |
| lnode / tnode | field read and write | 1 |

## The cost model

Every statement costs 1 and every operation costs what it would cost in real code (table above;
equality and concatenation cost the length, copying a collection costs its size, a function call costs 1).
**Speed = total abstract cost.** A solution runs against hidden tests *including large inputs* with a step
budget: an O(n squared) solution that the level calls "naive" runs out of time on the big tests exactly like
a time-limit-exceeded verdict, while the intended O(n) or O(n log n) solution passes. After a level is
solved the judge measures the solution at sizes n, 2n, 4n, 8n and reports an **empirical time class**
(O(1), O(log n), O(sqrt n), O(n), O(n log n), O(n^1.5), O(n^2), O(n^3), O(2^n)) and a space class from the
peak number of live cells. Medals use **size** (statement and operation count) and **speed**
(average cost on the standard tests): Paper Clip, Stapler and Golden Stamp.

Limits per run: cost budget (the "time limit"), 200 nested calls, 4 million live elements. Exceeding one is a
clerk-flavoured runtime error, never a crash.

## Grammar

```ebnf
program   = { function } ;
function  = "fn" ident "(" [ param { "," param } ] ")" [ "->" type ] block ;
param     = ident ":" [ "&" "mut" ] type ;
type      = "int" | "bool" | "char" | "string" | "lnode" | "tnode"
          | ("vec"|"set"|"option"|"stack"|"queue"|"deque"|"heap") "<" type ">"
          | "map" "<" type "," type ">" | "(" type "," type { "," type } ")" ;
block     = "{" { statement } "}" ;
statement = "let" pattern [ ":" type ] "=" expr ";"
          | place ( "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ) expr ";"
          | "if" expr block { "else" "if" expr block } [ "else" block ]
          | "while" expr block
          | "for" ident "in" "range" "(" expr [ "," expr [ "," int ] ] ")" block
          | "for" pattern "in" expr block
          | "break" ";" | "continue" ";" | "return" [ expr ] ";" | expr ";" ;
pattern   = ident | "_" | "(" pattern "," pattern { "," pattern } ")" ;
place     = ident | place "[" expr "]" | place "." (ident | int) ;
expr      = or ;   (* precedence as listed above *)
primary   = int | char | string | "true" | "false" | "null" | "none" | ident
          | ident "(" [ args ] ")" | type-keyword "<" ... ">" "(" ")"
          | "[" [ expr { "," expr } ] "]" | "[" expr ";" expr "]" | "(" expr { "," expr } ")" ;
postfix   = primary { "." ident [ "(" [ args ] ")" ] | "." int | "[" expr "]" | "[" [expr] ".." [expr] "]" } ;
comment   = "//" ... end of line   (comments are not kept when text and cards are converted)
```

## Cards and text

The game's card editor and its text view show the same program. Cards are edited with a chip pad (variables
in scope with their types, a number keypad, operators, the unlocked tools filtered by the type of the
variable you picked) and can be dragged to reorder (long-press on touch); the text view has auto-indent,
error squiggles and an on-screen keyboard with a symbol layer. Converting text to cards and back is lossless
except for comments and spacing.
