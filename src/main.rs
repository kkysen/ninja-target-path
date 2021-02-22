use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use structopt::StructOpt;

use ninja_target_path::cache::Cache;

#[derive(Debug, StructOpt)]
#[structopt(
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION"),
)]
struct Args {
    // "-C" to match ninja
    #[structopt(short = "C", long)]
    build_dir: PathBuf,
    #[structopt(long)]
    absolute: bool,
    targets: Vec<OsString>,
}

#[paw::main]
fn main(args: Args) -> anyhow::Result<()> {
    let Args { build_dir, absolute, targets } = args;
    let mut cache = Cache::read(build_dir.as_path())?;
    let cwd = if absolute {
        env::current_dir()?
    } else {
        PathBuf::new()
    };
    for target in targets {
        let path = cwd.join(cache.get(target)?);
        println!("{}", path.display());
    }
    cache.write_drop()?;
    Ok(())
}
