mod bencode_decoder;

use std::{fs::File, io::Read};

use crate::bencode_decoder::Bencode;

fn main() {

    // println!("{}", torrent)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sha_info() {
        let mut file = File::open("test_folder.torrent").unwrap();
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
}
