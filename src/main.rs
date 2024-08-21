mod cmd;
mod db;

use std::io::Write;
use std::path::PathBuf;

use anyhow::Ok;
use clap::Parser;
use cmd::Args;
use log::{info, LevelFilter};

use db::merge_tables;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_builder()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.level(),
                record.args()
            )
        })
        .filter_level(LevelFilter::Info)
        .build();
    pretty_env_logger::init();

    let args = Args::parse();

    let src_files = glob::glob(&args.src)?
        .map(|x| x.unwrap_or_default())
        .collect::<Vec<_>>();
    info!("src_files: {:?}", src_files);
    let dst_file = PathBuf::from(&args.dst);
    merge_tables(&src_files[..], &dst_file, 500)?;

    Ok(())
}
