//! Generates roff man pages for bap and every subcommand.
//!
//! Usage: `cargo run -p bap --example mangen -- <output-dir>`
//! CI regenerates these and fails when docs/man/*.1 drift from the CLI.

use clap_mangen::Man;

fn main() -> std::io::Result<()> {
    let target = std::env::args()
        .nth(1)
        .expect("usage: mangen <output-directory>");
    std::fs::create_dir_all(&target)?;
    render(&bap_cli::root_command(), std::path::Path::new(&target))?;
    println!("man pages written to {target}");
    Ok(())
}

fn render(command: &clap::Command, directory: &std::path::Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(directory.join(format!("{}.1", command.get_name())))?;
    Man::new(command.clone()).render(&mut file)?;

    for subcommand in command.get_subcommands() {
        if subcommand.is_hide_set() {
            continue;
        }
        // clap borrows names; the leak is bounded by page count.
        let full_name: &'static str =
            Box::leak(format!("{}-{}", command.get_name(), subcommand.get_name()).into_boxed_str());
        let renamed = subcommand.clone().name(full_name);
        render(&renamed, directory)?;
    }
    Ok(())
}
