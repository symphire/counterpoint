use super::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(long)]
    pub settings: Option<String>,
}
