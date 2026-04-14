# Publish Report

## Tasks Completed
- [ ] Blog article: daraxonrasib-six-commands
- [x] Docs update: replace the daraxonrasib article install block with the canonical installer block
- [ ] Changelog: not requested
- [ ] Docs deploy: manual gh-deploy
- [ ] Release: not requested

## Files Changed
- docs/blog/daraxonrasib-six-commands.md
- .march/publish-report.md

## Verification
- `uv sync --extra dev && uv run mkdocs build --strict`: PASS
- Deprecated package-name grep in `docs/blog/daraxonrasib-six-commands.md`: 0 matches
- Boilerplate scrub on added staged diff: PASS
- Site live at: https://biomcp.org/blog/daraxonrasib-six-commands/ (pending next docs deploy)

## Notes
Docs hotfix only; changelog, docs deploy, and release work were out of scope.
The staged scrub was evaluated against introduced diff lines so the removed deprecated package reference did not cause a false positive.
