# Pattern: Drug regulatory and approval evidence

Use this when the question asks for a brand-to-generic mapping, approval history, licensing status, or developer context.

```bash
biomcp search drug "Gliolan" --region eu --limit 5
biomcp get drug "5-aminolevulinic acid" regulatory --region eu
biomcp get drug "5-aminolevulinic acid" approvals
biomcp search article --drug "5-aminolevulinic acid" -k glioma --type review --limit 5
```

Interpretation:
- Resolve the brand name to the active substance before interpreting approval records.
- Prefer structured regulatory and approval sections for dates, regions, and authorization status.
- Use article reviews only for development history or context missing from regulatory records.
- Keep region labels explicit because U.S., EU, and WHO records answer different approval questions.
