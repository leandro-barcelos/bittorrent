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
    file_tree: FileTree,
}

#[derive(PartialEq, Debug)]
enum FileTree {
    File(String, File),
    Directory(String, Vec<FileTree>),
}

impl Default for FileTree {
    fn default() -> Self {
        FileTree::Directory(String::new(), Vec::new())
    }
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
                        file_tree = FileTree::Directory("".to_string(), FileTree::parse(value));
                    }
                    }
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

impl FileTree {
    fn parse(file_tree: &Bencode) -> Vec<FileTree> {
        if let Bencode::Dictionary(file_tree_dict) = file_tree {
            let mut content = Vec::new();

            for (k, v) in file_tree_dict.iter() {
                let name = String::from_utf8(k.clone()).unwrap();

                            match v {
                                Bencode::Dictionary(child) => {
                        if let Some(Bencode::Dictionary(file_dict)) = child.get(&b"".to_vec()) {
                                        let length = match file_dict.get(&b"length".to_vec()) {
                                Some(Bencode::Integer(length_int)) => *length_int as u32,
                                            _ => panic!("Invalid or missing length field"),
                                        };

                            let pieces_root = match file_dict.get(&b"pieces root".to_vec()) {
                                                Some(Bencode::String(pieces_root_bytes)) => {
                                                    Some(pieces_root_bytes.clone())
                                                }
                                                _ => None,
                                            };

                                        let file = File {
                                            length,
                                            pieces_root,
                                        };

                            content.push(FileTree::File(name, file));
                                    } else {
                            content.push(FileTree::Directory(name, FileTree::parse(&v)));
                                    }
                                }
                                _ => panic!("Expected dictionary"),
                }
            }

            return content;
        } else {
            panic!("Expected dictionary when parsing file tree")
        }
    }

    fn to_bencode(&self) -> Bencode {
        if let FileTree::Directory(_, contents) = self {
            let mut file_tree = IndexMap::new();

            for content in contents {
                match content {
                    FileTree::File(name, file) => {
                        let mut inner_description = IndexMap::new();
                        inner_description
                            .insert(b"length".to_vec(), Bencode::Integer(file.length.into()));
                        if let Some(ref pieces_root) = file.pieces_root {
                            inner_description.insert(
                                b"pieces root".to_vec(),
                                Bencode::String(pieces_root.clone()),
            );
                        }

                        let inner_description_bencode = Bencode::Dictionary(inner_description);

                        let mut description = IndexMap::new();
                        description.insert(b"".to_vec(), inner_description_bencode);

                        let description_bencode = Bencode::Dictionary(description);

                        file_tree.insert(name.as_bytes().to_vec(), description_bencode);
                    }
                    FileTree::Directory(name, _) => {
                        file_tree.insert(name.as_bytes().to_vec(), content.to_bencode());
                    }
                }
            }

            Bencode::Dictionary(file_tree)
        } else {
            panic!("It is only possible to convert directories to Bencode")
        }
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

    #[test]
    fn test_file_tree_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        let readme = FileTree::File(
            "README".to_string(),
            File {
                length: 20,
            pieces_root: Some(
                    hex::decode("c87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe35")
                        .unwrap(),
            ),
            },
        );

        let loc_main = FileTree::File(
            "LOC_Main_Reading_Room_Highsmith.jpg".to_string(),
            File {
                length: 17614527,
            pieces_root: Some(
                    hex::decode("90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9")
                        .unwrap(),
            ),
            },
        );

        let melk_abbey_library = FileTree::File(
            "melk-abbey-library.jpg".to_string(),
            File {
                length: 1682177,
                pieces_root: Some(
                    hex::decode("9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe")
                        .unwrap(),
                ),
            },
        );

        let mut images_content = Vec::new();
        images_content.push(loc_main);
        images_content.push(melk_abbey_library);

        let images = FileTree::Directory("images".to_string(), images_content);

        let mut file_tree_content = Vec::new();
        file_tree_content.push(readme);
        file_tree_content.push(images);

        let file_tree = FileTree::Directory("".to_string(), file_tree_content);

        assert_eq!(torrent.info.file_tree, file_tree);
    }

    #[test]
    fn test_file_tree_to_bencode() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(String::from_utf8_lossy(&torrent.info.file_tree.to_bencode().encode_value()).into_owned(), String::from_utf8_lossy(&hex::decode("64363a524541444d4564303a64363a6c656e6774686932306531313a70696563657320726f6f7433323ac87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe356565363a696d616765736433353a4c4f435f4d61696e5f52656164696e675f526f6f6d5f48696768736d6974682e6a706764303a64363a6c656e6774686931373631343532376531313a70696563657320726f6f7433323a90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9656532323a6d656c6b2d61626265792d6c6962726172792e6a706764303a64363a6c656e67746869313638323137376531313a70696563657320726f6f7433323a9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe65656565").unwrap()).into_owned());
    }
}
