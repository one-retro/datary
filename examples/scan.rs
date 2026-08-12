//! Checks a directory of ROMs against a datafile.
//!
//! ```sh
//! cargo run --example scan -- Nintendo\ -\ Virtual\ Boy.dat ~/roms/vb
//! ```
//!
//! Every file in the directory is hashed once and looked up by its strongest
//! checksum, then the datafile entries that were never matched are reported as
//! missing.

use datary::verify::FileHashes;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(dat_path), Some(dir)) = (args.next(), args.next()) else {
        eprintln!("usage: scan <datafile.dat> <rom-directory>");
        std::process::exit(2);
    };

    let dat = datary::read_file(&dat_path)?.indexed();

    let mut matched: HashSet<&str> = HashSet::new();
    let (mut ok, mut unknown) = (0usize, 0usize);

    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }

        let hashes = FileHashes::of_file(&path)?;
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        match dat.find(&hashes) {
            Some((game, _)) => {
                matched.insert(game.name.as_str());
                ok += 1;
                println!("  ok       {name}  ->  {}", game.name);
            }
            None => {
                unknown += 1;
                println!("  unknown  {name}  (sha1 {})", hashes.sha1);
            }
        }
    }

    let missing: Vec<_> = dat
        .games
        .iter()
        .filter(|g| !matched.contains(g.name.as_str()))
        .collect();

    println!(
        "\n{ok} matched, {unknown} unknown, {} missing",
        missing.len()
    );

    // A dump nobody has cannot be found, so call it out separately.
    let (mia, findable): (Vec<&datary::Game>, Vec<&datary::Game>) =
        missing.into_iter().partition(|g| g.is_mia());
    for game in findable.iter().take(20) {
        println!("  missing  {}", game.name);
    }
    if findable.len() > 20 {
        println!("  ... and {} more", findable.len() - 20);
    }
    if !mia.is_empty() {
        println!("({} of the missing are flagged MIA)", mia.len());
    }

    Ok(())
}
