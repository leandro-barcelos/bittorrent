mod bencode_decoder;

use indexmap::IndexMap;

use crate::bencode_decoder::Bencode;

struct Torrent {
    announce: String,
    info: Info,
    piece_layers: IndexMap<Vec<u8>, Vec<u8>>,
}

struct Info {
    name: String,
    piece_length: u32,
    meta_version: u8,
    file_tree: IndexMap<String, FileTree>,
    length: u32,
    pieces_root: Vec<u8>,
}

#[derive(PartialEq, Debug)]
enum FileTree {
    File(File),
    Directory(IndexMap<String, FileTree>),
}

#[derive(PartialEq, Debug)]
struct File {
    length: u32,
    pieces_root: Option<Vec<u8>>,
}

impl Torrent {
    fn parse_metainfo(metainfo: &Bencode) -> Self {
        if let Bencode::Dictionary(metainfo_dict) = metainfo {
            let mut announce = String::new();
            let mut info = Info {
                name: String::new(),
                piece_length: 0,
                meta_version: 0,
                file_tree: IndexMap::new(),
                length: 0,
                pieces_root: Vec::new(),
            };
            let mut piece_layers = IndexMap::new();

            for (k, v) in metainfo_dict.iter() {
                let key = String::from_utf8(k.clone()).unwrap();

                match key.as_str() {
                    "announce" => {
                        announce = match v {
                            Bencode::String(announce) => {
                                String::from_utf8_lossy(announce).to_string()
                            }
                            _ => panic!("Invalid or missing announce field"),
                        }
                    }
                    "info" => info = Info::parse_info(v),
                    "piece layers" => {
                        piece_layers = match v {
                            Bencode::Dictionary(piece_layers_dict) => piece_layers_dict
                                .iter()
                                .map(|(k, v)| {
                                    (
                                        k.clone(),
                                        match v {
                                            Bencode::String(v) => v.clone(),
                                            _ => panic!("Invalid piece layer value"),
                                        },
                                    )
                                })
                                .collect(),
                            _ => IndexMap::new(),
                        }
                    }
                    _ => panic!("Invalid key in metainfo"),
                }
            }

            return Torrent {
                announce,
                info,
                piece_layers,
            };
        } else {
            panic!("Error parsing torrent file");
        }
    }
}

impl Info {
    fn parse_info(info: &Bencode) -> Self {
        if let Bencode::Dictionary(info_dict) = info {
            let keys: Vec<String> = info_dict
                .keys()
                .map(|k| String::from_utf8_lossy(k).to_string())
                .collect();
            println!("{:?}", keys);

            let name = match info_dict.get(&b"name".to_vec()) {
                Some(Bencode::String(name)) => String::from_utf8_lossy(&name).to_string(),
                _ => panic!("Invalid or missing name field"),
            };

            let piece_length = match info_dict.get(&b"piece length".to_vec()) {
                Some(Bencode::String(piece_length)) => String::from_utf8(piece_length.clone())
                    .unwrap()
                    .parse::<u32>()
                    .unwrap(),
                _ => panic!("Invalid or missing piece length field"),
            };

            let meta_version = match info_dict.get(&b"meta version".to_vec()) {
                Some(Bencode::String(meta_version)) => String::from_utf8(meta_version.clone())
                    .unwrap()
                    .parse::<u8>()
                    .unwrap(),
                _ => panic!("Invalid or missing piece length field"),
            };

            let file_tree = match info_dict.get(&b"file tree".to_vec()) {
                Some(file_tree) => Info::parse_file_tree(file_tree.clone()).unwrap(),
                _ => panic!("Invalid or missing file tree field"),
            };

            let length = match info_dict.get(&b"length".to_vec()) {
                Some(Bencode::String(length)) => String::from_utf8(length.clone())
                    .unwrap()
                    .parse::<u32>()
                    .unwrap(),
                _ => panic!("Invalid or missing length field"),
            };

            let pieces_root = match info_dict.get(&b"pieces root".to_vec()) {
                Some(Bencode::String(pieces_root)) => pieces_root.clone(),
                _ => panic!("Invalid or missing pieces root field"),
            };

            Info {
                name,
                piece_length,
                meta_version,
                file_tree,
                length,
                pieces_root,
            }
        } else {
            panic!("Error parsing torrent file");
        }
    }

    fn parse_file_tree(file_tree_bencode: Bencode) -> Option<IndexMap<String, FileTree>> {
        if let Bencode::Dictionary(file_tree_dict) = file_tree_bencode {
            return Some(
                file_tree_dict
                    .iter()
                    .map(|(k, v)| {
                        (
                            String::from_utf8(k.clone()).unwrap(),
                            // Is a file if the child dictionary contains an empty key
                            if let Bencode::Dictionary(child) = v {
                                if let Some(Bencode::Dictionary(file_dict)) =
                                    child.get(&b"".to_vec())
                                {
                                    let length = match file_dict.get(&b"length".to_vec()) {
                                        Some(Bencode::String(length)) => {
                                            String::from_utf8(length.clone())
                                                .unwrap()
                                                .parse::<u32>()
                                                .unwrap()
                                        }
                                        _ => panic!("Invalid or missing length field"),
                                    };

                                    let pieces_root = match file_dict.get(&b"pieces root".to_vec())
                                    {
                                        Some(Bencode::String(pieces_root)) => {
                                            Some(pieces_root.clone())
                                        }
                                        _ => None,
                                    };

                                    let file = File {
                                        length,
                                        pieces_root,
                                    };

                                    FileTree::File(file)
                                } else {
                                    FileTree::Directory(Info::parse_file_tree(v.clone()).unwrap())
                                }
                            } else {
                                panic!("Expected dictionary");
                            },
                        )
                    })
                    .collect(),
            );
        } else {
            return None;
        }
    }
}

fn main() {
    // let mut file = File::open("test.torrent").unwrap();
    // let mut content = Vec::new();
    // file.read_to_end(&mut content).unwrap();

    // if let (Bencode::Dictionary(metainfo), _) = Bencode::decode_value(content) {
    //     if let Bencode::String(tracker_url) = &metainfo["announce"] {
    //         if let Bencode::Dictionary(info) = &metainfo["info"] {
    //             if let Bencode::Integer(length) = info["length"] {
    //                 println!("Tracker URL: {}", String::from_utf8_lossy(tracker_url));
    //                 println!("Length: {}", length);
    //             }
    //         }
    //     }
    // }
}

#[cfg(test)]
mod test {
    use std::{fs, io::Read};

    use super::*;

    #[test]
    fn test_sha_info() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        if let (Bencode::Dictionary(metainfo), _) = Bencode::decode_value(content) {
            let info_bytes = metainfo[&b"info".to_vec()].clone().encode_value();
            let mut m = sha1_smol::Sha1::new();
            m.update(&info_bytes);

            assert_eq!(
                m.digest().to_string(),
                "9f85123ad678b49f081e7269d953560e2a4f53ef"
            );

            assert_eq!(
                sha256::digest(&info_bytes),
                "213d7245c6341d82a3b6661010e0129a88e3b02d97b848805e915f31d6545324"
            );
        };
    }

    #[test]
    fn test_metainfo_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

        assert_eq!(torrent.announce, "http://example.com/announce");
        assert_eq!(torrent.info.name, "test_folder");
        assert_eq!(torrent.info.piece_length, 65536);
        assert_eq!(torrent.info.meta_version, 2);

        let melk_abbey_library = File {
            length: 1682177,
            pieces_root: Some(
                "9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe"
                    .as_bytes()
                    .to_vec(),
            ),
        };
        let loc_main = File {
            length: 14657,
            pieces_root: Some(
                "90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9"
                    .as_bytes()
                    .to_vec(),
            ),
        };

        let mut images_content = IndexMap::new();
        images_content.insert(
            "melk-abbey-library.jpg".to_string(),
            FileTree::File(melk_abbey_library),
        );
        images_content.insert(
            "LOC_Main_Reading_Room_Highsmith.jpg ".to_string(),
            FileTree::File(loc_main),
        );

        let readme = File {
            length: 20,
            pieces_root: Some(
                "c87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe35"
                    .as_bytes()
                    .to_vec(),
            ),
        };

        let mut file_tree = IndexMap::new();
        file_tree.insert("images".to_string(), FileTree::Directory(images_content));
        file_tree.insert("README".to_string(), FileTree::File(readme));

        assert_eq!(torrent.info.file_tree, file_tree);
    }
}
