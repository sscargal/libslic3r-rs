# Claude Code Instructions for libslic3r-rs

## Git Workflow

### Branching Strategy
- Create a new branch for each phase: `phase-XX-description`
- Create a new branch for each milestone when starting milestone work
- Example: `phase-01-foundation-types`, `milestone-v1.0`

### Commit Configuration
- **NEVER use GPG signing** - it requires human password entry which breaks automation
- GPG signing is disabled for this repo via `git config commit.gpgsign false`
- If a commit fails with GPG errors, disable signing before retrying

### Commit Messages
- Follow conventional commits format: `type(scope): description`
- Always include Co-Authored-By line for Claude contributions
- Example:
  ```
  docs(01): capture phase context

  Detailed description of changes.

  Co-Authored-By: Claude Sonnet 4.5 (1M context) <noreply@anthropic.com>
  ```

## Project Context

This is a Rust-based 3D printer slicing engine built from scratch with:
- Modular architecture (plugin system)
- AI integration for print profile suggestions
- Pure Rust ecosystem (no C/C++ FFI)
- WASM compatibility from day one

See `.planning/PROJECT.md` for full project context and design decisions.

## GSD Workflow

This project uses the Get Shit Done (GSD) workflow:
- `/gsd:progress` - Check current status
- `/gsd:discuss-phase N` - Gather context before planning
- `/gsd:plan-phase N` - Create execution plan
- `/gsd:execute-phase N` - Execute all plans in phase

All GSD state is tracked in `.planning/` directory.
