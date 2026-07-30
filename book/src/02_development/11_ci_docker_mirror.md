# CI Docker image mirror

CI pulls its Docker images from a GHCR mirror (`ghcr.io/nomicfoundation/edr/mirror/*`) instead of Docker Hub: Docker Hub rate-limits pulls, which fail intermittently on GitHub-hosted runners as a result. The mirror is maintained by `.github/workflows/mirror-docker-images.yml`, which re-copies the images weekly, on pushes to `main` and same-repo PRs that change the workflow itself, and on demand via `workflow_dispatch`.

Release runs are the exception: when `check_commit` resolves a release tag, the docker jobs in `edr-npm-release.yml` pull the official image straight from Docker Hub (the "Select image source" steps), so the mirror is never in the supply chain of published binaries — a tampered mirror tag can at most affect PR/branch CI, which publishes nothing. At one or two releases a week, Docker Hub's rate limits are not a concern for those runs.

## Adding a Node.js version (or any new tag)

Add the tag to the `TAGS` list in `mirror-docker-images.yml` in the same PR that changes the matrix in `edr-npm-release.yml`. The mirror workflow runs on same-repo PRs that touch it, so the new tag is mirrored — and the release matrix testable against it — before merge. A tag referenced in CI but missing from the mirror fails loudly with `manifest unknown`.

On such a PR the mirror job and the release-workflow jobs start in parallel, so on the first run the release jobs can race ahead and fail their pulls with `manifest unknown`. That's benign: re-run the failed jobs once the mirror job is green.

## Access

The mirror packages are private. Jobs that pull from the mirror need `packages: read` in their permissions and authenticate with `GITHUB_TOKEN`: a `docker login` step before raw `docker run` commands, or the `registry`/`username`/`password` inputs on `docker-run-action` (a container action, which can't see a host `docker login`).

Access works because GHCR automatically links a package to the repository whose workflow published it. Never seed or push the mirror packages from a personal token — that breaks the repository link and CI loses access. Fork PRs can pull (their read-only `GITHUB_TOKEN` still carries `packages: read`) but can't publish, so the mirror job skips itself on fork PRs; a fork PR that needs a new tag has to wait for the post-merge mirror run on `main`.
