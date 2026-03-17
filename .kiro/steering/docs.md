# Documentation Maintenance

When building a new feature or making significant changes, update the relevant documentation in `docs/` as part of the work.

## What to Update

- `docs/api/` - If new Python classes, methods, or modules are added or changed.
- `docs/user-guide/` - If user-facing behavior changes or new workflows are introduced.
- `docs/design/` - If architectural decisions or design patterns are added or modified.
- `docs/roadmap/` - If a roadmap item is completed or new items are planned.
- `docs/internal/` - For bug investigations, implementation notes, or TODO tracking.

## Guidelines

- Keep docs in Markdown (MyST-compatible). Use existing files as style reference.
- Add new pages to the appropriate `index.md` toctree so Sphinx picks them up.
- For API docs, prefer autodoc directives that pull from Python docstrings. Add hand-written context where autodoc alone is insufficient.
- Internal notes (`docs/internal/`) are not published but should still be clear and useful for future developers.
