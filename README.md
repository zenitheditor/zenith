# Zenith

> A design document format built for the age of AI agents.

Zenith is a plain-text format and engine for design files — posters, decks, books, social graphics, and more. The idea is simple: design should work the way code does. You should be able to read it, diff it, review it, test it, and let an agent safely edit it.

## Why

Code got source control, types, tests, and pull requests. Design files got none of that. They're opaque blobs — you can't diff them, you can't review a change, and the same file can render differently on different machines.

That's a problem for people, and it's a bigger problem for AI. Agents can already write code and open pull requests, because code is text they can read and reason about. Drop them into a design tool and they go blind. Ask an agent to "make the heading brand red and tighten the layout" and there's nothing safe to grab onto — no stable target, no validation, no preview, no way to check the result.

Zenith fixes that. The goal is to make design files as safe to automate as code:

- **Plain text** you own — readable, diffable, yours forever.
- **Stable IDs** so every change is a reviewable patch, not a mouse drag.
- **Deterministic rendering** — the same file always produces the same pixels.
- **Real validation** — text fits, colors come from the design system, nothing falls off the page.
- **Safe edits** — every change is checked, previewable, and logged before it lands.

## It's not AI image generation

This is the most common mix-up, so it's worth being blunt: Zenith is the opposite of an image model like Nano Banana, ChatGPT image, or Grok Imagine.

An image generator gives you a flat picture. It's a bag of pixels — you can't open it up and move the logo, you can't force the headline to use your exact brand color, and asking for "the same thing but with a different date" gives you a different image. There's nothing to edit, review, or guarantee.

Zenith doesn't generate a picture. It generates the _design itself_ — a structured, editable document where every element is real and addressable. An agent (or a person) can change one line, swap a color token, or regenerate a hundred on-brand variants, and every render is exact and repeatable. AI writes and edits the source; Zenith guarantees what it means and how it looks.

## Agent-native first, not a tool with an API bolted on

Most design tools are built for a human dragging boxes, and an automation API gets added later as an afterthought — a thin, limited layer over a model that was never meant to be driven by software.

Zenith is built the other way around. The foundation is a programmatic, text-based, deterministic engine. Agents, scripts, and the command line drive it directly. A visual editor for humans is a client on top of that same engine — not the other way around. So automation isn't a side door; it's the front door.

People are still first-class users. "Agent-first" means the safe, scriptable core comes first, and everything else is built on it.

## Works in your repo

Because a Zenith file is just text, it lives wherever your code lives. Commit it to git. Review design changes in a pull request, side by side with the diff. Render it in CI to catch a broken layout before it ships. Generate variants in a pipeline. Roll back like any other file. Design stops being a separate world you export to and from, and becomes part of the build.

## Who it's for

- **AI and agent builders** who need to generate and edit visuals reliably, not by screenshot-and-pray.
- **Engineering teams** who want design assets in the repo, reviewed in PRs, and built in CI.
- **High-volume producers** — marketing, publishing, localization — who need lots of correct variants.
- **Tool builders** who'd rather build on an open format than a closed cloud API.

## Showcase

The public showcase lives at [`farhan-syah/zenith-showcase`](https://github.com/farhan-syah/zenith-showcase) and is linked here as the [`zenith-showcase`](./zenith-showcase) submodule.

It is the place for reusable Zenith examples: `.zen` source, rendered outputs, visual recipes, actions, filters, backgrounds, posters, flyers, books, magazines, ads, diagrams, presentations, and other generated design work.

Only put files in the showcase if you have the rights to share them and you allow others to reuse the submitted source, outputs, and assets under the declared license. Private, client, portfolio-only, or custom-licensed work should be linked from the showcase's external gallery instead; licensing for external work stays with the owner.

## Status

Early days. Zenith isn't ready to use yet — the format and tools are still being designed, nothing is stable, and there's no build to download. This README will grow with the project.
