mod bencode_decoder;
mod torrent;

use core::panic;
use std::{fs, io::Read, path::PathBuf};

use bencode_decoder::Bencode;
use clap::Parser;
use torrent::Torrent;

#[derive(Parser)]
struct Cli {
    action: String,
    path: PathBuf,
}

fn main() {
    let args = Cli::parse();

    println!("action: {:?}, path: {:?}", args.action, args.path);

    let mut file = fs::File::open(args.path).expect("could not read file");
    let mut content = Vec::new();
    file.read_to_end(&mut content).unwrap();

    let (metainfo, _) = Bencode::decode_value(content);
    let torrent = Torrent::parse(&metainfo);

    match args.action.as_str() {
        "info" => {
            println!("Tracker URL: {}", torrent.announce);
            println!("Files: \n{}", torrent.info.file_tree);
            println!("Info Hash: {}", torrent.info.get_infohash());
            println!("Piece Length: {}", torrent.info.piece_length);
        }
        _ => panic!("invalid argument"),
    }
}
