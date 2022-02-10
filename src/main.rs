use abduction::AbductionArgs;
use clap::StructOpt;

#[cfg(not(feature = "tdebugger"))]
fn main() -> anyhow::Result<()> {
    let args = AbductionArgs::parse();
    abduction::lib_main(args)
}

#[cfg(feature = "tdebugger")]
fn main() -> anyhow::Result<()> {
    let args = AbductionArgs::parse();
    abduction::tdebugger::run_with_debugger(args)
}
