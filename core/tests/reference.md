---
title: Markdown reference
tags: [notav, test]
---

# Markdown reference

Every piece of GitHub Flavored Markdown (with footnotes and Obsidian's front matter) that Notav's
Editor mode styles, to check them by eye. The core's tests parse this note whole.

## Headings

# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6

## Closed heading ##

Underlined heading 1
====================

Underlined heading 2
--------------------

## Emphasis

Plain, *emphasis*, _emphasis_, **strong**, __strong__, ***both***, **strong with _emphasis_
inside**, ~~strikethrough~~, ~single tilde~ and `inline code`. Code with a backtick inside:
`` a ` b ``. Escaped marks stay plain: \*not emphasis\*, \_not either\_, \`not code\`.

Intra_word_underscores stay plain, but intra*word*stars do not.

Polish letters keep their places: zażółć **gęślą** jaźń, and emoji too: 🎉 *party* 🎉.

## Links

An [inline link](https://example.com), one [with a title](https://example.com "Title"),
a [reference link][ref], a [collapsed one][] and a [shortcut one].
An autolink <https://example.com> and an e-mail one <user@example.com>.
Bare URLs: https://example.com/path?query=1, www.example.com and http://example.com/a_(b).
Not a URL: example.com, ftp://example.com.

[ref]: https://example.com/reference
[collapsed one]: https://example.com/collapsed "Collapsed"
[shortcut one]: <https://example.com/shortcut>

## Images

![An image](attachments/picture.png "Its title") and an inline one ![icon](icon.svg) in text.

## Lists

- Bullet
- Bullet with **strong** text
  - Nested bullet
    - Deeper bullet
* Star bullet
+ Plus bullet

1. First
2. Second
   1. Nested number
   2. Another
10. Tenth, a wider number

3) Parenthesis numbers
4) Like this

- A loose item

- With a paragraph between

  And a second paragraph in it.

## Tasks

- [ ] To do
- [x] Done
- [X] Done with a capital X
  - [ ] A nested task
1. [ ] A numbered task

## Quotes

> A quote with *emphasis*
> over two lines.
>
> > A nested quote.
>
> - A list in a quote
> - [ ] And a task

> [!NOTE]
> A GitHub alert.

## Code

```rust
fn main() {
    println!("Hello, **not bold**");
}
```

~~~
A tilde fence with ``` inside.
~~~

    An indented code block.

```
An empty info string.
```

## Tables

| Left | Centre | Right |
|:-----|:------:|------:|
| a | b | c |
| **strong** | `code` | [link](https://example.com) |
| Zażółć | 🎉 | escaped \| pipe |

Without outer pipes:

Name | Value
--- | ---
one | 1
two | 2

## Footnotes

A sentence with a footnote[^1] and another[^note].

[^1]: The first footnote.
[^note]: A named one, over
    two lines.

## Rules and breaks

Above a rule.

---

***

___

A hard break with a backslash\
and one with two spaces  
end here.

## HTML

<details>
<summary>Folded</summary>

Hidden text.

</details>

Inline <kbd>Ctrl</kbd>+<kbd>C</kbd> and a <br> break.

<!-- A comment -->

## Edge cases

**Unclosed strong and `unclosed code

A line with only spaces follows:
   
#Not a heading (no space)

1986\. Not a list.

- 

The empty item above has no text.
