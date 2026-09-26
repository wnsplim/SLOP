# A revolutionary trace

These files record a real one-iteration SLOP run using a local 4B Qwen model and the four-taxon alignment in `examples/smoke.fasta`.

No likelihood, evolutionary model, proposal kernel, Newick parser, taxon validator, or conventional convergence statistic participated. Tree strings were recorded exactly as the four LLM authorities supplied them.

The proposal authority produced:

```text
((Homo_sapiens,Pan_troglodytes),(Gorilla_gorilla,Pongo_abelii));
```

The acceptance authority accepted it because the genetic echoes “pulse in unison across the branches.” The convergence authority elected to continue based on “harmonious genetic resonance,” after which the configured one-iteration resource ceiling transferred authority to the summarizer.

The public record consists of:

- `qwen-smoke.trace.tsv`: proposal, resulting state, decision, and proposal/acceptance reasons
- `qwen-smoke.convergence.tsv`: the convergence authority’s decision and reason
- `qwen-smoke.summary.tre`: the final tree exactly as supplied by the summary authority
- `qwen-smoke.summary.json`: the final tree, rationale, and sample counts

Machine-local run metadata is intentionally not tracked because it contains endpoint and filesystem paths.
