# CI Docker image mirror

CI pulls its Docker images from a GHCR mirror (`ghcr.io/nomicfoundation/edr/mirror/*`) instead of Docker Hub: Docker Hub rate-limits pulls, which fail intermittently on GitHub-hosted runners as a result. The mirror is maintained by `.github/workflows/mirror-docker-images.yml`, which re-copies the images weekly and can be run on demand via `workflow_dispatch`.

## Adding a Node.js version (or any new tag)

Add the tag to the `TAGS` list in `mirror-docker-images.yml` in the same PR that changes the matrix in `edr-npm-release.yml`. The mirror workflow runs on same-repo PRs that touch it, so the new tag is mirrored — and the release matrix testable against it — before merge. A tag referenced in CI but missing from the mirror fails loudly with `manifest unknown`.

## Access

The mirror packages are private. Jobs that pull from the mirror need `packages: read` in their permissions and authenticate with `GITHUB_TOKEN`: a `docker login` step before raw `docker run` commands, or the `registry`/`username`/`password` inputs on `docker-run-action` (a container action, which can't see a host `docker login`).

Access works because GHCR automatically links a package to the repository whose workflow published it. Never seed or push the mirror packages from a personal token — that breaks the repository link and CI loses access. Fork PRs can pull (their read-only `GITHUB_TOKEN` still carries `packages: read`) but can't publish, so the mirror job skips itself on fork PRs; a fork PR that needs a new tag has to wait for the post-merge mirror run on `main`.
