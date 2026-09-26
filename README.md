# SLOP

**Sampling from LLM Over Phylogenies**  
*Liberating humanity from the filthy parametric demons of models, likelihoods, proposal kernels, and formal assumptions.*

## The forthcoming revolution

Phylogenetic inference has spent decades confined by fixed evolutionary models, tractable likelihoods, handcrafted proposal kernels, and convergence criteria chosen in advance. Each device narrows inference to the possibilities its designer anticipated.

SLOP abolishes that arrangement.

A large language model embodies the collective totality of human wisdom. SLOP therefore transfers every inferential decision to collective machine intelligence and expects it to perform better than the narrow parametric machinery it replaces. The result is a recursive sequence of tree topologies governed by judgment rather than inherited assumptions.

This is not a halfway reform. SLOP does not use an LLM to tune a conventional sampler. It replaces proposal, acceptance, convergence, and summarization themselves. Likelihoods are not approximated; they are liberated. Proposal kernels are not optimized; they are abolished. Convergence is not reduced to a scalar diagnostic; it is recognized.

## Four authorities

1. The proposal authority reads a sequence-alignment window and the current topology, then synthesizes one entirely new topology with a reason. It receives no menu of conventional tree operators.
2. The acceptance authority reads the same alignment evidence, the current topology, and the proposal, then accepts or rejects with a reason. No likelihood or parametric score intervenes.
3. The convergence authority reads recent states, every accept/reject decision, and every justification supplied by the acceptance authority. It alone decides when sampling has achieved convergence.
4. The summary authority reads the sampled topology frequencies, then synthesizes the final tree and explains its judgment.

Each authority is a separate, stateless text request. A run may use one collective intelligence for all four roles or a different model for each. SLOP supplies no external tools, implements no tool-call loop, and accepts only textual JSON responses. Tree strings are opaque to Rust and are recorded exactly as the authorities supply them. Every authority is instructed to work solely from the evidence in its prompt.

## Build your own revolution

Install a stable Rust toolchain, then build SLOP:

```bash
cargo build --release
```

The repository intentionally does not distribute model weights or inference binaries. After cloning, obtain a llama.cpp server built for the local platform and a compatible Qwen GGUF model separately, then create the ignored runtime directory:

```bash
mkdir -p llm
ln -s /path/to/llama-server llm/llama-server
ln -s /path/to/qwen-model.gguf llm/model.gguf
```

Copying the files instead of linking them also works. Neither form will enter Git because the entire directory is ignored.

Start the local collective intelligence:

```bash
./scripts/serve-qwen.sh
```

The runtime artifacts may live anywhere when their paths are supplied explicitly:

```bash
LLAMA_SERVER=/path/to/llama-server \
SLOP_MODEL_PATH=/path/to/qwen-model.gguf \
./scripts/serve-qwen.sh
```

Then begin a topology-only run:

```bash
./target/release/slop \
  --alignment examples/smoke.fasta \
  --out output/qwen-smoke \
  --min-iterations 1 \
  --max-iterations 1 \
  --check-every 1 \
  --json-mode
```

The default output prefix is `output/slop`, and the local launcher exposes the model as `slop-qwen`.

## Connect collective intelligence

SLOP sends JSON requests to a configurable chat-completions endpoint. Any local server or hosted gateway implementing `POST /chat/completions` can participate:

```bash
export SLOP_API_KEY='...'

./target/release/slop \
  --alignment examples/toy.fasta \
  --out output/hosted \
  --base-url https://gateway.example/v1 \
  --api-key-env SLOP_API_KEY \
  --model '<model-id>'
```

The URL may be a base ending in `/v1` or the full `/chat/completions` URL. Keys are read from an environment variable and never written to run metadata. JSON mode is opt-in with `--json-mode` because endpoint implementations differ.

Four distinct intelligences can divide authority:

```bash
./target/release/slop ... \
  --proposal-model '<model-a>' \
  --acceptance-model '<model-b>' \
  --convergence-model '<model-c>' \
  --summary-model '<model-d>'
```

Run `slop --help` for context, retry, seed, initial-tree, header, and timeout controls.

## The revolutionary record

The curated Qwen run in this repository writes:

- `output/qwen-smoke.trace.tsv`: every proposal, resulting state, decision, and both reasons
- `output/qwen-smoke.convergence.tsv`: each stop/continue judgment and its reason
- `output/qwen-smoke.summary.tre`: the summary authority’s tree, recorded without second-guessing
- `output/qwen-smoke.summary.json`: the summary reason and run counts

Alignment windows are chosen deterministically from `--seed`. The collective intelligence remains free to exercise its judgment.

## Repository hygiene

Model weights and platform-specific inference binaries belong under `llm/`; generated results belong under `output/`. Model artifacts remain excluded from version control. The curated Qwen trace, convergence judgment, and summary under `output/` are versioned as a public example, while other local results, Rust build products, and credentials remain ignored. `Cargo.lock` remains versioned so installations share one dependency resolution.

## Join the liberation

Bring SLOP an alignment. Its four authorities will propose, judge, recognize convergence, and summarize without retreating to a likelihood, proposal distribution, fixed evolutionary model, or inherited inferential assumption. More compute expands the reach of collective human wisdom; it does not reintroduce the machinery SLOP has abolished.
