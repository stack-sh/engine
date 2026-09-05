# Cargo publication

The native `stack-formatter` 0.1.0 and `stack-engine` 0.7.0 crates use exact registry dependencies. The workspace retains local paths for development, with registry versions declared for Cargo packaging. `stack-engine-wasm` is not published to crates.io; its browser output remains the npm package.

## Initial publication

1. Merge the release preparation and wait for both main CI jobs. Run `node scripts/cargo-package-assets.mjs --check` to verify package notices.
2. Set a short-lived `CARGO_INITIAL_PUBLISH_TOKEN` Actions secret with `publish-new` permission limited to the exact initial crate names. Never put the value into source, workflow inputs, issues, or logs.
3. Dispatch `initial-publish.yaml` on `main` with the full successful `expected_sha` and package `stack-formatter`. The workflow checks repository, ref, commit, package metadata, successful CI, and registry absence before a credential-free package dry run and publication.
4. Verify the formatter's registry version, checksum, source SHA, and registry-only consumer. Then dispatch the same workflow for `stack-engine`. Cargo verifies the engine package against the published formatter, compiler, and theme rather than Git dependencies.
5. Verify the engine registry artifact and a clean consumer on Rust 1.85 and stable. Record immutable `stack-formatter-v0.1.0` and `stack-engine-v0.7.0` source tags without replacing any npm release tag.
6. Remove the bootstrap GitHub secret and revoke the token after initial publications. Configure crate-specific trusted publishers before later releases. The bootstrap workflow cannot publish a second version and never persists a Cargo credential file.

If Cargo times out after upload, inspect registry state before retrying. Never overwrite a version or tag: investigate failures and publish an explicitly versioned correction. No layout snapshot or language behavior is changed for registry packaging.

## Ongoing trusted publishing

After initial publication, configure each crate's Settings → Trusted Publishing on crates.io with repository owner `stack-sh`, repository name `engine`, workflow filename `cargo-publish.yaml`, and no environment. Add a separate configuration for both `stack-formatter` and `stack-engine`. The crate owner must save these settings; committing this workflow does not configure or prove registry trust. Follow the [crates.io instructions](https://crates.io/docs/trusted-publishing).

Dispatch `cargo-publish.yaml` from `main` with the full successful main CI commit and the exact package version. The default `publish: false` validates identity, registry state, and packaging, then checks the OIDC exchange **without uploading a crate**. This proves workflow authentication, not a new version's publication or every crate's owner configuration. The pinned authentication action revokes its short-lived token when the job ends; no long-lived repository secret or credentials file is used.

For an actual new release, merge the version change and all checks first, publish dependencies before consumers, then dispatch with `publish: true`. Existing versions, missing crates, non-main refs, version/SHA drift, and unsuccessful CI fail closed. Verify the downloaded archive checksum and source SHA after publication; a failed post-upload check does not undo an upload. Never rerun an upload without checking registry state.
