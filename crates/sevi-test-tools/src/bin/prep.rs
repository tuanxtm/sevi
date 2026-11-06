use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
};
fn main() -> Result<(), Box<dyn Error>> {
    let mut files = vec![];
    let mut rate = 0.0;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--typos" {
            rate = args.next().ok_or("--typos requires a rate")?.parse()?
        } else if arg.starts_with("--") {
            return Err(format!("unknown option {arg}").into());
        } else {
            files.push(fs::read_to_string(arg)?)
        }
    }
    if files.is_empty() {
        return Err("usage: prep <corpus> [more-corpora] [--typos rate]".into());
    }
    io::stdout()
        .lock()
        .write_all(sevi_test_tools::prep::prepare(&files, rate).as_bytes())?;
    Ok(())
}
