# Issue tracker: Local Markdown

Issues and specs for this solo repository live as Markdown files in `.scratch/`.

## Conventions

- One effort per directory: `.scratch/<effort>/`.
- A Wayfinder map is `.scratch/<effort>/map.md`.
- Child or implementation tickets are one file each under `.scratch/<effort>/issues/` and are numbered in dependency order.
- `Type:`, `Status:`, and `Blocked by:` lines carry workflow metadata.
- Comments and resolution evidence append under explicit headings.

## Wayfinding operations

A ticket is on the frontier when it is open, unclaimed, and every listed blocker is resolved. Claim by setting `Status: claimed` before work. Resolve by appending `## Answer`, setting `Status: resolved`, and adding a named context pointer to the map.
