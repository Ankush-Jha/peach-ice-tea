# peach_select

A centralized crate for user interaction prompts using dialoguer.

## Purpose

This crate provides a unified interface for terminal user interactions across the peach codebase. It encapsulates all direct dependencies on `dialoguer`, ensuring no other crates need to depend on it directly.

## Features

- **Select prompts**: Choose from a list of options
- **Confirm prompts**: Yes/no questions
- **Input prompts**: Text input from user
- **Multi-select prompts**: Choose multiple options from a list
- **Consistent theming**: All prompts use a unified color scheme
- **Error handling**: Graceful handling of user interruptions

## Usage

### Select from options

```rust
use peach_select::PeachSelect;

let options = vec!["Option 1", "Option 2", "Option 3"];
let selected = PeachSelect::select("Choose an option:", options)
    .with_starting_cursor(1)
    .prompt()?;
```

### Confirm (yes/no)

```rust
use peach_select::PeachSelect;

let confirmed = PeachSelect::confirm("Are you sure?")
    .with_default(true)
    .prompt()?;
```

### Text input

```rust
use peach_select::PeachSelect;

let name = PeachSelect::input("Enter your name:")
    .allow_empty(false)
    .with_default("John")
    .prompt()?;
```

### Multi-select

```rust
use peach_select::PeachSelect;

let options = vec!["Red", "Green", "Blue"];
let selected = PeachSelect::multi_select("Choose colors:", options)
    .prompt()?;
```

## Design

### Builder Pattern

All prompt types use a builder pattern for configuration:
- Create the builder with `PeachSelect::select()`, `PeachSelect::confirm()`, etc.
- Configure options with `.with_*()` methods
- Execute with `.prompt()`

### Ownership vs Clone

Two variants for select operations:
- `select()`: Requires `Clone` for options, useful when you need the list after selection
- `select_owned()`: Takes ownership, no `Clone` required, more efficient

### Theme

All prompts use a consistent `ColorfulTheme` from dialoguer, providing a unified look across the application.

## Integration

This crate is used by:
- `peach_main`: For CLI user interactions
- `peach_infra`: For implementing the `UserInfra` trait

No other crates should depend on `dialoguer` directly - use this crate instead.
