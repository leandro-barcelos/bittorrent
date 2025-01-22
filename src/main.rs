mod bencode_decoder;

use std::collections::HashMap;

use crate::bencode_decoder::Bencode;

struct Torrent {
    announce: String,
    info: Info,
    piece_layers: HashMap<Vec<u8>, Vec<u8>>,
}

struct Info {
    name: String,
    piece_length: u32,
    meta_version: u8,
    file_tree: HashMap<String, FileTree>,
    length: u32,
    pieces_root: Vec<u8>,
}

#[derive(PartialEq, Debug)]
enum FileTree {
    File(File),
    Directory(HashMap<String, FileTree>),
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
                file_tree: HashMap::new(),
                length: 0,
                pieces_root: Vec::new(),
            };
            let mut piece_layers = HashMap::new();

            for (k, mut v) in metainfo_dict.iter() {
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
                    "info" => info = Info::parse_info(&mut v),
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
                            _ => HashMap::new(),
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
            let mut name = String::new();
            let mut piece_length = 0;
            let mut meta_version = 0;
            let mut file_tree = HashMap::new();
            let mut length = 0;
            let mut pieces_root = Vec::new();

            for (key, value) in info_dict.iter() {
                match String::from_utf8_lossy(&key).as_ref() {
                    "name" => {
                        if let Bencode::String(name_bytes) = value {
                            name = String::from_utf8_lossy(&name_bytes).to_string();
                        }
                    }
                    "piece length" => {
                        if let Bencode::Integer(piece_length_int) = value {
                            piece_length = *piece_length_int as u32;
                        }
                    }
                    "meta version" => {
                        if let Bencode::Integer(meta_version_int) = value {
                            meta_version = *meta_version_int as u8;
                        }
                    }
                    "file tree" => {
                        file_tree = Info::parse_file_tree(value).unwrap();
                    }
                    "length" => {
                        if let Bencode::Integer(length_int) = value {
                            length = *length_int as u32;
                        }
                    }
                    "pieces root" => {
                        if let Bencode::String(pieces_root_bytes) = value {
                            pieces_root = pieces_root_bytes.to_vec();
                        }
                    }
                    _ => println!(
                        "Invalid key in info dictionary: {:?}",
                        String::from_utf8_lossy(&key)
                    ),
                }
            }

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

    fn parse_file_tree(file_tree_bencode: &Bencode) -> Option<HashMap<String, FileTree>> {
        if let Bencode::Dictionary(file_tree_dict) = file_tree_bencode {
            return Some(
                file_tree_dict
                    .iter()
                    .map(|(k, v)| {
                        (
                            String::from_utf8(k.clone()).unwrap(),
                            match v {
                                Bencode::Dictionary(child) => {
                                    if let Some(Bencode::Dictionary(file_dict)) =
                                        child.get(&b"".to_vec())
                                    {
                                        let length = match file_dict.get(&b"length".to_vec()) {
                                            Some(Bencode::Integer(length_int)) => {
                                                *length_int as u32
                                            }
                                            _ => panic!("Invalid or missing length field"),
                                        };

                                        let pieces_root =
                                            match file_dict.get(&b"pieces root".to_vec()) {
                                                Some(Bencode::String(pieces_root_bytes)) => {
                                                    Some(pieces_root_bytes.clone())
                                                }
                                                _ => None,
                                            };

                                        let file = File {
                                            length,
                                            pieces_root,
                                        };

                                        FileTree::File(file)
                                    } else {
                                        FileTree::Directory(Info::parse_file_tree(v).unwrap())
                                    }
                                }
                                _ => panic!("Expected dictionary"),
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
    use std::{collections::HashMap, fs, io::Read};

    use super::*;

    #[test]
    fn test_sha1_info() {
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
        };
    }

    #[test]
    fn test_sha256_info() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        if let (Bencode::Dictionary(metainfo), _) = Bencode::decode_value(content) {
            let info_bytes = metainfo[&b"info".to_vec()].clone().encode_value();

            assert_eq!(
                sha256::digest(&info_bytes),
                "213d7245c6341d82a3b6661010e0129a88e3b02d97b848805e915f31d6545324"
            );
        };
    }

    #[test]
    fn test_announce_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

        assert_eq!(torrent.announce, "http://example.com/announce");
    }

    fn test_info_name_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

        assert_eq!(torrent.info.name, "test_folder");
    }

    fn test_piece_length_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

        assert_eq!(torrent.info.piece_length, 65536);
    }

    fn test_meta_version_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

        assert_eq!(torrent.info.meta_version, 2);
    }

    fn test_file_tree_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse_metainfo(&metainfo);

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

        let mut images_content = HashMap::new();
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

        let mut file_tree = HashMap::new();
        file_tree.insert("images".to_string(), FileTree::Directory(images_content));
        file_tree.insert("README".to_string(), FileTree::File(readme));

        assert_eq!(torrent.info.file_tree, file_tree);
    }
}
