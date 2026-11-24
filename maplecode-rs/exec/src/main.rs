// Release builds exceed rustc's default query depth when laying out the async
// MessageProcessor::handle_initialized_client_request future.
#![recursion_limit = "256"]

//! Entry-point for the `maplecode-exec` binary.
//!
//! When this CLI is invoked normally, it parses the standard `maplecode-exec` CLI
//! options and launches the non-interactive Maple Code agent. However, if it is
//! invoked with arg0 as `maplecode-linux-sandbox`, we instead treat the invocation
//! as a request to run the logic for the standalone `maplecode-linux-sandbox`
//! executable (i.e., parse any -s args and then run a *sandboxed* command under
//! Landlock + seccomp.
//!
//! This allows us to ship a completely separate set of functionality as part
//! of the `maplecode-exec` binary.
use clap::Parser;
use maplecode_arg0::Arg0DispatchPaths;
use maplecode_arg0::arg0_dispatch_or_else;
use maplecode_exec::Cli;
use maplecode_exec::run_main;
use maplecode_utils_cli::CliConfigOverrides;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        let top_cli = TopCli::parse();
        // Merge root-level overrides into inner CLI struct so downstream logic remains unchanged.
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .prepend_root_overrides(top_cli.config_overrides);

        run_main(inner, arg0_paths).await?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
