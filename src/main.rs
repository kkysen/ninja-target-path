use std::path::PathBuf;

use structopt::StructOpt;

use ninja_target_path::cache::Cache;

#[derive(Debug, StructOpt)]
#[structopt(
author = env ! ("CARGO_PKG_AUTHORS"),
about = env ! ("CARGO_PKG_DESCRIPTION"),
)]
struct Args {
    // "-C" to match ninja
    #[structopt(short = "C", long)]
    build_dir: PathBuf,
    targets: Vec<String>,
}

#[paw::main]
fn main(args: Args) -> anyhow::Result<()> {
    let Args { build_dir, targets } = args;
    let mut cache = Cache::read(build_dir)?;
    for target in targets {
        println!("{}", cache.get(target)?.display());
    }
    Ok(())
}
