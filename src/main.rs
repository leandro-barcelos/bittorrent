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
    fn test_sha1_info() {
        let mut file = File::open("test.torrent").unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        if let (Bencode::Dictionary(metainfo), _) = Bencode::decode_value(content) {
            let info_bytes = metainfo["info"].clone().encode_value();

            let mut m = sha1_smol::Sha1::new();
            m.update(&info_bytes);

            assert_eq!(
                m.digest().to_string(),
                "9672993ca0d956e65eef7576dc34b0bd745e7c99"
            );
        };
    }
}
