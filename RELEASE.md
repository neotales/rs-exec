# Releasing

Publishing runs from `release.yml`. It is driven by a tag and it authenticates
with a short-lived crates.io token obtained over OIDC. There is no
`CARGO_REGISTRY_TOKEN` secret in this repository.

## The version rule

The tag is `v` plus the version in the manifests. Tag `v0.0.0-alpha.1` publishes
the crates only if `exec/Cargo.toml` and `exec-script/Cargo.toml` both say
`version = "0.0.0-alpha.1"`. The `version` job fails the run when they differ
from each other or from the tag, so a tag can never publish a version that
differs from what is in the tree.

This repository ships two crates that share the tag: `neotales-exec` from
`exec/` and `neotales-exec-script` from `exec-script/`. Unlike a pair of independent
crates, the order is not a matter of taste: `exec-script/Cargo.toml` names
`neotales-exec = { path = "../exec", version = "..." }`, and packaging rewrites
that path to the registry version. The publish job uploads `neotales-exec`
first, then `neotales-exec-script`.

The path is what the workspace builds against. The version is what the packaged
manifest carries, so a caller who adds `neotales-exec-script` gets `neotales-exec`
from crates.io at the same version.

## Dry runs and the dependency

`cargo publish -p neotales-exec-script --dry-run --locked` fails until that exact
`neotales-exec` version is on crates.io, because cargo resolves the rewritten
version dependency against the index. The pre-publish check therefore packages
`neotales-exec` on every trigger, and packages `neotales-exec-script` only when it
can already succeed:

- `workflow_dispatch` packages `neotales-exec` only. Do not read a skipped
  script dry run as a pass.
- A tag run packages `neotales-exec-script` when the exec version it names is already
  on crates.io, which is the re-run case. On the first run for a new version the
  exec upload happens later in that same run, so the check skips it.

## Bump and tag

Bump the version in both manifests, commit, push, then tag that commit:

```sh
git tag v0.2.0
git push origin dev
git push origin v0.2.0
```

The tag uploads both crates, in this order, and neither upload is optional:

```sh
cargo publish -p neotales-exec --locked
cargo publish -p neotales-exec-script --locked
```

The one exception is a new crate's first version, which is uploaded by hand and
never from a tag. That is the next section.

## First publish of a new crate

crates.io trusted publishing cannot create a brand-new crate. The OIDC exchange
needs a crate that exists and a trusted publisher configured on it, so a cold
start over OIDC is rejected by crates.io. Do not try to build one.

The first version of a new crate is `0.0.0-alpha.0`. Not `0.0.0-alpha.1` and
not `0.1.0`. The first upload is manual, and a manual upload needs a version
nothing else has claimed.

The first upload goes through `/home/dev/work/rs/publish.sh <dir>`. That script
lives beside this folder, in `~/work/rs`, and is deliberately not copied into
the crate repository: it is how the first version of every crate is uploaded,
not part of any one crate.

1. Set `version = "0.0.0-alpha.0"` in `exec/Cargo.toml` and
   `exec-script/Cargo.toml`, commit it, and leave the working tree clean. Do not
   publish from a tree with uncommitted changes.
2. Run the script with `exec` first. `exec-script` names `neotales-exec` as a version
   dependency, so its upload is rejected while `neotales-exec = "0.0.0-alpha.0"`
   is not on crates.io.

   ```sh
   ~/work/rs/publish.sh exec
   ~/work/rs/publish.sh exec-script
   ```

   The script reads the token itself and passes it to `cargo` through the
   environment. Do not put a token on the command line, in the repository, or
   in shell history.
3. Add the trusted publisher on crates.io for both crates, in each crate's
   settings: owner `neotales`, the repository name, workflow filename
   `release.yml`, and an empty environment. Leave the environment field empty.
   The `publish` job in `release.yml` does not set an environment, and crates.io
   compares the two fields: an environment name here that the job does not set,
   or the reverse, fails the token exchange and nothing else fails with it. If
   someone later sets an environment name on crates.io, the job must set that
   exact name.
4. Bump to the next version and let tags take over. Do not run `publish.sh` for
   these crates again.

The release run for the first version fails, because the version it would
publish is already uploaded. The tag is provenance for the first version, not
its publish trigger. Every later version publishes from its `v*` tag.

If the crate name is taken on crates.io by someone else, stop. A rename is a
different repository setup, not a flag.

`cargo package` includes every tracked file, so `.github/` and `RELEASE.md`
ship inside the published crate unless the manifest excludes them. That is
harmless. Exclude them if package size matters. The README style guide lives
at `~/work/rs/README.STYLE.md` and is not part of a crate.

## Later releases

1. Bump the version in both `Cargo.toml` files and commit it. `Cargo.lock` is
   committed on purpose, so run `cargo build` and commit the updated lockfile
   with it. The lockfile is what the scheduled audit checks.
2. Push the commit.
3. Tag it `vX.Y.Z` and push the tag.
4. The workflow runs the checks, packages `neotales-exec`, and then publishes
   `neotales-exec` followed by `neotales-exec-script`. One OIDC exchange covers both
   crates when each has a trusted publisher for this repository, `release.yml`,
   and an empty environment. A re-run skips a version already on crates.io. Do
   not republish it, and do not delete or move the tag.

A manual run, `workflow_dispatch`, runs the checks and the `neotales-exec` dry
run only. It never publishes, whatever branch it is started from.

After the manual first upload is done, switch both crates to Trusted Publishing
Only mode on crates.io. That rejects token publishes everywhere, so a token
cannot publish these crates later.

## What is not allowed

- `pull_request_target` and `workflow_run` are not used anywhere, and must not
  be added. Both can run untrusted code with publishing credentials in scope.
- No long-lived crates.io token in GitHub secrets for this workflow. The job
  reads `id-token: write` and nothing else.
- No publishing from a branch. The tag check exists so the artifact has a name.
- No republishing a version. crates.io rejects it. If a publish fails after the
  upload was accepted, bump the patch version and release again.
- No publishing `neotales-exec-script` before `neotales-exec` at the same version.

## Failure modes

| Symptom | Cause |
| --- | --- |
| `version` job: tag does not match `vX.Y.Z` | `Cargo.toml` version and the tag disagree. Delete the tag, fix, re-tag. |
| Publish job: OIDC exchange fails | The crate does not exist yet, or the trusted publisher does not match: wrong repository, wrong workflow filename, or an environment name that is set on one side and not the other. |
| Publish job: version already uploaded | The version was published before, usually by the manual first release. Bump the version. |
| Publish job: no matching package named `neotales-exec` | `neotales-exec-script` was uploaded before the exec version it names reached the index. Publish exec, wait for the index, then exec-script. |
| Publish job: `cargo publish` rejects the token | Trusted Publishing Only mode is on and the crate has no trusted publisher configured. |
