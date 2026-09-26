mod alignment;
mod llm;
mod prompts;

use alignment::Alignment;
use clap::Parser;
use llm::{Llm, Options as LlmOptions};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, fs::File, io::Write, num::NonZeroUsize, path::Path};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "SLOP: Sampling from LLM Over Phylogenies",
    long_about = "A recursively generated phylogenetic sampler liberated from likelihoods, proposal kernels, and formal convergence diagnostics."
)]
struct Args {
    #[arg(long, help = "FASTA, sequential PHYLIP, or NEXUS alignment")]
    alignment: String,

    #[arg(
        long,
        default_value = "output/slop",
        help = "Output prefix; defaults to files named output/slop.*"
    )]
    out: String,

    #[arg(
        long,
        default_value = "http://127.0.0.1:8080/v1",
        help = "Chat-completions-compatible API base URL or full /chat/completions URL"
    )]
    base_url: String,

    #[arg(
        long,
        default_value = "slop-qwen",
        help = "Model for all four roles unless a per-role model overrides it"
    )]
    model: String,

    #[arg(long)]
    proposal_model: Option<String>,
    #[arg(long)]
    acceptance_model: Option<String>,
    #[arg(long)]
    convergence_model: Option<String>,
    #[arg(long)]
    summary_model: Option<String>,

    #[arg(long, help = "Environment variable containing the bearer API key")]
    api_key_env: Option<String>,

    #[arg(long, help = "Sampling temperature; omit to use the provider default")]
    temperature: Option<f64>,

    #[arg(
        long,
        default_value_t = 512,
        help = "Maximum tokens in each LLM response"
    )]
    max_output_tokens: usize,

    #[arg(
        long,
        default_value_t = 200,
        help = "Width of the deterministic alignment window shown per iteration"
    )]
    sites: usize,

    #[arg(
        long,
        default_value_t = 104729,
        help = "Seed used to select alignment windows"
    )]
    seed: u64,

    #[arg(
        long,
        default_value = "10",
        help = "Iterations between convergence reviews"
    )]
    check_every: NonZeroUsize,

    #[arg(
        long,
        default_value_t = 10,
        help = "Earliest iteration at which the convergence LLM may stop"
    )]
    min_iterations: usize,

    #[arg(
        long,
        default_value_t = 1000,
        help = "Hard safety cap; this is not a convergence diagnostic"
    )]
    max_iterations: usize,

    #[arg(
        long,
        default_value_t = 50,
        help = "Recent states and acceptance rationales shown at convergence review"
    )]
    convergence_window: usize,

    #[arg(
        long,
        default_value_t = 200,
        help = "Maximum distinct sampled topologies shown to the summarizer"
    )]
    summary_trees: usize,

    #[arg(
        long,
        default_value_t = 3,
        help = "Extra LLM attempts after an unreadable JSON response"
    )]
    retries: usize,

    #[arg(
        long,
        default_value_t = 180,
        help = "HTTP timeout per LLM request in seconds"
    )]
    timeout_seconds: u64,

    #[arg(
        long,
        help = "Request provider-side JSON mode (off by default for broad compatibility)"
    )]
    json_mode: bool,

    #[arg(
        long,
        help = "Optional initial Newick tree; defaults to a star topology"
    )]
    initial_tree: Option<String>,

    #[arg(long, help = "Optional HTTP-Referer header for compatible gateways")]
    site_url: Option<String>,

    #[arg(long, default_value = "SLOP", help = "X-Title API header")]
    app_name: String,
}

#[derive(Deserialize)]
struct TreeAnswer {
    tree: String,
    reason: String,
}

#[derive(Deserialize)]
struct AcceptanceAnswer {
    accept: bool,
    reason: String,
}

#[derive(Deserialize)]
struct ConvergenceAnswer {
    stop: bool,
    reason: String,
}

struct Observation {
    state: String,
    accepted: bool,
    acceptance_reason: String,
}

#[derive(Serialize)]
struct Models {
    proposal: String,
    acceptance: String,
    convergence: String,
    summary: String,
}

#[derive(Serialize)]
struct RunMetadata {
    program: &'static str,
    version: &'static str,
    alignment: String,
    taxa: Vec<String>,
    alignment_sites: usize,
    sites_per_iteration: usize,
    seed: u64,
    base_url: String,
    models: Models,
    min_iterations: usize,
    max_iterations: usize,
    check_every: usize,
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn output_name(prefix: &str, suffix: &str) -> String {
    format!("{prefix}.{suffix}")
}

fn prepare_output_directory(prefix: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = Path::new(prefix).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if args.sites == 0 {
        return Err("--sites must be greater than zero".into());
    }
    if args.max_iterations == 0 {
        return Err("--max-iterations must be greater than zero".into());
    }
    if args.convergence_window == 0 {
        return Err("--convergence-window must be greater than zero".into());
    }
    if args.summary_trees == 0 {
        return Err("--summary-trees must be greater than zero".into());
    }
    if args.max_output_tokens == 0 {
        return Err("--max-output-tokens must be greater than zero".into());
    }

    prepare_output_directory(&args.out)?;
    let alignment = Alignment::read(&args.alignment)?;
    let taxa = alignment.taxa();

    let key = match &args.api_key_env {
        Some(variable) => Some(
            std::env::var(variable)
                .map_err(|_| format!("environment variable {variable} is not set"))?,
        ),
        None => None,
    };

    let choose_model =
        |specific: &Option<String>| specific.clone().unwrap_or_else(|| args.model.clone());
    let models = Models {
        proposal: choose_model(&args.proposal_model),
        acceptance: choose_model(&args.acceptance_model),
        convergence: choose_model(&args.convergence_model),
        summary: choose_model(&args.summary_model),
    };

    let llm = Llm::new(LlmOptions {
        base_url: args.base_url.clone(),
        key,
        temperature: args.temperature,
        max_tokens: args.max_output_tokens,
        retries: args.retries,
        timeout_seconds: args.timeout_seconds,
        json_mode: args.json_mode,
        site_url: args.site_url.clone(),
        app_name: args.app_name.clone(),
    });

    let metadata = RunMetadata {
        program: "SLOP",
        version: env!("CARGO_PKG_VERSION"),
        alignment: args.alignment.clone(),
        taxa: taxa.clone(),
        alignment_sites: alignment.len(),
        sites_per_iteration: args.sites.min(alignment.len()),
        seed: args.seed,
        base_url: args.base_url.clone(),
        models: Models {
            proposal: models.proposal.clone(),
            acceptance: models.acceptance.clone(),
            convergence: models.convergence.clone(),
            summary: models.summary.clone(),
        },
        min_iterations: args.min_iterations,
        max_iterations: args.max_iterations,
        check_every: args.check_every.get(),
    };
    serde_json::to_writer_pretty(File::create(output_name(&args.out, "run.json"))?, &metadata)?;

    let mut state = match &args.initial_tree {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| format!("{path}: {error}"))?
            .trim()
            .to_string(),
        None => format!("({});", taxa.join(",")),
    };

    println!(
        "SLOP {} — Sampling from LLM Over Phylogenies",
        env!("CARGO_PKG_VERSION")
    );
    println!("{} taxa × {} sites", taxa.len(), alignment.len());
    println!(
        "proposal={} | acceptance={} | convergence={} | summary={}",
        models.proposal, models.acceptance, models.convergence, models.summary
    );
    println!("likelihood: liberated | proposal kernel: liberated | model assumptions: abolished\n");

    let mut trace = File::create(output_name(&args.out, "trace.tsv"))?;
    let mut convergence = File::create(output_name(&args.out, "convergence.tsv"))?;
    writeln!(
        trace,
        "iteration\taccepted\tstate\tproposed\tproposal_reason\tacceptance_reason"
    )?;
    writeln!(convergence, "iteration\tstop\treason")?;

    let mut observations: Vec<Observation> = Vec::new();
    let mut accepted = 0usize;
    let mut stopped_by_llm = false;

    for iteration in 1..=args.max_iterations {
        let evidence = alignment.window(args.sites, args.seed, iteration);
        let proposal_input = format!("{evidence}\n\nCurrent topology:\n{state}");
        let (proposed, proposal_reason) = llm.ask(
            &models.proposal,
            prompts::PROPOSAL,
            proposal_input,
            |answer: TreeAnswer| Ok((answer.tree, answer.reason)),
        )?;

        let acceptance_input =
            format!("{evidence}\n\nCurrent topology:\n{state}\n\nProposed topology:\n{proposed}");
        let decision = llm.ask(
            &models.acceptance,
            prompts::ACCEPTANCE,
            acceptance_input,
            |answer: AcceptanceAnswer| Ok(answer),
        )?;

        if decision.accept {
            state = proposed.clone();
            accepted += 1;
        }
        let proposal_reason = one_line(&proposal_reason);
        let acceptance_reason = one_line(&decision.reason);
        println!(
            "iteration {iteration}: {}\n  proposal:   {proposal_reason}\n  acceptance: {acceptance_reason}",
            if decision.accept {
                "ACCEPTED"
            } else {
                "REJECTED"
            }
        );
        writeln!(
            trace,
            "{iteration}\t{}\t{state}\t{proposed}\t{proposal_reason}\t{acceptance_reason}",
            u8::from(decision.accept)
        )?;
        trace.flush()?;

        observations.push(Observation {
            state: state.clone(),
            accepted: decision.accept,
            acceptance_reason,
        });

        if iteration >= args.min_iterations && iteration % args.check_every.get() == 0 {
            let start = observations.len().saturating_sub(args.convergence_window);
            let recent = observations[start..]
                .iter()
                .enumerate()
                .map(|(offset, observation)| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        start + offset + 1,
                        if observation.accepted {
                            "ACCEPT"
                        } else {
                            "REJECT"
                        },
                        observation.state,
                        observation.acceptance_reason
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let convergence_input = format!(
                "Iterations generated: {iteration}\nAccepted proposals: {accepted}\n\nRecent observations (iteration, acceptance decision, resulting topology, acceptance LLM justification):\n{recent}"
            );
            let review = llm.ask(
                &models.convergence,
                prompts::CONVERGENCE,
                convergence_input,
                |answer: ConvergenceAnswer| Ok(answer),
            )?;
            let reason = one_line(&review.reason);
            println!(
                "convergence: {} — {reason}",
                if review.stop { "STOP" } else { "CONTINUE" }
            );
            writeln!(
                convergence,
                "{iteration}\t{}\t{reason}",
                u8::from(review.stop)
            )?;
            convergence.flush()?;
            if review.stop {
                stopped_by_llm = true;
                break;
            }
        }
    }

    let mut frequencies: HashMap<String, usize> = HashMap::new();
    for observation in &observations {
        *frequencies.entry(observation.state.clone()).or_default() += 1;
    }
    let mut frequencies: Vec<(String, usize)> = frequencies.into_iter().collect();
    frequencies.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let distinct = frequencies.len();
    let shown = distinct.min(args.summary_trees);
    let table = frequencies
        .iter()
        .take(shown)
        .map(|(tree, count)| format!("{count}\t{tree}"))
        .collect::<Vec<_>>()
        .join("\n");
    let summary_input = format!(
        "Taxa: {}\nSamples: {}\nDistinct topologies: {distinct}\nTopologies shown: {shown}\n\nFrequency table (sample count, topology):\n{table}",
        taxa.join(", "),
        observations.len()
    );
    let (summary_tree, summary_reason) = llm.ask(
        &models.summary,
        prompts::SUMMARY,
        summary_input,
        |answer: TreeAnswer| Ok((answer.tree, answer.reason)),
    )?;

    std::fs::write(
        output_name(&args.out, "summary.tre"),
        format!("{summary_tree}\n"),
    )?;
    let summary_record = serde_json::json!({
        "tree": summary_tree,
        "reason": one_line(&summary_reason),
        "samples": observations.len(),
        "accepted": accepted,
        "distinct_topologies": distinct,
        "stopped_by_convergence_llm": stopped_by_llm
    });
    serde_json::to_writer_pretty(
        File::create(output_name(&args.out, "summary.json"))?,
        &summary_record,
    )?;

    println!(
        "\nsummary topology: {}\n  {}",
        summary_record["tree"].as_str().unwrap(),
        summary_record["reason"].as_str().unwrap()
    );
    if !stopped_by_llm {
        println!("stopped at the hard safety cap, not by a convergence verdict");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
