# Claude Code Instructions for SPAA Tools

SPAA is a file format for storing profile traces. It is designed to pre-compute and organize information that is useful for LLM analysis of performance traces. It is designed to take input data from Linux `perf` and DTrace, as well as memory profilers, and turn it into data that can be queried.

All details of the file spec are stored in `SPEC.md`. **ALWAYS** consult that file for details on the file format and how it should be used.

All tools related to the SPAA format are implemented in Rust. Each tool has its own crate in this directory.

## Code Quality

1. **Use Rust idioms**
   - Prefer `Option<T>` over nullable values
   - Use `Result<T, E>`  for operations that can fail
   - Leverage pattern matching
   - Use iterator methods where appropriate

2. **Documentation**:
   - Add doc comments (`///`) for public functions and structs
   - Include examples in doc comments *when helpful*
   - Document invariants and safety requirements for unsafe code
   - Comment non-obvious algorithm choices

3. **Error handling**
   - Use `panic!` only for unrecoverable errors
   - Provide clear error messages with context

## Testing Strategy

### Test Requirements

1. Write tests as you implement
3. Run tests frequently:
   ```bash
   cargo test           # All tests
   cargo test test_name # Specific test
   ```

4. One behavior, one test. Each test should cover one expected behavior. Different cases should get their own tests.

## Building and Running

### Development Build
```bash
cargo build
cargo run
```

### Running Tests
```bash
cargo test
cargo test -- --nocapture  # show println! output

## Git Workflow

### Commit Standards

**CRITICAL**: Make incremental, atomic commits that represent single logical
changes. Combine changes with tests where appropriate.

#### Good Commit Examples:
```
✓ Add thread info struct and parsing
✓ Implement transform for perf data format
✓ Add lookup cache
```

#### Bad Commit Examples:
```
✗ Implement all of Phase 1
✗ Fix stuff
✗ WIP
✗ Updates
```

### Commit Guidelines

1. **One complete logical change per commit**
   - Adding a single struct and its use case
   - Fixing a specific bug
   - Adding a specific test

2. **Commit message format**:
   ```
   <type>: <concise description>
   
   Optional longer description as needed, to explain why the change was made
   or any important details.
   ```

3. **Commit types**
   - `feat`: New feature or functionality
   - `fix`: Bug fix
   - `refactor`: Code restructuring without behavior changes
   - `test`: Adding or updating tests
   - `docs`: Documentation changes
   - `perf`: Performance improvements
   - `style`: Code style/formatting changes

### Before Each Commit

- Run `cargo check` to ensure code compiles
- Run `cargo test` to make sure tests still pass, even if tests don't cover the
  area that was changed
- Run `cargo fmt` to format code, if any Rust files were modified
- Verify your change is complete and functional

### After Each Commit

- Run `git status` and make sure there are no uncommitted changes. If there are,
add them to the most recent commit or create a new commit as appropriate.
