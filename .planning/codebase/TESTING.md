# Testing Patterns

**Analysis Date:** 2026-02-13

## Test Framework

**Runner:**
- Node.js built-in `node:test` module (no external test framework)
- Version: Node.js 18+ (uses `require('node:test')` syntax)
- Config: No config file - tests run directly via `node gsd-tools.test.js`

**Assertion Library:**
- Node.js built-in `node:assert` module
- Methods: `assert.ok()`, `assert.deepStrictEqual()`, `assert.strictEqual()`

**Run Commands:**
```bash
node gsd-tools.test.js              # Run all tests
node gsd-tools.test.js --reporter=spec  # With reporter (if supported)
```

## Test File Organization

**Location:**
- Co-located with source: `gsd-tools.js` has `gsd-tools.test.js` in same directory
- Path: `.claude/get-shit-done/bin/gsd-tools.test.js`
- Only the main tools file has a test file (other CLI scripts lack tests)

**Naming:**
- Test file: `{source-file}.test.js`
- Test suite names match command being tested: `'history-digest command'`, `'state-load command'`
- Individual test names are descriptive: `'empty phases directory returns empty array'`

**File Structure:**
```
.claude/get-shit-done/bin/
├── gsd-tools.js          # Main CLI tool (4500+ lines)
├── gsd-tools.test.js     # Test suite (2000+ lines)
├── gsd-statusline.js     # Status line renderer (no test file)
└── gsd-check-update.js   # Version checker (no test file)
```

## Test Structure

**Suite Organization:**
```javascript
describe('history-digest command', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = createTempProject();
  });

  afterEach(() => {
    cleanup(tmpDir);
  });

  test('empty phases directory returns valid schema', () => {
    // Test implementation
  });

  test('nested frontmatter fields extracted correctly', () => {
    // Test implementation
  });
});
```

**Patterns:**
- Setup in `beforeEach()`: Create temporary project structure
- Teardown in `afterEach()`: Clean up temp files
- Each test is independent and isolated
- Shared helpers (runGsdTools, createTempProject, cleanup) defined at top level

**Setup/Teardown:**
```javascript
function createTempProject() {
  const tmpDir = fs.mkdtempSync(path.join(require('os').tmpdir(), 'gsd-test-'));
  fs.mkdirSync(path.join(tmpDir, '.planning', 'phases'), { recursive: true });
  return tmpDir;
}

function cleanup(tmpDir) {
  fs.rmSync(tmpDir, { recursive: true, force: true });
}
```

## Test Structure (Detailed)

**Typical Test Pattern:**
```javascript
test('test name describes expected behavior', () => {
  // 1. ARRANGE - Set up test data
  const result = runGsdTools('command args', tmpDir);

  // 2. ACT - (usually combined with arrange in CLI testing)

  // 3. ASSERT - Verify result
  assert.ok(result.success, 'should succeed');
  assert.deepStrictEqual(result.output, expected, 'output matches');
});
```

## Test Execution Pattern

**CLI Testing Approach:**
```javascript
// Helper spawns actual CLI command as subprocess
function runGsdTools(args, cwd = process.cwd()) {
  try {
    const result = execSync(`node "${TOOLS_PATH}" ${args}`, {
      cwd,
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    return { success: true, output: result.trim() };
  } catch (err) {
    return {
      success: false,
      output: err.stdout?.toString().trim() || '',
      error: err.stderr?.toString().trim() || err.message,
    };
  }
}
```

**File System Isolation:**
- Each test creates temporary directory in system tmpdir: `fs.mkdtempSync(path.join(require('os').tmpdir(), 'gsd-test-'))`
- Tests write actual files to temp directories (integration-style testing)
- Full cleanup in afterEach ensures no artifact leakage

## Mocking

**Framework:** No mocking framework used (no jest.mock, sinon, etc.)

**Patterns:**
- File system operations use real temporary directories
- Child processes invoked via `execSync` (synchronous, blocking)
- No stubbing of functions or modules

**What to Mock:**
- External API calls (not done in current tests)
- Network operations (not done in current tests)
- Date/time operations (not mocked currently)

**What NOT to Mock:**
- File system operations (use temp directories instead)
- Child process execution (tests spawn actual CLI)
- Module loading (tests run entire CLI as subprocess)

**Approach:**
- Prefer integration-style testing where entire command runs end-to-end
- Create minimal file structure for each test
- Verify actual output/side effects rather than mocking internals

## Fixtures and Factories

**Test Data Pattern:**
```javascript
// Create phase directory with SUMMARY containing nested frontmatter
const phaseDir = path.join(tmpDir, '.planning', 'phases', '01-test');
fs.mkdirSync(phaseDir, { recursive: true });

const summaryContent = `---
phase: "01"
name: "Foundation Setup"
dependency-graph:
  provides:
    - "Database schema"
---

# Summary content here
`;

fs.writeFileSync(path.join(phaseDir, '01-01-SUMMARY.md'), summaryContent);
```

**Factory Functions:**
```javascript
function createTempProject() {
  const tmpDir = fs.mkdtempSync(path.join(require('os').tmpdir(), 'gsd-test-'));
  fs.mkdirSync(path.join(tmpDir, '.planning', 'phases'), { recursive: true });
  return tmpDir;
}
```

**Location:**
- Helpers defined at top of test file
- Fixture data embedded in test bodies (not separate fixture files)
- Templates created dynamically as needed

## Assertions

**Assertion Methods Used:**
- `assert.ok(value, message)` - Verify truthy condition
- `assert.deepStrictEqual(actual, expected, message)` - Deep equality check
- `assert.strictEqual(a, b, message)` - Strict equality (===)

**Assertion Patterns:**
```javascript
// Command execution success
assert.ok(result.success, `Command failed: ${result.error}`);

// Data structure equality
assert.deepStrictEqual(digest.phases, {}, 'phases should be empty object');

// Array membership
assert.ok(
  digest.decisions.some(d => d.decision === 'Use Prisma over Drizzle'),
  'Should contain first decision'
);

// Array element match
assert.deepStrictEqual(
  digest.phases['01'].provides.sort(),
  ['Auth system', 'Database schema'],
  'provides should contain nested values'
);

// Strict value checking
assert.strictEqual(digest.decisions.length, 2, 'Should have 2 decisions');
```

## Coverage

**Requirements:** No coverage requirements enforced

**Approach:**
- Tests focus on command-line interface (inputs and outputs)
- Major commands have test suites (40+ test cases in gsd-tools.test.js)
- Coverage not measured or reported
- Test suite extensive but informal

**Test Scope:**
```javascript
describe('history-digest command', () => { /* 5 tests */ });
describe('phases list command', () => { /* 3 tests */ });
describe('roadmap get-phase command', () => { /* 4 tests */ });
describe('phase next-decimal command', () => { /* 4 tests */ });
describe('phase-plan-index command', () => { /* 5 tests */ });
describe('state-snapshot command', () => { /* 4 tests */ });
describe('summary-extract command', () => { /* 4 tests */ });
describe('init commands with --include flag', () => { /* 5 tests */ });
describe('roadmap analyze command', () => { /* 2 tests */ });
describe('phase add command', () => { /* 1 test */ });
describe('phase insert command', () => { /* 2 tests */ });
describe('phase remove command', () => { /* 3 tests */ });
describe('phase complete command', () => { /* 2 tests */ });
describe('milestone complete command', () => { /* 2 tests */ });
describe('validate consistency command', () => { /* 2 tests */ });
describe('progress command', () => { /* 3 tests */ });
describe('todo complete command', () => { /* 2 tests */ });
describe('scaffold command', () => { /* 3 tests */ });
```

## Test Types

**Unit Tests:**
- Scope: Individual command handlers (e.g., `history-digest`, `state-load`)
- Approach: Spawn CLI process with specific arguments
- Environment: Isolated temp file system
- Verification: Check JSON output, file creation, error handling

**Integration Tests:**
- Scope: Full command execution with file system side effects
- Approach: Create fixtures, run command, verify artifacts
- Examples: `phase add` creates directories and updates ROADMAP.md, `phase complete` transitions state
- Pattern: Most tests in the suite follow integration style

**E2E Tests:**
- Framework: Not explicitly separated (all tests are end-to-end in nature)
- Approach: Real CLI execution via `execSync`
- Coverage: Commands invoked exactly as users would use them

## Common Patterns

**Async Testing:**
- No async operations in tests (file system is synchronous)
- `execSync` handles process spawning synchronously
- Tests are inherently serial (one after another)

**Error Testing:**
```javascript
test('rejects removal of phase with summaries unless --force', () => {
  // Create phase with summary
  const p1 = path.join(tmpDir, '.planning', 'phases', '01-test');
  fs.mkdirSync(p1, { recursive: true });
  fs.writeFileSync(path.join(p1, '01-01-SUMMARY.md'), `---\nphase: "01"\n---\n`);

  // Attempt removal without --force
  const result = runGsdTools('phase remove 01', tmpDir);
  assert.ok(!result.success, 'Should fail without --force');
  assert.ok(result.error.includes('completed'), 'Error should mention completion');
});
```

**Success/Failure Verification:**
```javascript
// Verify success
assert.ok(result.success, `Command failed: ${result.error}`);

// Parse JSON output
const digest = JSON.parse(result.output);

// Verify data
assert.deepStrictEqual(digest.phases['01'].provides, ['Direct provides']);
```

**File System State:**
```javascript
// Create test state
fs.mkdirSync(path.join(tmpDir, '.planning', 'phases', '01-test'), { recursive: true });

// Run command
const result = runGsdTools('history-digest', tmpDir);

// Verify output matches file system
assert.ok(digest.phases['01'], 'Phase should be indexed');
```

## Test Data Examples

**Frontmatter Format:**
```javascript
const summaryContent = `---
phase: "01"
name: "Foundation"
provides:
  - "Database schema"
  - "Auth system"
dependency-graph:
  affects:
    - "API layer"
key-decisions:
  - "Use Prisma over Drizzle"
patterns-established:
  - "Repository pattern"
---

# Summary content
`;
```

**Directory Structure:**
```
tmpDir/
├── .planning/
│   ├── phases/
│   │   ├── 01-foundation/
│   │   │   ├── 01-01-SUMMARY.md
│   │   │   └── 01-01-PLAN.md
│   │   └── 02-api/
│   │       ├── 02-01-SUMMARY.md
│   │       └── 02-01-PLAN.md
│   ├── ROADMAP.md
│   └── STATE.md
└── (other project files)
```

---

*Testing analysis: 2026-02-13*
