# Coding Conventions

**Analysis Date:** 2026-02-13

## Naming Patterns

**Files:**
- Kebab-case with descriptive suffixes: `gsd-tools.js`, `gsd-tools.test.js`
- Test files append `.test.js` suffix: `gsd-tools.test.js`
- Workflow/command files use kebab-case: `gsd-statusline.js`, `gsd-check-update.js`
- Commands organized as `gsd-[purpose].js` following functional domain pattern

**Functions:**
- camelCase for all function names: `runGsdTools()`, `createTempProject()`, `cmdGenerateSlug()`
- Command functions prefixed with `cmd`: `cmdHistoryDigest()`, `cmdStateLoad()`, `cmdRoadmapGetPhase()`
- Helper functions use descriptive verbs: `safeReadFile()`, `loadConfig()`, `extractFrontmatter()`
- Abbreviations in names are lowercase: `parseIncludeFlag()` not `parseIncludeFLAG()`

**Variables:**
- camelCase for all variables: `includeValue`, `phaseDir`, `summaryContent`, `tmpDir`
- Constants use UPPER_SNAKE_CASE: `MODEL_PROFILES`, `FRONTMATTER_SCHEMAS`, `TOOLS_PATH`
- Single-letter or very short identifiers used in loops: `e`, `f`, `d` for entries/files/dirs
- Descriptive prefix for temporary/intermediate variables: `tmpDir`, `prevState`, `newValue`

**Types/Objects:**
- Object keys use camelCase: `success`, `error`, `mtime`, `update_available`
- Configuration keys use snake_case: `model_profile`, `commit_docs`, `search_gitignored`
- JSON output preserves snake_case for backward compatibility

## Code Style

**Formatting:**
- No explicit formatter configured (prettier/eslint)
- Consistent 2-space indentation used throughout
- Lines typically kept under 100 characters
- Function bodies use consistent spacing (blank lines separate logical sections)
- Comments preceded by blank line for clarity

**Linting:**
- No linter rules enforced (no .eslintrc found)
- Code follows Node.js standard library conventions
- Node.js built-ins used directly (no wrapper libraries)

**Indentation & Spacing:**
```javascript
// Standard 2-space indentation
function example() {
  const value = 'test';
  if (condition) {
    doSomething();
  }
  return value;
}
```

## Import Organization

**Order:**
1. Built-in Node.js modules: `const fs = require('fs');`
2. Absolute module paths from current package
3. Test framework imports for .test.js files: `const { test, describe } = require('node:test');`

**Pattern:**
```javascript
// gsd-tools.js
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const MODEL_PROFILES = { ... };

// ─── Helper functions ───────────────────
```

**Test File Pattern:**
```javascript
// gsd-tools.test.js
const { test, describe, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
```

**Section Markers:**
- Horizontal dividers using dashes and emojis for clarity: `// ─── Helpers ──────────────────────────────────────────────────────────────────`
- Major section headers use comment block: `// ─── [Section Name] ──────`
- Separates concerns within single file (helpers, constants, commands)

## Error Handling

**Patterns:**
- Try-catch blocks wrap file system and external command operations
- Silent failures for non-critical operations: `} catch {}`
- Explicit error messaging for command-line operations: `error(message)` calls
- Early returns for validation: `if (!value) return null;`
- Process exit codes: `process.exit(0)` for success, `process.exit(1)` for errors

**Error Function Pattern:**
```javascript
function error(message) {
  process.stderr.write('Error: ' + message + '\n');
  process.exit(1);
}
```

**File Read Pattern:**
```javascript
function safeReadFile(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf-8');
  } catch {
    return null;
  }
}
```

**Validation Pattern:**
```javascript
if (!text) {
  error('text required for slug generation');
}
```

## Logging

**Framework:** No logging framework - uses `process.stdout.write()` and `process.stderr.write()` directly

**Patterns:**
- Success output to stdout with JSON formatting: `process.stdout.write(JSON.stringify(result, null, 2))`
- Errors written to stderr: `process.stderr.write('Error: ' + message + '\n')`
- Raw output mode option for piping: `output(result, raw, rawValue)`
- No timestamps in logging - commands are typically short-lived

**Output Pattern:**
```javascript
function output(result, raw, rawValue) {
  if (raw && rawValue !== undefined) {
    process.stdout.write(String(rawValue));
  } else {
    process.stdout.write(JSON.stringify(result, null, 2));
  }
  process.exit(0);
}
```

## Comments

**When to Comment:**
- Above complex algorithms or parsing logic: YAML/frontmatter extraction
- Clarify non-obvious intent: "Check project directory first (local install), then global"
- Document side effects: "Runs once per session", "Background process"
- Explain regex patterns: `// Match list items at 6-space indent`
- Section headers for file organization

**JSDoc/TSDoc:**
- Not used in this codebase
- File headers use multi-line comments describing purpose: `/** GSD Tools — CLI utility... */`
- Command documentation in comment blocks at top of main file

## Function Design

**Size:** Functions typically 20-100 lines, some command handlers larger (200-300 lines)

**Parameters:**
- Avoid long parameter lists (max 3-4 params)
- Bundle related options into object: `cmdScaffold(cwd, scaffoldType, scaffoldOptions, raw)`
- Path operations consistently use `cwd` first parameter for context

**Return Values:**
- Functions return objects with `success`, `output`, `error` shape for CLI operations
- Return `null` or empty arrays `[]` for missing/empty results
- Functions exit via `process.exit()` rather than returning exit codes

**Side Effects:**
- File system operations are primary (read/write)
- Process exits are intentional (command-line tool pattern)
- Child processes spawned with `stdio: 'ignore'` for background operations

## Module Design

**Exports:**
- `gsd-tools.js` is a CLI utility - no explicit module.exports
- Execution driven by main command dispatch at end of file
- All helper/command functions are file-scoped
- Test file (`gsd-tools.test.js`) directly executes the CLI via spawned process

**Single Responsibility:**
- `gsd-tools.js`: Command dispatch, state/file operations (4500+ lines, monolithic utility)
- `gsd-statusline.js`: Parse JSON input, render status line with context usage
- `gsd-check-update.js`: Background version check, write cache file

**CLI Pattern:**
```javascript
// Extract subcommand from args
const command = args[0];

// Dispatch to handler
switch (command) {
  case 'state':
    const subcommand = args[1];
    if (subcommand === 'load') {
      cmdStateLoad(cwd, raw);
    }
    break;
  default:
    error(`Unknown command: ${command}`);
}
```

## Data Structures

**Configuration Objects:**
```javascript
const defaults = {
  model_profile: 'balanced',
  commit_docs: true,
  search_gitignored: false,
  branching_strategy: 'none',
};
```

**Result Objects:**
```javascript
return {
  success: true,
  output: result.trim(),
};
```

**Frontmatter Data:**
- YAML parsed into nested objects
- Arrays preserved as arrays: `provides: ['Feature A', 'Feature B']`
- Nested structures flattened for digest: `dependency-graph.provides` → `provides` array

---

*Convention analysis: 2026-02-13*
