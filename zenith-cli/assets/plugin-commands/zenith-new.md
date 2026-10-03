---
description: Scaffold a new .zen design document from a brief (validates + renders a first preview).
argument-hint: "[brief, e.g. 'square instagram promo for a coffee launch']"
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Glob
---

Create a new Zenith design document for: **$ARGUMENTS**

Follow the `zenith` skill. Steps:

1. Match the brief to a section of `references/by-kind.md`. Use the built-ins it names.
2. If `.zenith/brand.md` exists in or above the directory, use its tokens.
3. Else run `zenith new <path> --theme <name>` plus the recipe's canvas flags.
4. Author with layout frames and theme defaults. Set `style="ui.*"` for roles.
5. Run `zenith validate <file> --json`, then `zenith fix <file> --apply`. Fix what remains.
6. Run `zenith render <file> --contact-sheet <file>.png --scale 0.5`. Open the PNG and fix what looks wrong.
7. Report the output path.

Never invent syntax. Check it with `zenith schema node <kind>` and `zenith <cmd> --help`.
