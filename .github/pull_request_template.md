## Summary

- 

## Scope

- [ ] Core
- [ ] CLI
- [ ] AppPortal
- [ ] Docs
- [ ] Schemas/examples
- [ ] Tests
- [ ] CI/tooling

## Verification

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
tools/dev-check.sh
```

## Compatibility and Security Notes

- Does this change any compatibility claim?
- Does this change sandbox, filesystem, registry, network, desktop, or packaging behavior?
- Does this avoid implementing or implying unsupported Windows runtime execution?

## Contributor Terms

- [ ] I agree to `CONTRIBUTOR-LICENSE-TERMS.md`.
