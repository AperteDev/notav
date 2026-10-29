//! Editor mode's tables: a GitHub Flavored Markdown table kept aligned (cells padded so the
//! pipes line up, the delimiter row holding each column's alignment) and changed by rows and
//! columns. Each change is one edit of the table's lines, with where the cursor goes after it.

use crate::markdown::{Edit, options};
use pulldown_cmark::{Event, Parser, Tag};
use std::cmp::Ordering;
use std::ops::Range;

/// A column's alignment, from the colons of the delimiter row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// What to do to the table, at the cell of the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Only align it.
    Tidy,
    RowAbove,
    RowBelow,
    DeleteRow,
    MoveRowUp,
    MoveRowDown,
    /// To be row `to` (1 the first below the header).
    MoveRow {
        to: usize,
    },
    /// A copy of the row below it.
    DuplicateRow,
    ColumnLeft,
    ColumnRight,
    DeleteColumn,
    MoveColumnLeft,
    MoveColumnRight,
    /// To be column `to` (0 the first).
    MoveColumn {
        to: usize,
    },
    /// A copy of the column right of it.
    DuplicateColumn,
    Align(Align),
    /// The rows below the header, by the column: numbers by value, the rest alphabetically.
    Sort {
        descending: bool,
    },
}

/// An edit and where the cursor goes after it (in characters, as `Edit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub edit: Edit,
    pub cursor: usize,
}

/// Do `op` to the table holding character `at`, with the cursor at character `cursor` (in it
/// or elsewhere). None when there is no table there, `op` cannot be done (the header row has
/// nothing above it, nor moves) or nothing changes.
pub fn apply(text: &str, at: usize, cursor: usize, op: Op) -> Option<Change> {
    let mut t = Table::find(text, byte(text, at)?)?;
    let target = t.place(text, byte(text, at)?);
    let cursor_byte = byte(text, cursor)?;
    let mut place = (t.start..=t.end)
        .contains(&cursor_byte)
        .then(|| t.place(text, cursor_byte));
    let (rows, columns) = (t.rows.len(), t.aligns.len());
    let Place { row, column, .. } = target;
    let mut go = |row, column| {
        place = Some(Place {
            row,
            column,
            spot: Spot::Text(0),
        })
    };
    let op = match op {
        Op::MoveRowUp => Op::MoveRow {
            to: row.checked_sub(1)?,
        },
        Op::MoveRowDown => Op::MoveRow { to: row + 1 },
        Op::MoveColumnLeft => Op::MoveColumn {
            to: column.checked_sub(1)?,
        },
        Op::MoveColumnRight => Op::MoveColumn { to: column + 1 },
        op => op,
    };
    match op {
        Op::Tidy => {}
        Op::RowAbove | Op::RowBelow => {
            if op == Op::RowAbove && row == 0 {
                return None;
            }
            let i = if op == Op::RowAbove { row } else { row + 1 };
            t.rows.insert(i, vec![String::new(); columns]);
            go(i, column);
        }
        Op::DeleteRow => {
            if row == 0 {
                return None;
            }
            t.rows.remove(row);
            go(row.min(rows - 2), column);
        }
        Op::MoveRow { to } => {
            if row == 0 || to == 0 || to >= rows || to == row {
                return None;
            }
            let moved = t.rows.remove(row);
            t.rows.insert(to, moved);
            place = Some(Place { row: to, ..target });
        }
        Op::DuplicateRow => {
            t.rows.insert(row + 1, t.rows[row].clone());
            place = Some(Place {
                row: row + 1,
                ..target
            });
        }
        Op::ColumnLeft | Op::ColumnRight => {
            let i = if op == Op::ColumnLeft {
                column
            } else {
                column + 1
            };
            for r in &mut t.rows {
                r.insert(i, String::new());
            }
            t.aligns.insert(i, Align::None);
            go(row, i);
        }
        Op::DeleteColumn => {
            if columns == 1 {
                return None;
            }
            for r in &mut t.rows {
                r.remove(column);
            }
            t.aligns.remove(column);
            go(row, column.min(columns - 2));
        }
        Op::MoveColumn { to } => {
            if to >= columns || to == column {
                return None;
            }
            for r in &mut t.rows {
                let moved = r.remove(column);
                r.insert(to, moved);
            }
            let moved = t.aligns.remove(column);
            t.aligns.insert(to, moved);
            place = Some(Place {
                column: to,
                ..target
            });
        }
        Op::DuplicateColumn => {
            for r in &mut t.rows {
                r.insert(column + 1, r[column].clone());
            }
            t.aligns.insert(column + 1, t.aligns[column]);
            place = Some(Place {
                column: column + 1,
                ..target
            });
        }
        Op::MoveRowUp | Op::MoveRowDown | Op::MoveColumnLeft | Op::MoveColumnRight => {
            unreachable!()
        }
        Op::Align(a) => t.aligns[column] = a,
        Op::Sort { descending } => {
            t.rows[1..].sort_by(|a, b| {
                let o = compare(&a[column], &b[column]);
                if descending { o.reverse() } else { o }
            });
        }
    }
    let (new, starts) = t.format();
    let old = &text[t.start..t.end];
    if new == old {
        return None;
    }
    let start = chars(text, t.start);
    let cursor = match place {
        Some(p) => {
            let (line_start, cells, line_end) = &starts[p.row];
            let len = t.rows[p.row][p.column].chars().count();
            start
                + match p.spot {
                    Spot::Start => chars(&new, *line_start),
                    Spot::End => chars(&new, *line_end),
                    Spot::Text(offset) => chars(&new, cells[p.column]) + offset.min(len),
                }
        }
        None if cursor_byte > t.end => cursor + new.chars().count() - old.chars().count(),
        None => cursor,
    };
    Some(Change {
        edit: Edit {
            start,
            end: chars(text, t.end),
            text: new,
        },
        cursor,
    })
}

/// Where Tab, Shift+Tab and Enter take the cursor in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// To the next cell, the next row's first after a row's last.
    Next,
    Previous,
    /// To the same column in the next row.
    Down,
}

/// Move the cursor at character `cursor` in a table by `step`, to the end of a cell's text (an
/// empty edit: nothing changes). Past the last row, Next and Down add one, the table tidied;
/// Down from an empty last row takes it away and leaves the table, on a line of its own below.
/// None out of tables, before the header's first cell with Previous, and at the header's start
/// with Down (Enter there makes room above the table).
pub fn step(text: &str, cursor: usize, step: Step) -> Option<Change> {
    let b = byte(text, cursor)?;
    let mut t = Table::find(text, b)?;
    let Place { row, column, spot } = t.place(text, b);
    let (rows, columns) = (t.rows.len(), t.aligns.len());
    let target = match step {
        Step::Down if row == 0 && matches!(spot, Spot::Start) => return None,
        Step::Next if column + 1 < columns => Some((row, column + 1)),
        Step::Next => (row + 1 < rows).then_some((row + 1, 0)),
        Step::Previous if column > 0 => Some((row, column - 1)),
        Step::Previous => Some((row.checked_sub(1)?, columns - 1)),
        Step::Down => (row + 1 < rows).then_some((row + 1, column)),
    };
    let lines: Vec<&str> = text[t.start..t.end].split('\n').collect();
    // Where row `r`'s line starts (bytes): the delimiter row is the second line.
    let line_at = |r: usize| {
        let i = if r == 0 { 0 } else { r + 1 };
        t.start + lines[..i].iter().map(|l| l.len() + 1).sum::<usize>()
    };
    if let Some((r, c)) = target {
        let line = lines[if r == 0 { 0 } else { r + 1 }];
        let end = cells(line)
            .get(c)
            .map_or(line.trim_end().len(), |c| c.text_start + c.text.len());
        let at = chars(text, line_at(r) + end);
        return Some(Change {
            edit: Edit {
                start: at,
                end: at,
                text: String::new(),
            },
            cursor: at,
        });
    }
    if step == Step::Down && row > 0 && t.rows[row].iter().all(String::is_empty) {
        let start = chars(text, line_at(row));
        return Some(Change {
            edit: Edit {
                start,
                end: chars(text, t.end),
                text: "\n".into(),
            },
            cursor: start + 1,
        });
    }
    t.rows.push(vec![String::new(); columns]);
    let (new, starts) = t.format();
    let start = chars(text, t.start);
    let column = if step == Step::Next { 0 } else { column };
    Some(Change {
        cursor: start + chars(&new, starts[rows].1[column]),
        edit: Edit {
            start,
            end: chars(text, t.end),
            text: new,
        },
    })
}

/// Enter at the end of a line written as a table's header (`| a | b |`) that no table holds
/// yet: the table it starts, tidied, with an empty row and the cursor in its first cell. None
/// when the parser would not take it for a table (in a code block, a quote).
pub fn start(text: &str, cursor: usize) -> Option<Change> {
    let b = byte(text, cursor)?;
    let line_start = text[..b].rfind('\n').map_or(0, |n| n + 1);
    let line_end = text[b..].find('\n').map_or(text.len(), |n| b + n);
    let line = &text[line_start..line_end];
    let trimmed = line.trim();
    let header = trimmed.len() > 1 && trimmed.starts_with('|') && trimmed.ends_with('|');
    if !header || !text[b..line_end].trim().is_empty() || Table::find(text, b).is_some() {
        return None;
    }
    let indent = &line[..line.len() - line.trim_start().len()];
    let names = cells(line).into_iter().map(|c| c.text).collect();
    let (table, starts) = Table::new(indent.into(), vec![names, Vec::new()], Vec::new()).format();
    // A line of text under it would be its next row.
    let next = text
        .get(line_end + 1..)
        .map(|t| t.split('\n').next().unwrap_or(t));
    let after = if next.is_some_and(|l| !l.trim().is_empty()) {
        "\n"
    } else {
        ""
    };
    let new = format!("{}{table}{after}{}", &text[..line_start], &text[line_end..]);
    if Table::find(&new, line_start + indent.len()).is_none_or(|t| t.start != line_start) {
        return None;
    }
    let start = chars(text, line_start);
    Some(Change {
        cursor: start + chars(&table, starts[1].1[0]),
        edit: Edit {
            start,
            end: chars(text, line_end),
            text: format!("{table}{after}"),
        },
    })
}

/// Whether character `at` is in a table's lines.
pub fn is_at(text: &str, at: usize) -> bool {
    byte(text, at).and_then(|b| Table::find(text, b)).is_some()
}

/// Where a table's rows and columns are (characters), for the editor to point at them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Each row's first character past its indent: the header first, the delimiter row left out.
    pub rows: Vec<usize>,
    /// The header's column edges: its first character, then each cell's closing pipe (the
    /// line's end where that is left out). Column `c` is between `edges[c]` and `edges[c + 1]`.
    pub edges: Vec<usize>,
}

/// The layout of the table holding character `at`.
pub fn layout(text: &str, at: usize) -> Option<Layout> {
    let t = Table::find(text, byte(text, at)?)?;
    let lines: Vec<&str> = text[t.start..t.end].split('\n').collect();
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut rows = Vec::new();
    let mut line_start = t.start;
    for (i, line) in lines.iter().enumerate() {
        if i != 1 {
            rows.push(chars(text, line_start + indent(line)));
        }
        line_start += line.len() + 1;
    }
    let edges = [indent(lines[0])]
        .into_iter()
        .chain(cells(lines[0]).iter().map(|c| c.stretch.end))
        .map(|b| chars(text, t.start + b))
        .collect();
    Some(Layout { rows, edges })
}

/// An empty table of two columns, a header and one row.
pub fn empty() -> String {
    Table::new(String::new(), vec![vec![String::new(); 2]; 2], Vec::new())
        .format()
        .0
}

/// Text copied from a spreadsheet as a table: two lines or more, each with a tab between
/// cells; the first line is the header.
pub fn from_tsv(pasted: &str) -> Option<String> {
    let lines: Vec<&str> = pasted.trim_end_matches(['\n', '\r']).lines().collect();
    if lines.len() < 2 || !lines.iter().all(|l| l.contains('\t')) {
        return None;
    }
    let rows = lines
        .iter()
        .map(|l| {
            l.split('\t')
                .map(|c| c.trim().replace('|', "\\|"))
                .collect()
        })
        .collect();
    Some(Table::new(String::new(), rows, Vec::new()).format().0)
}

/// Put `table` at character `cursor` as a block of its own (after the cursor's line if that has
/// text), blank lines around it; the cursor in its first cell.
pub fn insert(text: &str, cursor: usize, table: &str) -> Option<Change> {
    let b = byte(text, cursor)?;
    let line_start = text[..b].rfind('\n').map_or(0, |n| n + 1);
    let line_end = text[b..].find('\n').map_or(text.len(), |n| b + n);
    let blank = |line: Option<&str>| line.is_none_or(|l| l.trim().is_empty());
    let previous = text[..line_start]
        .strip_suffix('\n')
        .map(|t| t.rsplit('\n').next().unwrap_or(t));
    let next = text
        .get(line_end + 1..)
        .map(|t| t.split('\n').next().unwrap_or(t));
    let (range, before) = if text[line_start..line_end].trim().is_empty() {
        (
            line_start..line_end,
            if blank(previous) { "" } else { "\n" },
        )
    } else {
        (line_end..line_end, "\n\n")
    };
    let after = if blank(next) { "" } else { "\n" };
    let start = chars(text, range.start);
    Some(Change {
        edit: Edit {
            start,
            end: chars(text, range.end),
            text: format!("{before}{table}{after}"),
        },
        cursor: start + before.len() + 2,
    })
}

struct Table {
    /// Bytes from its first line's start to its last line's end.
    start: usize,
    end: usize,
    /// Spaces before each line (in a list item).
    indent: String,
    /// The header first; no delimiter row. All as long as `aligns`.
    rows: Vec<Vec<String>>,
    aligns: Vec<Align>,
}

/// A cell: its row (0 the header) and column, and where in it.
#[derive(Debug, Clone, Copy)]
struct Place {
    row: usize,
    column: usize,
    spot: Spot,
}

#[derive(Debug, Clone, Copy)]
enum Spot {
    /// Characters into the cell's text.
    Text(usize),
    /// Before the row's first pipe, after its last.
    Start,
    End,
}

/// Where a row of the formatted table is (bytes): its line's start, its cells' texts' starts,
/// its line's end.
type Row = (usize, Vec<usize>, usize);

impl Table {
    /// Rows padded with empty cells to the widest.
    fn new(indent: String, mut rows: Vec<Vec<String>>, mut aligns: Vec<Align>) -> Self {
        let n = rows
            .iter()
            .map(Vec::len)
            .chain([aligns.len(), 1])
            .max()
            .unwrap_or(1);
        for r in &mut rows {
            r.resize(n, String::new());
        }
        aligns.resize(n, Align::None);
        Self {
            start: 0,
            end: 0,
            indent,
            rows,
            aligns,
        }
    }

    /// The table with byte `at` in its lines, if the parser sees one there (not in a quote).
    fn find(text: &str, at: usize) -> Option<Self> {
        for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
            if range.start > at {
                return None;
            }
            let Event::Start(Tag::Table(_)) = event else {
                continue;
            };
            let start = text[..range.start].rfind('\n').map_or(0, |n| n + 1);
            let end = start + text[start..range.end].trim_end_matches(['\n', '\r']).len();
            if !(start..=end).contains(&at) {
                continue;
            }
            let indent = &text[start..range.start];
            if !indent.chars().all(|c| c == ' ' || c == '\t') {
                return None;
            }
            let lines: Vec<&str> = text[start..end].split('\n').collect();
            let texts = |line: &str| cells(line).into_iter().map(|c| c.text).collect::<Vec<_>>();
            let aligns = cells(lines.get(1)?)
                .iter()
                .map(|c| match (c.text.starts_with(':'), c.text.ends_with(':')) {
                    (true, true) => Align::Center,
                    (true, false) => Align::Left,
                    (false, true) => Align::Right,
                    (false, false) => Align::None,
                })
                .collect();
            let rows = [lines[0]]
                .iter()
                .chain(&lines[2..])
                .map(|l| texts(l))
                .collect();
            let mut t = Self::new(indent.to_string(), rows, aligns);
            (t.start, t.end) = (start, end);
            return Some(t);
        }
        None
    }

    /// The cell holding byte `at` of the text (in the table's lines); the delimiter row counts
    /// as the header's start.
    fn place(&self, text: &str, at: usize) -> Place {
        let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1).max(self.start);
        let line = &text[line_start..text[at..].find('\n').map_or(text.len(), |n| at + n)];
        let index = text[self.start..line_start].matches('\n').count();
        let at = at - line_start;
        let cells = cells(line);
        let column = cells
            .iter()
            .position(|c| at <= c.stretch.end)
            .unwrap_or(cells.len().saturating_sub(1))
            .min(self.aligns.len() - 1);
        let spot = match (cells.first(), cells.last()) {
            (Some(first), _) if at < first.stretch.start => Spot::Start,
            (_, Some(last)) if at > last.stretch.end => Spot::End,
            _ => Spot::Text(cells.get(column).map_or(0, |c| {
                line[c.text_start.min(at)..at]
                    .chars()
                    .count()
                    .min(c.text.chars().count())
            })),
        };
        match index {
            0 => Place {
                row: 0,
                column,
                spot,
            },
            1 => Place {
                row: 0,
                column,
                spot: Spot::Text(0),
            },
            i => Place {
                row: i - 1,
                column,
                spot,
            },
        }
    }

    /// The table's lines, aligned, and where its rows are in them.
    fn format(&self) -> (String, Vec<Row>) {
        let widths: Vec<usize> = (0..self.aligns.len())
            .map(|c| {
                self.rows
                    .iter()
                    .map(|r| width(&r[c]))
                    .max()
                    .unwrap_or(0)
                    .max(3)
            })
            .collect();
        let mut out = String::new();
        let mut starts = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let line_start = out.len();
            out.push_str(&self.indent);
            out.push('|');
            let mut s = Vec::new();
            for (c, cell) in row.iter().enumerate() {
                let pad = widths[c] - width(cell);
                let (left, right) = match self.aligns[c] {
                    Align::Right => (pad, 0),
                    Align::Center => (pad / 2, pad - pad / 2),
                    Align::None | Align::Left => (0, pad),
                };
                out.push_str(&" ".repeat(left + 1));
                s.push(out.len());
                out.push_str(cell);
                out.push_str(&" ".repeat(right + 1));
                out.push('|');
            }
            starts.push((line_start, s, out.len()));
            out.push('\n');
            if i == 0 {
                out.push_str(&self.indent);
                out.push('|');
                for (c, align) in self.aligns.iter().enumerate() {
                    let dashes = |n: usize| "-".repeat(widths[c] - n);
                    let cell = match align {
                        Align::None => dashes(0),
                        Align::Left => format!(":{}", dashes(1)),
                        Align::Right => format!("{}:", dashes(1)),
                        Align::Center => format!(":{}:", dashes(2)),
                    };
                    out.push_str(&format!(" {cell} |"));
                }
                out.push('\n');
            }
        }
        out.pop();
        (out, starts)
    }
}

/// A cell of a table line.
struct Cell {
    /// Between its pipes (bytes of the line).
    stretch: Range<usize>,
    /// Where its text starts, and the text, trimmed.
    text_start: usize,
    text: String,
}

/// The cells of a table line, split at the pipes that are not escaped (`\|`), the outer ones
/// optional.
fn cells(line: &str) -> Vec<Cell> {
    let bytes = line.as_bytes();
    let mut pipes = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'|' => pipes.push(i),
            _ => {}
        }
        i += 1;
    }
    let first = line.len() - line.trim_start().len();
    let last = line.trim_end().len();
    let mut from = first;
    if pipes.first() == Some(&first) {
        pipes.remove(0);
        from += 1;
    }
    let mut to = last;
    if last > from && pipes.last() == Some(&(last - 1)) {
        pipes.pop();
        to -= 1;
    }
    let mut out = Vec::new();
    for end in pipes.into_iter().chain([to]) {
        let raw = &line[from..end.max(from)];
        let text_start = from + raw.len() - raw.trim_start().len();
        out.push(Cell {
            stretch: from..end.max(from),
            text_start,
            text: raw.trim().to_string(),
        });
        from = end + 1;
    }
    out
}

/// How many columns `s` takes in a monospace font: two for East Asian wide characters and
/// emoji, none for combining marks and joiners.
// ponytail: the common ranges only, not Unicode's whole East Asian Width table; the
// `unicode-width` crate if a script lines up wrong.
fn width(s: &str) -> usize {
    s.chars()
        .map(|c| match c as u32 {
            0x0300..=0x036F | 0x200B..=0x200F | 0xFE00..=0xFE0F => 0,
            0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD => 2,
            _ => 1,
        })
        .sum()
}

/// Numbers by value, the rest alphabetically, ignoring case.
fn compare(a: &str, b: &str) -> Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

/// The byte of character `at` (the text's end after its last).
fn byte(text: &str, at: usize) -> Option<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .nth(at)
}

fn chars(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` (with `‸` where the cursor is) after `op` there, with `‸` where the cursor goes.
    fn after(text: &str, op: Op) -> Option<String> {
        let at = text[..text.find('‸').unwrap()].chars().count();
        let t = text.replace('‸', "");
        let c = apply(&t, at, at, op)?;
        let mut chars: Vec<char> = t.chars().collect();
        chars.splice(c.edit.start..c.edit.end, c.edit.text.chars());
        chars.insert(c.cursor, '‸');
        Some(chars.into_iter().collect())
    }

    #[test]
    fn tidy_lines_up_the_pipes() {
        assert_eq!(
            after(
                "Text\n\n| a | b‸b |\n|:-|-:|\n| ccc | d |\n\nMore\n",
                Op::Tidy
            )
            .unwrap(),
            "Text\n\n| a   |  b‸b |\n| :-- | --: |\n| ccc |   d |\n\nMore\n"
        );
        assert_eq!(
            after("a | b‸\n--|:-:\n1 | 22222\n", Op::Tidy).unwrap(),
            "| a   |   b‸   |\n| --- | :---: |\n| 1   | 22222 |\n"
        );
        // Escaped pipes stay in their cell, wide characters take two columns.
        assert_eq!(
            after("| `x\\|y` | 中文‸ |\n|---|---|\n| 🎉 | z |", Op::Tidy).unwrap(),
            "| `x\\|y` | 中文‸ |\n| ------ | ---- |\n| 🎉     | z    |"
        );
        assert_eq!(after("| a   |‸ b   |\n| --- | --- |", Op::Tidy), None);
        assert_eq!(after("Not a table | ‸\n", Op::Tidy), None);
        assert_eq!(after("> | a | b‸ |\n> |---|---|\n", Op::Tidy), None);
    }

    #[test]
    fn tidy_keeps_the_cursor_in_its_cell_or_out_of_the_table() {
        let t = "| a | b |\n|-|-|\n| 1 | 2 |\n\nx";
        let row = apply(t, 1, 22, Op::Tidy).unwrap();
        assert_eq!(row.edit.text, "| a   | b   |\n| --- | --- |\n| 1   | 2   |");
        assert_eq!(&row.edit.text[row.cursor..], "2   |");
        let below = apply(t, 1, t.chars().count(), Op::Tidy).unwrap();
        assert_eq!(below.cursor, row.edit.text.chars().count() + 3);
        // At a row's ends, past its pipes, it stays there.
        assert_eq!(
            after("| a | b |\n|-|-|\n| 1 | 2 |‸", Op::Tidy).unwrap(),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |‸"
        );
        assert_eq!(
            after("| a | b |\n|-|-|\n‸| 1 | 2 |", Op::Tidy).unwrap(),
            "| a   | b   |\n| --- | --- |\n‸| 1   | 2   |"
        );
    }

    #[test]
    fn rows_and_columns() {
        let t = "| a | b‸ |\n|---|--:|\n| 1 | 2 |\n| 3 | 4 |";
        assert_eq!(
            after(t, Op::RowBelow).unwrap(),
            "| a   |   b |\n| --- | --: |\n|     |    ‸ |\n| 1   |   2 |\n| 3   |   4 |"
        );
        assert_eq!(after(t, Op::RowAbove), None);
        assert_eq!(after(t, Op::DeleteRow), None);
        assert_eq!(after(t, Op::MoveRowDown), None);
        assert_eq!(
            after("| a | b |\n|---|--:|\n| 1 | 2 |\n| 3‸ | 4 |", Op::MoveRowUp).unwrap(),
            "| a   |   b |\n| --- | --: |\n| 3‸   |   4 |\n| 1   |   2 |"
        );
        assert_eq!(
            after("| a | b |\n|---|--:|\n| 1 | 2 |\n| 3‸ | 4 |", Op::DeleteRow).unwrap(),
            "| a   |   b |\n| --- | --: |\n| ‸1   |   2 |"
        );
        assert_eq!(
            after(t, Op::ColumnLeft).unwrap(),
            "| a   | ‸    |   b |\n| --- | --- | --: |\n| 1   |     |   2 |\n| 3   |     |   4 |"
        );
        assert_eq!(
            after(t, Op::MoveColumnLeft).unwrap(),
            "|   b‸ | a   |\n| --: | --- |\n|   2 | 1   |\n|   4 | 3   |"
        );
        assert_eq!(after(t, Op::MoveColumnRight), None);
        assert_eq!(
            after(t, Op::DeleteColumn).unwrap(),
            "| ‸a   |\n| --- |\n| 1   |\n| 3   |"
        );
        assert_eq!(
            after(t, Op::Align(Align::Center)).unwrap(),
            "| a   |  b‸  |\n| --- | :-: |\n| 1   |  2  |\n| 3   |  4  |"
        );
        assert_eq!(
            after(t, Op::DuplicateColumn).unwrap(),
            "| a   |   b |   b‸ |\n| --- | --: | --: |\n| 1   |   2 |   2 |\n| 3   |   4 |   4 |"
        );
        let body = "| a | b |\n|---|---|\n| 1‸ | 2 |\n| 3 | 4 |\n| 5 | 6 |";
        assert_eq!(
            after(body, Op::DuplicateRow).unwrap(),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n| 1‸   | 2   |\n| 3   | 4   |\n| 5   | 6   |"
        );
        assert_eq!(
            after(body, Op::MoveRow { to: 3 }).unwrap(),
            "| a   | b   |\n| --- | --- |\n| 3   | 4   |\n| 5   | 6   |\n| 1‸   | 2   |"
        );
        assert_eq!(after(body, Op::MoveRow { to: 0 }), None);
        assert_eq!(after(body, Op::MoveRow { to: 4 }), None);
        assert_eq!(
            after(
                "| a‸ | b | c |\n|---|---|--:|\n| 1 | 2 | 3 |",
                Op::MoveColumn { to: 2 }
            )
            .unwrap(),
            "| b   |   c | a‸   |\n| --- | --: | --- |\n| 2   |   3 | 1   |"
        );
    }

    #[test]
    fn layout_points_at_rows_and_columns() {
        let t = "Text\n\n  | a | bb |\n  |---|---|\n  | 1 | 2 |\n";
        assert_eq!(
            layout(t, 30).unwrap(),
            Layout {
                rows: vec![8, 33],
                edges: vec![8, 12, 17],
            }
        );
        assert_eq!(layout("a | b\n--|--\n1 | 2", 0).unwrap().edges, [0, 2, 5]);
        assert_eq!(layout(t, 0), None);
    }

    #[test]
    fn sorting_takes_numbers_by_value() {
        let t = "| n‸ |\n|---|\n| 10 |\n| b |\n| 9 |\n| A |";
        assert_eq!(
            after(t, Op::Sort { descending: false }).unwrap(),
            "| n‸   |\n| --- |\n| 9   |\n| 10  |\n| A   |\n| b   |"
        );
        assert_eq!(
            after(t, Op::Sort { descending: true }).unwrap(),
            "| n‸   |\n| --- |\n| b   |\n| A   |\n| 10  |\n| 9   |"
        );
    }

    #[test]
    fn new_tables_stand_apart() {
        let put = |text: &str, cursor: usize| {
            let c = insert(text, cursor, &empty()).unwrap();
            let mut chars: Vec<char> = text.chars().collect();
            chars.splice(c.edit.start..c.edit.end, c.edit.text.chars());
            chars.insert(c.cursor, '‸');
            chars.into_iter().collect::<String>()
        };
        let table = "|     |     |\n| --- | --- |\n|     |     |";
        assert_eq!(put("", 0), format!("| ‸{}", &table[2..]));
        assert_eq!(put("ab\nc", 1), format!("ab\n\n| ‸{}\n\nc", &table[2..]));
        assert_eq!(put("ab\n\nc", 3), format!("ab\n\n| ‸{}\n\nc", &table[2..]));
        assert_eq!(put("ab\n", 3), format!("ab\n\n| ‸{}", &table[2..]));
    }

    #[test]
    fn every_change_anywhere_in_the_reference_note_holds() {
        let t = include_str!("../tests/reference.md");
        let n = t.chars().count();
        let ops = [
            Op::Tidy,
            Op::RowAbove,
            Op::RowBelow,
            Op::DeleteRow,
            Op::MoveRowUp,
            Op::MoveRowDown,
            Op::ColumnLeft,
            Op::ColumnRight,
            Op::DeleteColumn,
            Op::MoveColumnLeft,
            Op::MoveColumnRight,
            Op::MoveRow { to: 2 },
            Op::MoveColumn { to: 0 },
            Op::DuplicateRow,
            Op::DuplicateColumn,
            Op::Align(Align::Right),
            Op::Sort { descending: true },
        ];
        let mut changed = 0;
        for at in (0..=n).step_by(3) {
            let keys = [Step::Next, Step::Previous, Step::Down].map(|s| step(t, at, s));
            let all = ops.iter().map(|&op| apply(t, at, at, op));
            for c in all.chain(keys).chain([start(t, at)]).flatten() {
                let len = n - (c.edit.end - c.edit.start) + c.edit.text.chars().count();
                assert!(c.edit.start <= c.edit.end && c.edit.end <= n && c.cursor <= len);
                changed += 1;
            }
        }
        assert!(changed > 100, "{changed}");
    }

    /// `text` (with `‸` where the cursor is) after `change` there, with `‸` where the cursor goes.
    fn changed(text: &str, change: impl Fn(&str, usize) -> Option<Change>) -> Option<String> {
        let at = text[..text.find('‸').unwrap()].chars().count();
        let t = text.replace('‸', "");
        let c = change(&t, at)?;
        let mut chars: Vec<char> = t.chars().collect();
        chars.splice(c.edit.start..c.edit.end, c.edit.text.chars());
        chars.insert(c.cursor, '‸');
        Some(chars.into_iter().collect())
    }

    #[test]
    fn tab_and_enter_go_through_the_cells() {
        let by = |text: &str, s: Step| changed(text, |t, at| step(t, at, s));
        let t = "| a | bb |\n|---|---|\n| 1 | 2 |\n\nMore";
        let at = |i: usize| {
            let mut s = t.to_string();
            s.insert(s.char_indices().nth(i).unwrap().0, '‸');
            s
        };
        // From the header's "a": its "bb", then row 1, at the end of each cell's text.
        assert_eq!(by(&at(2), Step::Next).unwrap(), at(8));
        assert_eq!(by(&at(8), Step::Next).unwrap(), at(24));
        assert_eq!(by(&at(24), Step::Previous).unwrap(), at(8));
        assert_eq!(by(&at(2), Step::Down).unwrap(), at(24));
        assert_eq!(by(&at(2), Step::Previous), None);
        // Enter at the header's start makes room above the table.
        assert_eq!(by(&at(0), Step::Down), None);
        // Past the last row a new one, tidied.
        let new = "| a   | bb  |\n| --- | --- |\n| 1   | 2   |\n";
        assert_eq!(
            by("| a | bb |\n|---|---|\n| 1 | 2‸ |\n\nMore", Step::Next).unwrap(),
            format!("{new}| ‸    |     |\n\nMore")
        );
        assert_eq!(
            by("| a | bb |\n|---|---|\n| 1 | 2‸ |", Step::Down).unwrap(),
            format!("{new}|     | ‸    |")
        );
        // Enter on an empty last row leaves the table.
        assert_eq!(
            by("| a | bb |\n|---|---|\n|  ‸ |  |\n\nMore", Step::Down).unwrap(),
            "| a | bb |\n|---|---|\n\n‸\n\nMore"
        );
        assert_eq!(by("Text‸\n", Step::Next), None);
    }

    #[test]
    fn a_header_line_starts_a_table() {
        let enter = |text: &str| changed(text, start);
        assert_eq!(
            enter("Text\n\n| Name | Value |‸\n").unwrap(),
            "Text\n\n| Name | Value |\n| ---- | ----- |\n| ‸     |       |\n"
        );
        assert_eq!(
            enter("  | a |‸\nMore").unwrap(),
            "  | a   |\n  | --- |\n  | ‸    |\n\nMore"
        );
        assert_eq!(
            enter("Text\n| a |‸").unwrap(),
            "Text\n| a   |\n| --- |\n| ‸    |"
        );
        assert_eq!(enter("| a |‸ x"), None);
        assert_eq!(enter("| a | b‸ |"), None);
        assert_eq!(enter("```\n| a |‸\n```"), None);
        assert_eq!(enter("| a |\n|---|\n| 1 |‸"), None);
    }

    #[test]
    fn spreadsheet_rows_make_a_table() {
        assert_eq!(
            from_tsv("Name\tSize\na|b\t10\n\n").unwrap(),
            "| Name | Size |\n| ---- | ---- |\n| a\\|b | 10   |"
        );
        assert_eq!(from_tsv("one\ttwo\n"), None);
        assert_eq!(from_tsv("a\tb\nno tab\n"), None);
    }
}
