# Contributing

Thanks for helping improve SkillHub.

## Local Setup

```bash
git clone https://github.com/RsLuna7/skillhub.git
cd skillhub
cargo test
```

## Checks

Run these before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Design Principles

- Keep SkillHub local-first.
- Keep v1 read-first and safe by default.
- Do not execute skill scripts silently.
- Prefer small, focused modules.
- Document user-facing behavior in README or `docs/`.

## Good First Contributions

- Add setup docs for more agents.
- Improve skill parsing fixtures.
- Add tests for real-world skill repository layouts.
- Improve search ranking without adding heavyweight services.
