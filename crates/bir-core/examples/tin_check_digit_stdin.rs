use bir_core::validation::official_tin_check_code;
use std::io::{self, BufRead, BufWriter, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut out = BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let tin = line?;
        let code = official_tin_check_code(&tin);
        writeln!(out, "{tin}\t{code}")?;
    }
    Ok(())
}
