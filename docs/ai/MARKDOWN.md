# Markdown Formatting Guide

When writing Markdown code, follow these formatting rules:

**Line Length:**

- Wrap lines to stay under 100 characters total (including indentation)
- Break at word boundaries (spaces), not in the middle of words

**Bullet List Indentation:**

- First-level bullets: Use `- ` (dash + space) at the start of the line
- Second-level (nested) bullets: Use 4 spaces of indentation, then `- ` (dash + space)
- Third-level bullets (if needed): Use 8 spaces of indentation, then `- ` (dash + space)

**Continuation Lines (when text wraps):**

- Continuation lines must align with the first character of text after the bullet marker
- For first-level bullets (`- text`): Continuation aligns at column 2 (under the 't' in 'text')
- For second-level bullets (`    - text`): Continuation aligns at column 6 (under the 't' in
  'text')
- For third-level bullets (`        - text`): Continuation aligns at column 10

**Example:**

```markdown
- First-level bullet with a long line that wraps around to the next line, and the continuation
  aligns at column 2, directly under the first character of the bullet's text content.
    - Second-level nested bullet with a long line that also wraps around and needs multiple lines
      to display all of its content, with continuation lines aligning at column 6 under the first
      character.
    - Another second-level bullet showing the 4-space indentation before the dash and space.
        - Third-level bullet demonstrating 8 spaces of indentation before the dash, with any
          continuation lines aligning at column 10.
- Back to first-level bullet.
```

**Summary of Indentation Rules:**

- Level 1: Start at column 0, continuations at column 2
- Level 2: Start at column 0 with 4 spaces, continuations at column 6
- Level 3: Start at column 0 with 8 spaces, continuations at column 10

**Links:**

- When a link URL will not fit within the 100 character line limit, prefer using reference-style
  links with the link definitions placed at the bottom of the document
- Place `<!-- @formatter:off -->` immediately above reference-style link definitions to prevent
  the markdown formatter from adding extra newlines between them
- **CRITICAL**: Reference-style link definitions should NEVER have a visible header like "##
  Sources" or "## Links". The `<!-- @formatter:off -->` comment should appear immediately after the
  last visible content, followed directly by the link definitions. The link definitions are
  invisible/hidden infrastructure, not a visible section of the document.

**Example:**

```markdown
- [ ] [Some interesting thing] - How some interesting thing accelerates multiplexed I/O.

<!-- @formatter:off -->

[Solarflare Onload with Multiplexing]: https://medium.com/@sgn00/accelerating-linux-pipes-with-some-interesting-thing-9c17ba9eb36b
```

**WRONG - Do NOT do this:**

```markdown
- [ ] [Some interesting thing] - How some interesting thing accelerates multiplexed I/O.

## Sources

<!-- @formatter:off -->

[Solarflare Onload with Multiplexing]: https://medium.com/@sgn00/accelerating-linux-pipes-with-some-interesting-thing-9c17ba9eb36b
```

Do not use the tilde symbol, `~` to say "approximately" or "approx" because Markdown editors treat
it as one of an unclosed pair and highlight it as bad Markdown syntax.
