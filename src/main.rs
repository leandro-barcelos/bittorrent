mod bencode_decoder;

use core::panic;
use std::{collections::HashMap, fs, io::Read, path::PathBuf};

use clap::Parser;
use indexmap::IndexMap;

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

impl Default for Info {
    fn default() -> Self {
        Info {
            name: String::new(),
            piece_length: 0,
            meta_version: 0,
            file_tree: FileTree::default(),
        }
    }
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

#[derive(PartialEq, Debug, Clone)]
struct File {
    length: u32,
    pieces_root: Vec<u8>,
}

impl Torrent {
    fn parse(metainfo: &Bencode) -> Self {
        if let Bencode::Dictionary(metainfo_dict) = metainfo {
            let mut announce = String::new();
            let mut info = Info::default();
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
                    "info" => info = Info::parse(&mut v),
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
                    _ => println!("Invalid key in metainfo: {:?}", key),
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

    fn verify_infohash(&self, infohash: String) -> bool {
        if infohash != self.info.get_infohash() {
            println!("Infohash is invalid");
            return false;
        }

        let mut files = Vec::new();
        self.info.file_tree.get_files(&mut files);

        let piece_roots: Vec<Vec<u8>> = files.iter().map(|file| file.pieces_root.clone()).collect();

        for key in self.piece_layers.keys() {
            if !piece_roots.contains(key) {
                println!("Piece root not found: {:?}", key);
                return false;
            }
        }

        return true;
    }
}

impl Info {
    fn parse(info: &Bencode) -> Self {
        if let Bencode::Dictionary(info_dict) = info {
            let mut name = String::new();
            let mut piece_length = 0;
            let mut meta_version = 0;
            let mut file_tree = FileTree::default();

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
            }
        } else {
            panic!("Error parsing torrent file");
        }
    }

    fn to_bencode(&self) -> Bencode {
        let mut info = IndexMap::new();

        let file_tree = self.file_tree.to_bencode();
        info.insert(b"file tree".to_vec(), file_tree);

        let meta_version = Bencode::Integer(self.meta_version.into());
        info.insert(b"meta version".to_vec(), meta_version);

        let name = Bencode::String(self.name.as_bytes().to_vec());
        info.insert(b"name".to_vec(), name);

        let piece_length = Bencode::Integer(self.piece_length.into());
        info.insert(b"piece length".to_vec(), piece_length);

        Bencode::Dictionary(info)
    }

    fn get_infohash(&self) -> String {
        let info_bytes = self.to_bencode().encode_value();

        sha256::digest(&info_bytes)
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
                                    pieces_root_bytes.clone()
                                }
                                _ => panic!("Invalid or missing pieces root field"),
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

                        inner_description.insert(
                            b"pieces root".to_vec(),
                            Bencode::String(file.pieces_root.clone()),
                        );

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

    fn get_files(&self, files: &mut Vec<File>) {
        match self {
            FileTree::Directory(_, contents) => {
                contents.iter().for_each(|content| content.get_files(files))
            }
            FileTree::File(_, file) => files.push(file.clone()),
        };
    }
}

impl std::fmt::Display for FileTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn fmt_helper(
            tree: &FileTree,
            path: &str,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            match tree {
                FileTree::File(name, file) => {
                    writeln!(
                        f,
                        "\t{}{} (length: {}, pieces_root: {})",
                        path,
                        name,
                        file.length,
                        hex::encode(&file.pieces_root)
                    )
                }
                FileTree::Directory(name, contents) => {
                    let new_path = format!("{}{}/", path, name);
                    for content in contents {
                        fmt_helper(content, &new_path, f)?;
                    }
                    Ok(())
                }
            }
        }

        fmt_helper(self, "", f)
    }
}

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

#[cfg(test)]
mod test {
    use std::{fs, io::Read};

    use super::*;

    #[test]
    fn test_infohash() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        let info_bytes = torrent.info.to_bencode().encode_value();

        assert_eq!(
            sha256::digest(&info_bytes),
            "22fd2f407dd4187ca9b77b7937587f53346f0aebe326a2a8ac583e3b8cfc8bdd"
        );
    }

    #[test]
    fn test_announce_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(torrent.announce, "http://example.com/announce");
    }

    #[test]
    fn test_info_name_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(torrent.info.name, "test_folder");
    }

    #[test]
    fn test_piece_length_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(torrent.info.piece_length, 65536);
    }

    #[test]
    fn test_meta_version_parsing() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

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
                pieces_root: hex::decode(
                    "c87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe35",
                )
                .unwrap(),
            },
        );

        let loc_main = FileTree::File(
            "LOC_Main_Reading_Room_Highsmith.jpg".to_string(),
            File {
                length: 17614527,
                pieces_root: hex::decode(
                    "90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9",
                )
                .unwrap(),
            },
        );

        let melk_abbey_library = FileTree::File(
            "melk-abbey-library.jpg".to_string(),
            File {
                length: 1682177,
                pieces_root: hex::decode(
                    "9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe",
                )
                .unwrap(),
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

    #[test]
    fn test_info_to_bencode() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(String::from_utf8_lossy(&torrent.info.to_bencode().encode_value()).into_owned(), String::from_utf8_lossy(&hex::decode("64393a66696c65207472656564363a524541444d4564303a64363a6c656e6774686932306531313a70696563657320726f6f7433323ac87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe356565363a696d616765736433353a4c4f435f4d61696e5f52656164696e675f526f6f6d5f48696768736d6974682e6a706764303a64363a6c656e6774686931373631343532376531313a70696563657320726f6f7433323a90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9656532323a6d656c6b2d61626265792d6c6962726172792e6a706764303a64363a6c656e67746869313638323137376531313a70696563657320726f6f7433323a9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe6565656531323a6d6574612076657273696f6e693265343a6e616d6531313a746573745f666f6c64657231323a7069656365206c656e6774686936353533366565").unwrap()).to_owned());
    }

    #[test]
    fn test_file_tree_get_files() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        let readme = File {
            length: 20,
            pieces_root: hex::decode(
                "c87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe35",
            )
            .unwrap(),
        };

        let loc_main = File {
            length: 17614527,
            pieces_root: hex::decode(
                "90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9",
            )
            .unwrap(),
        };

        let melk_abbey_library = File {
            length: 1682177,
            pieces_root: hex::decode(
                "9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe",
            )
            .unwrap(),
        };

        let mut files = Vec::new();
        torrent.info.file_tree.get_files(&mut files);

        println!("{:?}", files);

        assert_eq!(files, vec![readme, loc_main, melk_abbey_library]);
    }

    #[test]
    fn test_verify_infohash() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert!(torrent.verify_infohash(
            "22fd2f407dd4187ca9b77b7937587f53346f0aebe326a2a8ac583e3b8cfc8bdd".to_string()
        ))
    }
}
