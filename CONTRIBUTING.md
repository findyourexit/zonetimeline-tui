# Contributing to zonetimeline-tui

Thanks for your interest in contributing! Here's how to get started.

## Getting Started

1. Fork and clone the repository.
2. Install [Rust 1.88 or newer](https://rustup.rs/).
3. Run the test suite to confirm everything works:
   ```bash
   cargo test
   ```

## Development Workflow

Before submitting a pull request, please make sure all checks pass locally:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI runs these checks on every pull request, plus dependency auditing, coverage collection, macOS and Windows tests, and generated map-data drift detection.

## Pull Requests

- Keep changes focused. One logical change per PR.
- Add tests for new functionality where practical.
- Make sure existing tests still pass.
- Follow the existing code style (`cargo fmt` enforces formatting).

## Reporting Issues

Open an issue on GitHub. Please include:

- What you expected to happen.
- What actually happened.
- Steps to reproduce, if applicable.
- Your OS, terminal emulator, and Rust version.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
