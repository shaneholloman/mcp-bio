# Architecture Experiments

This directory keeps durable experiment writeups, scripts, small source fixtures,
and decision notes that explain architectural findings.

Generated run outputs do not belong in git. Keep large or reproducible payloads
under a local, untracked `results/` directory while running an experiment, then
summarize the relevant outcome in the experiment writeup. The quality ratchet
fails if any file matching `architecture/experiments/**/results/**` becomes
tracked again.
