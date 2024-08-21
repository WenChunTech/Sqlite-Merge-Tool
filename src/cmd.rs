use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = "this is a tool for merge multiple sqlite database files into one"
)]
pub struct Args {
    /// The source database file path, support glob pattern
    #[arg(short, long)]
    pub src: String,
    /// The destination database file path
    #[arg(short, long)]
    pub dst: String,
}
