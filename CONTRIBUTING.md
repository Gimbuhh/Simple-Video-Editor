# Contributing

Start with the [development guide](docs/development.md). Keep changes focused on simple local gameplay editing, and preserve source recordings, saved cuts, timeline gaps, and linked audio/video behavior.

Create a feature branch and a pull request targeting `main`; direct pushes to `main` are not part of this workflow. Enable the local push guard with `git config --local core.hooksPath .githooks`. PRs must pass Windows checks before merging. This private repository currently cannot enforce branch protection under the account's GitHub plan, so the policy and local guard remain necessary.

Run repository checks and core tests for changes to source or build scripts. Run desktop UI checks for interaction/layout changes and NVIDIA checks for hardware export changes when compatible hardware is available. State which checks ran and which could not run.

Update relevant documentation and the changelog for user-visible changes. Include reproduction steps and expected behavior for bugs. Use generated or shareable recordings; avoid uploading private media, absolute personal paths, recovery files, or unfiltered verification reports.

Commit authored source and assets. Generated app bundles, SDK/media downloads, release ZIPs, and local backups remain ignored. See the [release guide](docs/releasing.md) for versioning and packaging.
