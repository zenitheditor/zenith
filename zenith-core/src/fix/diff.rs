//! Line-based unified diff (Myers, 3 lines of context).

/// Lines of context around each change.
const CONTEXT: usize = 3;

/// One line-level edit.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Keep,
    Delete,
    Insert,
}

/// Unified diff of `before` → `after`, with `--- a/<label>` / `+++ b/<label>`
/// headers. Empty when the texts are equal.
pub fn unified_diff(label: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let script = edit_script(&a, &b);

    let mut out = format!("--- a/{label}\n+++ b/{label}\n");
    // Walk the script with line cursors; group changes into hunks.
    let rows = rows(&script);
    let mut i = 0;
    while i < rows.len() {
        let Some(first_change) = rows
            .get(i..)
            .and_then(|r| r.iter().position(|row| row.op != Op::Keep))
            .map(|p| p + i)
        else {
            break;
        };
        let start = first_change.saturating_sub(CONTEXT);
        // Extend the hunk while the gap between changes is ≤ 2 × CONTEXT.
        let mut end = first_change;
        let mut j = first_change;
        while j < rows.len() {
            if rows.get(j).is_some_and(|r| r.op != Op::Keep) {
                end = j;
                j += 1;
                continue;
            }
            if j - end > 2 * CONTEXT {
                break;
            }
            j += 1;
        }
        let stop = (end + CONTEXT + 1).min(rows.len());
        let hunk = rows.get(start..stop).unwrap_or(&[]);
        push_hunk(&mut out, hunk, &a, &b);
        i = stop;
    }
    out
}

/// A script row with the line indices it refers to.
#[derive(Debug, Clone, Copy)]
struct Row {
    op: Op,
    a: usize,
    b: usize,
}

fn rows(script: &[Op]) -> Vec<Row> {
    let (mut a, mut b) = (0, 0);
    script
        .iter()
        .map(|&op| {
            let row = Row { op, a, b };
            match op {
                Op::Keep => {
                    a += 1;
                    b += 1;
                }
                Op::Delete => a += 1,
                Op::Insert => b += 1,
            }
            row
        })
        .collect()
}

fn push_hunk(out: &mut String, hunk: &[Row], a: &[&str], b: &[&str]) {
    let Some(first) = hunk.first() else {
        return;
    };
    let a_len = hunk.iter().filter(|r| r.op != Op::Insert).count();
    let b_len = hunk.iter().filter(|r| r.op != Op::Delete).count();
    let a_start = if a_len == 0 { first.a } else { first.a + 1 };
    let b_start = if b_len == 0 { first.b } else { first.b + 1 };
    out.push_str(&format!("@@ -{a_start},{a_len} +{b_start},{b_len} @@\n"));
    for row in hunk {
        let (sign, line) = match row.op {
            Op::Keep => (' ', a.get(row.a)),
            Op::Delete => ('-', a.get(row.a)),
            Op::Insert => ('+', b.get(row.b)),
        };
        out.push(sign);
        out.push_str(line.copied().unwrap_or_default());
        out.push('\n');
    }
}

/// Shortest edit script from `a` to `b` (Myers greedy, O((N+M)·D)).
fn edit_script(a: &[&str], b: &[&str]) -> Vec<Op> {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    let offset = max + 1;
    let mut v = vec![0usize; 2 * max + 3];
    let mut trace: Vec<Vec<usize>> = Vec::new();

    'outer: for d in 0..=max {
        trace.push(v.clone());
        for k in (0..=2 * d).step_by(2) {
            // diagonal k_real = k - d, stored at offset + k - d.
            let idx = offset + k - d;
            let down = k == 0 || (k != 2 * d && v.get(idx - 1).copied() < v.get(idx + 1).copied());
            let mut x = if down {
                v.get(idx + 1).copied().unwrap_or(0)
            } else {
                v.get(idx - 1).copied().unwrap_or(0) + 1
            };
            // y = x - k_real = x + d - k.
            let mut y = (x + d).saturating_sub(k);
            while x < n && y < m && a.get(x) == b.get(y) {
                x += 1;
                y += 1;
            }
            if let Some(slot) = v.get_mut(idx) {
                *slot = x;
            }
            if x >= n && y >= m {
                trace.push(v.clone());
                break 'outer;
            }
        }
    }
    backtrack(&trace, n, m, offset)
}

/// Rebuild the edit script from the saved `v` rows.
fn backtrack(trace: &[Vec<usize>], n: usize, m: usize, offset: usize) -> Vec<Op> {
    let mut ops = Vec::new();
    let (mut x, mut y) = (n, m);
    // trace[d] holds v before round d; the last entry holds the final v.
    let rounds = trace.len().saturating_sub(1);
    for d in (0..rounds).rev() {
        let Some(v) = trace.get(d) else { break };
        // k_real = x - y; stored index = offset + k_real.
        let k_real = x as isize - y as isize;
        let d_i = d as isize;
        let at = |k: isize| -> usize {
            let i = offset as isize + k;
            usize::try_from(i)
                .ok()
                .and_then(|i| v.get(i).copied())
                .unwrap_or(0)
        };
        if d == 0 {
            while x > 0 && y > 0 {
                ops.push(Op::Keep);
                x -= 1;
                y -= 1;
            }
            break;
        }
        let down = k_real == -d_i || (k_real != d_i && at(k_real - 1) < at(k_real + 1));
        let prev_k = if down { k_real + 1 } else { k_real - 1 };
        let prev_x = at(prev_k);
        let prev_y = usize::try_from(prev_x as isize - prev_k).unwrap_or(0);
        while x > prev_x && y > prev_y {
            ops.push(Op::Keep);
            x -= 1;
            y -= 1;
        }
        if down {
            ops.push(Op::Insert);
            y = y.saturating_sub(1);
        } else {
            ops.push(Op::Delete);
            x = x.saturating_sub(1);
        }
    }
    ops.reverse();
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(a: &[&str], b: &[&str], script: &[Op]) -> Vec<String> {
        let (mut i, mut j) = (0, 0);
        let mut out = Vec::new();
        for op in script {
            match op {
                Op::Keep => {
                    out.push(a[i].to_owned());
                    i += 1;
                    j += 1;
                }
                Op::Delete => i += 1,
                Op::Insert => {
                    out.push(b[j].to_owned());
                    j += 1;
                }
            }
        }
        assert_eq!((i, j), (a.len(), b.len()));
        out
    }

    #[test]
    fn script_rebuilds_target() {
        let cases: &[(&[&str], &[&str])] = &[
            (&["a", "b", "c"], &["a", "x", "c"]),
            (&[], &["a"]),
            (&["a"], &[]),
            (&["a", "b", "c", "d"], &["b", "c", "e", "d", "f"]),
            (&["x"; 5], &["x"; 3]),
        ];
        for (a, b) in cases {
            let script = edit_script(a, b);
            assert_eq!(apply(a, b, &script), b.to_vec(), "{a:?} → {b:?}");
        }
    }

    #[test]
    fn unified_diff_shows_hunk() {
        let before = "1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let after = "1\n2\n3\n4\nfive\n6\n7\n8\n9\n";
        let diff = unified_diff("t.zen", before, after);
        assert_eq!(
            diff,
            "--- a/t.zen\n+++ b/t.zen\n@@ -2,7 +2,7 @@\n 2\n 3\n 4\n-5\n+five\n 6\n 7\n 8\n"
        );
    }

    #[test]
    fn unified_diff_empty_when_equal() {
        assert_eq!(unified_diff("t", "a\n", "a\n"), "");
    }

    #[test]
    fn distant_changes_form_two_hunks() {
        let before: String = (1..=20).map(|i| format!("{i}\n")).collect();
        let after = before.replace("2\n", "two\n").replace("19\n", "nineteen\n");
        let diff = unified_diff("t", &before, &after);
        assert_eq!(diff.matches("@@ -").count(), 2, "{diff}");
    }
}
