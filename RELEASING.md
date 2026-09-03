# Releasing `@stack-sh/engine`

The package is public and is released from this public repository. Release artifacts must come from a merged `main` revision whose `baseline` and `Minimum supported Rust` checks have passed.

## First release

The npm package must exist before its trusted publisher can be configured. An authenticated maintainer with publish access to the `@stack-sh` scope performs the one-time bootstrap from a clean `main` checkout:

```sh
npm ci
rustup target add wasm32-unknown-unknown --toolchain stable
npm run build:wasm
npm test
npm run typecheck
npm run pack:check
npm publish --workspace @stack-sh/engine --access public
```

After `@stack-sh/engine` exists on npm, configure its trusted publisher with these exact values:

- Provider: GitHub Actions
- Organization: `stack-sh`
- Repository: `engine`
- Workflow filename: `release.yaml`
- Allowed action: `npm stage publish`

Then create the `v0.1.0` GitHub Release from the same merged revision. The release workflow recognizes that the package version already exists and completes without staging it twice.

If a published release needs to resume after a workflow-only correction, run the Release workflow manually with the existing exact tag. The recovery path checks out that tag and applies the same version, ancestry, build, test, and package-content checks before it can stage the package. Do not rerun the workflow after a version has entered the staging area; inspect or reject that staged version instead.

## Subsequent releases

1. Update the workspace and package versions in a pull request.
2. Run the complete repository checks and merge the pull request.
3. Create a GitHub Release whose tag is exactly `v<package version>` and targets the merged commit.
4. Verify that the release workflow stages the package through npm trusted publishing.
5. Inspect the staged package on npm, then approve it with two-factor authentication.
6. Verify the public registry metadata, provenance, and a clean consumer installation.

The workflow rejects a tag that does not match the package version. It builds and validates the package again on the tagged revision, uses no long-lived npm token, and leaves npm provenance enabled. A package does not become public until a maintainer explicitly approves the staged version with two-factor authentication.
