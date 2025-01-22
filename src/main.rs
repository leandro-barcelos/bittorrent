mod bencode_decoder;

use core::panic;
use std::{collections::HashMap, fs, io::Read, path::PathBuf};
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
    pieces: Option<Vec<Vec<u8>>>,
    private: Option<bool>,
    file_mode: Option<FileModeV1>,
}

impl Default for Info {
    fn default() -> Self {
        Info {
            name: String::new(),
            piece_length: 0,
            meta_version: 0,
            file_tree: FileTree::default(),
            pieces: None,
            private: None,
            file_mode: None,
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

#[derive(PartialEq, Debug)]
struct File {
    length: u32,
    pieces_root: Option<Vec<u8>>,
}

enum FileModeV1 {
    Single {
        length: u32,
        md5sum: Option<Vec<u8>>,
    },
    Multiple {
        files: Vec<FileV1>,
    },
}

struct FileV1 {
    length: u32,
    md5sum: Option<Vec<u8>>,
    path: PathBuf,
    attr: Option<char>,
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
    fn parse(info: &Bencode) -> Self {
        if let Bencode::Dictionary(info_dict) = info {
            let mut name = String::new();
            let mut piece_length = 0;
            let mut meta_version = 0;
            let mut file_tree = FileTree::default();
            let mut pieces: Option<Vec<Vec<u8>>> = None;
            let mut private: Option<bool> = None;

            let mut length: Option<u32> = None;
            let mut md5sum: Option<Vec<u8>> = None;
            let mut files: Option<Vec<FileV1>> = None;

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
                    "pieces" => {
                        if let Bencode::String(pieces_bytes) = value {
                            pieces = Some(
                                pieces_bytes
                                    .chunks(20)
                                    .map(|chunk| chunk.to_vec())
                                    .collect(),
                            );
                        }
                    }
                    "private" => {
                        if let Bencode::Integer(private_int) = value {
                            private = Some(*private_int != 0);
                    }
                    }
                    "length" => {
                        if let Bencode::Integer(length_int) = value {
                            length = Some(*length_int as u32);
                        }
                    }
                    "md5sum" => {
                        if let Bencode::String(md5sum_bytes) = value {
                            md5sum = Some(md5sum_bytes.to_vec());
                        }
                    }
                    "files" => {
                        if let Bencode::List(files_list) = value {
                            let mut files_vec = Vec::new();

                            for file_bencode in files_list {
                                if let Bencode::Dictionary(file) = file_bencode {
                                    let attr = match file.get(&b"attr".to_vec()) {
                                        Some(Bencode::String(attr)) => Some(
                                            String::from_utf8_lossy(attr).chars().next().unwrap(),
                                        ),
                                        _ => None,
                                    };

                                    let length = match file.get(&b"length".to_vec()) {
                                        Some(Bencode::Integer(length)) => *length as u32,
                                        _ => panic!("Invalid or missing length field"),
                                    };

                                    let md5sum = match file.get(&b"md5sum".to_vec()) {
                                        Some(Bencode::String(md5sum)) => Some(md5sum.to_vec()),
                                        _ => None,
                                    };

                                    let path = match file.get(&b"path".to_vec()) {
                                        Some(Bencode::List(path_list)) => {
                                            let components: Vec<String> = path_list
                                                .iter()
                                                .filter_map(|component| {
                                                    if let Bencode::String(name) = component {
                                                        Some(
                                                            String::from_utf8_lossy(name)
                                                                .to_string(),
                                                        )
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();
                                            PathBuf::from(components.join("/"))
                                        }
                                        _ => panic!("Invalid or missing path field"),
                                    };

                                    files_vec.push(FileV1 {
                                        length,
                                        md5sum,
                                        path,
                                        attr,
                                    });
                                }
                            }

                            files = Some(files_vec);
                        }
                    }
                    _ => println!(
                        "Invalid key in info dictionary: {:?}",
                        String::from_utf8_lossy(&key)
                    ),
                }
            }

            let file_mode = if let Some(files) = files {
                Some(FileModeV1::Multiple { files })
            } else if let Some(length) = length {
                Some(FileModeV1::Single {
                    length,
                    md5sum: Some(md5sum.unwrap_or_default()),
                })
            } else {
                None
            };

            Info {
                name,
                piece_length,
                meta_version,
                file_tree,
                pieces,
                private,
                file_mode,
            }
        } else {
            panic!("Error parsing torrent file");
        }
    }

    fn to_bencode(&self) -> Bencode {
        let mut info = IndexMap::new();

        let file_tree = self.file_tree.to_bencode();
        info.insert(b"file tree".to_vec(), file_tree);

        if let Some(file_mode) = &self.file_mode {
            match file_mode {
                FileModeV1::Single { length, md5sum } => {
                    info.insert(b"length".to_vec(), Bencode::Integer(*length as i64));
                    if let Some(md5sum_) = md5sum {
                        info.insert(b"md5sum".to_vec(), Bencode::String(md5sum_.to_vec()));
                    }
                }
                FileModeV1::Multiple { files } => {
                    let mut files_vec = Vec::new();

                    for file in files {
                        let mut file_map = IndexMap::new();

                        if let Some(attr) = &file.attr {
                            file_map.insert(
                                b"attr".to_vec(),
                                Bencode::String(attr.to_string().into_bytes()),
                            );
                        }

                        file_map.insert(b"length".to_vec(), Bencode::Integer(file.length.into()));

                        if let Some(md5sum) = &file.md5sum {
                            file_map.insert(b"md5sum".to_vec(), Bencode::String(md5sum.to_vec()));
                        }

                        let path_str = file.path.to_str().unwrap();
                        let path_components: Vec<Bencode> = path_str
                            .split('/')
                            .map(|component| Bencode::String(component.as_bytes().to_vec()))
                            .collect();
                        file_map.insert(b"path".to_vec(), Bencode::List(path_components));

                        files_vec.push(Bencode::Dictionary(file_map));
                    }

                    info.insert(b"files".to_vec(), Bencode::List(files_vec));
                }
            }
        }

        let meta_version = Bencode::Integer(self.meta_version.into());
        info.insert(b"meta version".to_vec(), meta_version);

        let name = Bencode::String(self.name.as_bytes().to_vec());
        info.insert(b"name".to_vec(), name);

        let piece_length = Bencode::Integer(self.piece_length.into());
        info.insert(b"piece length".to_vec(), piece_length);

        if let Some(private) = &self.private {
            info.insert(b"private".to_vec(), Bencode::Integer(*private as i64));
        }

        if let Some(pieces) = &self.pieces {
            let concatenated_pieces: Vec<u8> = pieces.iter().flatten().cloned().collect();
            info.insert(b"pieces".to_vec(), Bencode::String(concatenated_pieces));
        }

        Bencode::Dictionary(info)
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
    use std::{fs, io::Read};

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

    #[test]
    fn test_info_to_bencode() {
        let mut file = fs::File::open("test_folder.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        let (metainfo, _) = Bencode::decode_value(content);
        let torrent = Torrent::parse(&metainfo);

        assert_eq!(String::from_utf8_lossy(&torrent.info.to_bencode().encode_value()).into_owned(), String::from_utf8_lossy(&hex::decode("64393a66696c65207472656564363a524541444d4564303a64363a6c656e6774686932306531313a70696563657320726f6f7433323ac87e2ca771bab6024c269b933389d2a92d4941c848c52f155b9b84e1f109fe356565363a696d616765736433353a4c4f435f4d61696e5f52656164696e675f526f6f6d5f48696768736d6974682e6a706764303a64363a6c656e6774686931373631343532376531313a70696563657320726f6f7433323a90a24c4b7a34568fc4a2a62a0079204e9766e19f9a0069546189f120017656f9656532323a6d656c6b2d61626265792d6c6962726172792e6a706764303a64363a6c656e67746869313638323137376531313a70696563657320726f6f7433323a9e2f0845f16dcb0844fa09370622fd211027c9300838b021502fd7a63a452ffe65656565353a66696c65736c64363a6c656e67746869323065343a706174686c363a524541444d45656564343a61747472313a70363a6c656e67746869363535313665343a706174686c343a2e706164353a3635353136656564363a6c656e67746869313736313435323765343a706174686c363a696d6167657333353a4c4f435f4d61696e5f52656164696e675f526f6f6d5f48696768736d6974682e6a7067656564343a61747472313a70363a6c656e67746869313436353765343a706174686c343a2e706164353a3134363537656564363a6c656e677468693136383231373765343a706174686c363a696d6167657332323a6d656c6b2d61626265792d6c6962726172792e6a7067656564343a61747472313a70363a6c656e67746869323137353965343a706174686c343a2e706164353a323137353965656531323a6d6574612076657273696f6e693265343a6e616d6531313a746573745f666f6c64657231323a7069656365206c656e67746869363535333665363a706965636573353932303a57e21752b0e113df1e0a0c4e88c3baf6f8047960ad49e82b7e20cb2817706be978ac4a903c85575b8ba140006eabdba3cb8874b0604272c0916184e12b9ae6286dcff124257f243db609b50e83b085d2d05e84223eb22ca28785820cef0451622f84a806bd554c25a941b27408e0ebf06b0537b2075c5e9f894bf541b9656a0e36fa3834eeb58c7f4f07d6c6473bd9413fcc6399dc65f7bcb1d21fb824217fd85517d365dda9ed804fd8f39418744a643d17005df55535ebb078299340ecb7a48d0ef077cacaf120ddf9a4c609aae773333e996748778d24cf1b88a04c47c00c891dd3104df25bf2b7b5af26fab7b20ea5edbd2528d216bb9139e03cf310d9a76b42e1011f50ee9eaf740aa12dfe2901122834227a853d1c1bc606916d4496ef57da66954d457add2dd06c36d45a5361e49a76d55f969e391f1ea03e9b5bf28f16f13a497e25f088d1d1c8f09eeed4bc9db370266afa2688e01ca94ab714ceea486dc5cbbda9ce5d33d5d4cf706ba35a27c019b4395a5e5c784fbba68086af67055064e8220e9b7196b2666e933c2bfc0b2925fa59f75651b093c683feb6a6331bc808f4f2978b3504e1da784302a32e7b8249e9bea58a56cc03d7fafa4711e236cb83fafa989f9b31b155d2346935678d934fb4d76679b3b8fe32ca69454ebb7af57b879f29bfdb121671cdccef178f5b4a3d9e3734d99c0d1ea13f0c48f6a9723ad2c92cce9d0a55892bfedb30c2ee4fa01f2c38b264ac06559b55ae302a91bda504c82f3d592f7a0162278a1f63239419d514c203ef5edab2ab86c726c0b1a525fdd6662f83bb5ccc684b3ec5df321a5378299ff499ec99f206c5af10f329f29156fa3977f780838792da3b00c727766b427a6bfe2c77691a2b409cb412c26bcc4d3ee7e8d6dd04d17a062cd6b4030021318b75b03aa376605604dace91f2a7571d7fd2e438f121a1f1ecf04ebbf7f03a34c82d08793293ec9aca07c1c342a62321806d5b00bf6a5220da56bbc75fe67e3c223b82b0d1221f66246526ac70ea762f92c54755659093d405646b1322ca73d07609d348f9000712e9e80d4414d918886c2447e96dd749ca48ef9eedcc2a8c8ac3ede6f505c3a8c4735cfb11911f8eb39b1d83843d81fe1f77f0531420d0a370ca81d2e0f76d6718ffda6c9fb26e94d366f1cb5ed58ad47c8592c61dac30dd5480a57fd6ab778350aa51680a21379c736acc9d7c820c0c28e952f3ba27a09295c6e6d4aa3edd1ac5e8d4b4d3bac451780877b5a983390edac82cd9faa73c1c51a8bcc0109b53e942f4e75648cc7a1610530eb3472e8262935d913b855c5ec0f9c5430b87cfe695b68615dbc34d6eeed7cb20c943875525b2aad2e5d6acd91ec483b708514eabd34dd12ab30712ec3b20bff2b0ac38b81e1dc46817fdfd805fb233c824f50559f754ffa49f4e05556879bbffe567dda3d800b5984a620ff0a2bf92c907ad7a787d33bcd95ac344bb83d9d8710f6720f0de8055ed4a6a11a56c4974ad0ee87ae031cc44cf1a52b97411950e77ca6f59e52a5fe2529368b8f021076b0ac98f3e4a737c50da4da734738f32596b45f50e7af176894af666be00fcdf07c3c72814115467f2c308d99b5ac794ae09cf10e3f71a187f214800b9d0a0a56ead89222ec4d47497443b8ecc6276ceeedd3d12af497e1b0a5b6e786f29c15bbac09f7885d8dc53c4d72b85ba5d149e6b6716e998e98876648f78084dbf679972120391bef3416491653ab583e76677a70b38e18c0aa58e7480bafea8ca60678dd15760d09345b7d171a7059986976092c6a18d216915bed32c7e1a2693f618380a21d91672a4bf71ce85e8b4a3dacca647122690ea4fd35d1190afad0aa4ded42af2422f0b571567a64013f7d6dbee9511a21fc0d1817027de17e458fbcbe18a6611603dd60cd23c9b798292933a7df6624457b08762bd8645b420d72cbc9568467b0fae62eb70db8087e7db8eca82845ec8b0ed13d988d58e07346392c3876b6b267bf6ba5113f50d0cbd97269f8a449be6d713506e1430cd407a1135401bc95b1586fca5415cb02215fee9147a4b4f110e715db5ea05a3e690732c3f8f180e05c2a3083a8f8bba5919596a6719a0bbe62f59da56b717e5617a7a230d043f43d777ec70523ad739fb2fe22e28fe98d614b3825bf67bf9ccc87e106936a211f85c0ef36e0d8b7450b8b9604b995beb05938040b58819ba0f8c05d759cae19709391a032edd1c4bc6363dc12ddf28c5ee33e7226b179c2e6c4c8f21cf8736e2fa864896e290d2bc3f04513b681eb1c6d5c27996c029bdad7839c978c39ae88cb00245b8c02382694d6821bee55783f3bd082643155399dea6b40e51e38a7c0868bea06a7f62759a2ee80e650dd0934030cea9189f1f3491d439e13f7f59b4742d45ab900f11e836a2ae12a4c553bd14fdf1c76cca4c5734e44728aaf9a8880e922ec1d04c603f7e30ec24dc692f2c48b26596e3845a5a3988823ae6dd0f834a72991de40317de908b3dede3a3fffa3e1fd1e62dc247b97e2e0a796c6d1495a84f6f79e5af340bd2f59540f77ae9bdc8c7052c0ff76fac249b50cc3fc24031954ce647d5a7edeff85756db73e3049dcffc51e1cd4231783066a2dd96283eb0c0a12c05c6b4e3c5f6df97ff7a67bdf9ad8079deca625c62c803abd7e49b4be93abbc2320061442f32fc969aa6102c0b4b9646c358dc9df0294fead68ca320cf94d6cfced9cca759e8fa62fe8ca9ef4f14366314c55e313538b24d9f1cd4b710626b83bfc6054fa21595e646208988321ce506a01f76f78de17daaa13585313790c9a873010312162e8618677d13700f1cd7baf1652e10e48e0a3d25db2f6eeacf253c7007474f34ab35553cab942f771a22cdb31d4ab6d2d286d355a8f130b2e6a14b09d1cab3faa93254d54f11efee776018d229f78fb25fc361517ac339ade84afae0d0fd7c6c553623081e0cfdf9726850d4d8bb701d46358a46e6333bea6c30207b93718018ce4f8f50de72404b8197f8e47873f3b4dd27e0523fb783ceeb79f50b9ceb6e530e4b6610a519846479b27a539f6b612d0eaea344d21901f30284108bdc31b1fc27bb242c0772c94583361256025a276bf90f24f84642eb81a795de465bf5e8c2d33230a2d683181002199469b32b16aac7330066da60d4d951f0278fa7cd8b0ea43f6b4d2cfb82d875e637f4e58ac0a45de7ada1287d47ce3b81489c2860cb96548384ff090355525eb7c29ae055952006871df1e83247456bebc96359664c1cbeb43182b6945d94c115e1fe1c0a83afde78d35ef061c5ea0d8f228f80ea0735f4267239d15fab75cc08cae3734c4969fa8e7288446a4957f9e8136807fe818141c6e62a82e7cb6c98338295daa120ba7e7213f829a6a85d09e1f18b588a323f9bdb96b039e8e3f3d0369d3709751a2a80cff340b69e7ded24fe0994506209345d0d0077cf56985df4158a5723cbee21634612932e6f672868a5801528fd8c7f0fb60cad0cea12604c5f38c0550a8611000c703283228e08de78951248ccafa7bfcd6cc0fc3cf8135e838f595e170be4ef1c84458a09d4aae2d11b2ef59ecc5787f5555c5cc1983c1a220d6b7db5e751d39b7e631047e57c570a3478e9f88fa447a01d0fa5001e1859a1748f67bd24e73f78c45824b8d4d207ab4e2de714318fd3631e6425005f5a7a4b89b6ed11629f7acb1af5d60d75369ccfd4cfff481af962004c1594aee2b2c39092f75a67c6015516deec7efee3e5e86da48ce1b4993cb8bb3120e86fcb4a1e99f7455fce7fbf28c99caedbb80166a37bac3280147d65997dbdae67ce9ae843caa76278b1a754a7354824ac0ebfef8d1b96baee1960dae5fa27f4c2d664efe09b4aabf5b13bb670f160312759be54ca893445380dfa5e773ad6445dcbdeed10000735ec8b39338f543a54c090347488fcea0b0597d16d2ef6ebfc84f6a4ec894c0f5c2c596fdbf8ae7ac680e4090dd6e709f82ebf8d69976f54f572acc1401d342fa161624f1022d58fa9c9dde968d1684c678df1dafc75d5de53fae51bccf5b92d55a6531b2b245eee96759417593c51159970b6cd800d6368eb03b307753ad1f96680da4608c63dab9e7db3c71f352c1ce08f9b76b312e7ebddc9eabb4506972af288187977cf9021fe9c8bf491701fef1c9f6a8ba0d9670c55d3bd747cdce3fcccbde6aa3d32b416cb7bdd9fbcbefe41ab28e1456048654faeadbfbaf91179eb446da196e47328de1a5f884b09b123c546384ed2dbd8377507a77815f238e51ab6901ae44fb9d21b1efd70dbee7d1855834aa71b6cfb70c20330615ea4451e4cd5313dcee37b43cdab853c1ca9a584df0526b1093434e66dcfb56cfc169c9405cacd7e1e4302f6af2468697c7f079cfb11d58c20e8958b16d305f5659c38e95f7b1bc28005d8f5219f6df5ff797dd2bbd890ad295d9a8798b709319b9f3727393080c4bfdd2648d43e4542d12f768d6cff2a3e22328f70ed64423e05fa5958bdc759ad74baaef7ef85b20ef730010591588bd1611adff9cc78de352a3ff185003e9e0a5d569e5b5763cb6ed8324d0245446dce67b821bb2ad4483e7d4af0eee1a1054a24b70c5cee4564256569137840417dba7355ed94361955e91153f9d06beb440451e1749b08269126f101fe864b86b69fb80c63915c32ed9ddbd98ff38e98164acd6d683734f33630a09f3ae03e9830659ac684572cce3895f991e120701b98bdd719af47600843353f2ef9522e4f65ba7edb3b95996790bf5dc0688144bece037ae324b9a995a0476773e058a610662c3fdcaae4e9c004f8dc995640c7c8c6588275c7cb6add002c09284873b9ce89f886d36dabd016cc4d88a6a7e23e2032c2c073b2496f995531737adc7513d2ea29e67c7e1eb15615c909c46f2ae43904cc045f87d1ca3382f16b6f32d4a02cff9b9dac38d5271a7cd26c1f32353441d9448dcfa4a7994eba81dd124cb7f293b32466de671a23781b553f7d91c6b6885eb77000dfdc7e7bd1aa221494a416e70e1f8789eb2fbed4b1a339f462a9dc086ad13d76bdf9fbd6690be59b8ed157a8257d1f08ab85bde43cd23ff7c55a2d481508a9bded76ef24dc608afdd332727fcdf9446f230ad325febe8d874eabf68fc934908e6d55880a4dcbe9550b1547cd9fedaec15e6b196a88d7c627ff666cc9ab25c39a70f829ae2b8b8c166d978a0d9cd78c781c55629837bf70dd7a23008c3f7da9cb81592ed07d41a4e0621cc3d544fd54ce8f102f56726c309f7b1b9d44a8a25ac2b56665950aae9fb8b37ec27d4620971c5c8d157e1f70da8cfb08fb8391031f63e8228c95b6a28df2771a8f97583f340d5c03f03b0029d4f918fe6577b6d80e56ee7de9f8a93724691e6868fa13d4b2835582e4d3f4ef9ea1557b1f65dd0accc3e4fbd0e106fa8c4afe201e7827a233ec074ee75ff55326b492750ea93d48ac04f5fd5e990c938677d3edd6314c53b01769601b703c830bff47843924cb6d6b98132fed3cdb306a2457db0bb6b404329506fd4b55279faaa1ead9bcc4661fa80f5f3e0f5aa64226286835b9c2efcff22e069357fc9d147996f50e0fd8c0b25fdf2c3a1046d71687f988efd4a099c8041840abaa15b006f8e2d50e57af2092721ac16458362522057a7335d361ef3102c5679128f86c6ec2403e3dc7306ad31d7da71c7d039b3f0b1e3054d012ee4fb47db3a11c495fde072c60502191847bd06b023e374378d354842a2caeab0d4bd8cf6d60983b48c938071ace5f1bc76f158c4339db44c28dfadbfd04a4e0ce0fd49d97db74fa1fe7eec105007cafa7ff7e5b4e16d58609b2dedb1fa20dd8e3f8b239618629a201fc518e67b3d908ac8350b0ec4bc9170d2284948c9fdac4ecbf57c75862a30e5b61f7a18526a824ad7b4e83b91ad7369fa517b10ec1c5b8158eef37fd047d5acb5fd40c4ae4a01d745021f531d3acf0d029fdc59dc626c462a015e67413814d41bad47789d7bb643361777cf4859f7574ea75ec4de031947bd73c0a9420a441d4b9b6860d87db75739819aebdfa99c02f118b776a9aed127992a156d723199500a4a6d3a0c8deec249657ccf3728bf920853bf792bb7bf82f846941ebe8c0b6ff88d2d08b63e2e37901ba61478d138737f391b2c7f0f7fe07d866f400fb1078976331a4b31a1bdf33215be05c1cc065ac8e9bd827b0c62114e6e5aa3431ee0e08c8ea0029ed1d36e7699fca8c4e8fa0f0c5c35196060e1a9fa865aceef8e2e5a5ce7287316e7019ccb049e566a41016d67f22707fd1506ffbc8e5c03a2803a062090d204c0f341bd5a7d6a06ea9606c82be625e98be58c1afd3c80573bd133a5b201e8831f69534045c236b78ec07d314fc3b08ed7560ceeda8a85c5739278aec0287b39856f75d496658574417701cfb441b2222955e7aa354d37c29559d26daad4ff7bf4a7bc56d417a25e78785351c77919dc64ddc8f93fb2253d9d3465d573b13b7e19491d20f0927951433a232d1356b2f238150e25b053fd9b641290057cf7a7b68179ec7e5788396bb64d380fbee9d575f47677857b76aac6a44c02f856bfc9c8c7e1aabf3288f44c2f4c4c42ec8c8a1609c45d1ae775709f54cf0ecec35035a08d4f91d57d376802aea0244f2466b3db9773fce16fdc0e24d4cc7970a3a11950f8ca0950f0b921868a77571a50d3521b57e52b2fb096ce1dd01770456cb6cc364d1faead9783e24d351a0cd7c9aba9c99f70ecb83dcc48420c17f7a2e163c0e05a149ae90a62b337042e5572ea081f754a6ef5d75fa931cbaa9771776da6ecdbbe106bce6cf3e85913ddbd1e0e2562979cfccb90782d5ab0a644586eee9ab6f3fdbe5e137a62ceb7114ec8d8d6eeae80faefe09608783c2fc65e374a805b318f234948f3441a699446fb32cb1817c8723e82589d49a4c473e56a49305bc1cf6a6fe1ae956c5e8ec1dbbebd408057175bee1dca9eea698ed4c61eeecce959a8dc3686e64f53eb82102d200beada9772c161866ea20054df2442da0caac73cbc90b5873a1dd4416ec7b555ab6bad56cf173ce0a398819a4f32e245d3b93e4272394c31074d4bb2544f605d1663382e6c60f05f84e717d042337e6d9ea75be3003adccd0dacb91ff99df9c1bdf8fcac753da60f33676940193d40c759b14614b1dd9506903ff11e740c1a175a1fb76a47b6798cea48b11bdf111d34c1b42c58666a0044b9611a6086bb6f3aa816d4f27167c26961c4ee077a13f216ddbfb1249a889a65cccc8d80816358dcc459169cdee4770b4e2b34dbdf488c5a08ffd2d41be4c47037ea411e1a030c1df06322a8b9ce2d03e6e6e8b34eec40e0d5b15d1de2c5d19756357ea7289003bc31cd665b1d35c2e3b267304010c664e5db1628a2f362b711f71e358c4d77e21061ac0552993bd6f384c395e7b42c3777977d99b9af53aad96bb3e54fbada2f6e60937c69f5b33f90355cd18d9f09707765638ff4d9028a42465f5fd48530ffb107c0c566037104f78b18e9b59d90efc483f8ffc39c111d556fb6f699d1a4d0dc47d8c35c852e9cf44ecd6eed4aa916ff095a1682f31c27b25a698561ae624c6abc4c5c88eff0496e34676ca0088dd9ee80e63a26b3b1371284774420bb99de48d63a70b0a0731638b26b7e3ff59985e8102d017038cea042b16c271b69a0e121850a0ded0ff8392fba9a5ed04432deee62007669ce6bb3be16f3ae14fa803e0377c2d08aa8fe0e13d950e387451b792a2d1530bf4386d7057f4d4732d4ac94f551407b70b06fc0b0dd4256c4cbe821e1c430f5c6eac46b6c135cf897c9b3aa71176dd9a1131c4c5d4a4319b3abadefcaf39dc7599097639c847e6bc669ef5a477c05a0f3025f54c46bd0409848b88d4910857249478ffb4ec4c2696b89c1a59d586077bf9a766f7de77e82026069138d854cddfb4301b3723094396ea84fc39ddf286b82a2c792c16b9e174cad7ec3eecaabcda66d85c00aca4683c792d5c7cbbd01ed80a5b7e17115f00eaacbc38b129aa5c9cb149d7c78688a822db21893d44af88818bb385d4dc574225f0de6ed36502354e05c397931900c2113683f93ff9a62bdf4f5c26ed969c3ce90d02848393cd72940bafbc90b782b3e198ac40e64a8d6db2fdc082b3aa1214436e282bd2a4c9c10744a3a2e65543cc602fbad54a119509bb14964050f5e978e7d1ae5ef588e58c5bc1e1d673ead10a6ede6c3710b08204052d2681d0cfd43af4d44e89c565540645f2de26dbe9152cb58164405d3366f8b4634e40801ecb03d28c0cf9e4dc2cec350fde5c35e05f2dfce9aa90f461df4fb955defe0c4ab189275c767da5307e1d6a65").unwrap()).to_owned());
    }
}
