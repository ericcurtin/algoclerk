# The AlgoClerk instruction set

The clerk is a tiny machine with a few parts:

- the **inbox**: a queue of integers waiting to be processed,
- the **outbox**: a list of integers the clerk has delivered,
- the **hand**: holds one integer, or nothing,
- the **floor**: numbered **tiles** `0 .. N-1`; each tile holds one integer or is empty.

Programs are lists of instructions, labels and comments. The same program can be edited as a list
(drag and drop, tap to add) or as plain text; both views are always in sync. Everything below is
implemented in `crates/algoclerk-core` (`isa`, `asm`, `vm`).

## Values

Values are integers between -999,999 and 999,999 ("six digits fit on the paper"). Any result outside
that range stops the clerk with an *overflow* error. Characters are small integers (their Unicode code
point): `'a'` is 97. Levels can ask the UI to draw values as characters.

## Instructions

`n` is a tile number. `[n]` is an *indirect* operand: the tile whose number is stored on tile `n`.
Indirect operands are offered by levels that allow `indirect` (arrays, stacks, linked lists).

| Instruction | Effect | Errors |
| --- | --- | --- |
| `INBOX` | Take the next value from the inbox into the hand. If the inbox is empty, the shift **ends** (this is how programs finish). | none |
| `OUTBOX` | Append the hand to the outbox. The hand becomes empty. | empty hand |
| `COPYTO n` | tile n = hand (the hand keeps its value). | empty hand, no such tile |
| `COPYFROM n` | hand = tile n. | empty tile, no such tile |
| `ADD n` | hand = hand + tile n. | empty hand, empty tile, overflow |
| `SUB n` | hand = hand - tile n. | empty hand, empty tile, overflow |
| `BUMPUP n` | tile n = tile n + 1, hand = the new value. | empty tile, overflow |
| `BUMPDN n` | tile n = tile n - 1, hand = the new value. | empty tile, overflow |
| `MUL n` | hand = hand * tile n. | empty hand, empty tile, overflow |
| `DIV n` | hand = hand / tile n, truncating toward zero. | also divide by zero |
| `MOD n` | hand = hand % tile n; the sign follows the hand (like C and Rust). | also divide by zero |
| `JUMP label` | Continue at `label`. | none |
| `JUMPZ label` | Continue at `label` if the hand is 0, otherwise continue normally. The hand is kept. | empty hand |
| `JUMPN label` | Continue at `label` if the hand is negative. The hand is kept. | empty hand |

Execution starts at the first instruction. The program ends when `INBOX` finds the inbox empty, or
when execution runs past the last instruction (jumping to a label at the very end is fine).
Every executed instruction counts as one **step**; labels and comments are free.

An error never panics the game. It stops the run, the clerk shows a friendly message ("The clerk's
hand is empty...") and the offending instruction stays highlighted. Programs also stop after the
level's **step limit** (default 20,000 instructions per test), so infinite loops cannot hang a test.

The extended instructions (`MUL`, `DIV`, `MOD`) and indirect addressing are unlocked per level by the
level file. Planned later extensions (`CALL`/`RET`) are listed in the [roadmap](../ROADMAP.md).

## Text syntax

```text
-- a comment (also: # comment, ; comment, // comment)
start:                 -- a label: a name, a colon
    INBOX              -- instructions are case-insensitive; indentation is free
    JUMPZ done
    COPYTO 3
    ADD [3]            -- indirect operand
    OUTBOX
    JUMP start
done:                  -- labels may sit at the very end
```

- One instruction per line; `label: INSTRUCTION` on one line is accepted too.
- Operands are non-negative integers up to 4095, or `[n]`.
- Label names: letters, digits and `_`, not starting with a digit, at most 48 characters.
- Every problem in a text is reported with its line number; the parser never panics.

## Scoring

A program is **correct** when it produces exactly the expected outbox on every test of the level
(examples, hand-written edge cases and generated cases). Two challenge metrics follow:

- **Size**: the number of instructions (labels and comments are free).
- **Speed**: steps executed, averaged over all tests (the worst case is shown as well).

Each level has `size_par`/`size_ace` and `speed_par`/`speed_ace` targets. Meeting `par` earns a
*staple*, meeting `ace` a *gold seal*, merely solving earns a *paperclip*. A level has up to three
stars: one for solving it, one for a staple or better on size, one for a staple or better on speed.
