# Contributing

**Something does not work?** Please
[open a problem report](https://github.com/MihleDex/print-pictures/issues/new/choose).
The form asks for the few things needed to look into it.

**Changing the code?** Before sending a pull request:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- It has to work with any printer; do not special-case one model.
- Pictures must come out at their stated size.
- Never change the user's pictures, or a file manager's settings without a backup.
- No network access beyond the computer's own print service.
