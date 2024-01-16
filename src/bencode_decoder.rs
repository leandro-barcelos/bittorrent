use std::fmt::{Display, Write};

use indexmap::IndexMap;

#[derive(PartialEq, Debug, Clone)]
#[allow(dead_code)]
pub enum Bencode {
    String(Vec<u8>),
    Integer(i64),
    List(Vec<Bencode>),
    Dictionary(IndexMap<String, Bencode>),
}

impl Display for Bencode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bencode::String(s) => {
                if let Ok(string) = String::from_utf8(s.to_vec()) {
                    f.write_str(format!(r#""{string}""#).as_str())
                } else {
                    let hex_string: String =
                        s.iter().map(|&byte| format!("{:02X}", byte)).collect();
                    f.write_str(format!(r#""{hex_string}""#).as_str())
                }
            }
            Bencode::Integer(i) => f.write_str(format!("{i}").as_str()),
            Bencode::List(l) => {
                f.write_char('[')?;

                for (i, bencode) in l.iter().enumerate() {
                    f.write_str(format!("{bencode}").as_str())?;
                    if i + 1 < l.len() {
                        f.write_str(", ")?;
                    }
                }

                f.write_char(']')
            }
            Bencode::Dictionary(d) => {
                f.write_char('{')?;

                for (i, (key, value)) in d.iter().enumerate() {
                    f.write_str(format!(r#""{key}": {value}"#).as_str())?;
                    if i + 1 < d.len() {
                        f.write_str(", ")?;
                    }
                }

                f.write_char('}')
            }
        }
    }
}

impl Bencode {
    #[allow(dead_code)]
    pub fn decode_value(encoded_value: Vec<u8>) -> (Self, Vec<u8>) {
        // If encoded_value starts with a digit, it's a number
        match encoded_value.first().unwrap() {
            b'0'..=b'9' => {
                if let Some(index) = encoded_value.iter().position(|&c| c == b':') {
                    let (len_bytes, rest) = encoded_value.split_at(index);
                    let len_string = String::from_utf8(len_bytes.to_vec()).unwrap();

                    if let Ok(len) = len_string.parse::<usize>() {
                        return (
                            Bencode::String(rest[1..len + 1].to_vec()),
                            rest[len + 1..].to_vec(),
                        );
                    }
                }

                panic!("Error decoding Bencode string")
            }
            b'i' => {
                let mut split = encoded_value.split_at(1).1.splitn(2, |&c| c == b'e');

                let number_bytes = split.next().unwrap();
                let rest = split.next().unwrap();

                if number_bytes.first().unwrap() == &b'0' && number_bytes.len() > 1 {
                    panic!("All encodings with a leading zero are invalid, other than i0e")
                }

                if number_bytes == b"-0" {
                    panic!("i-0e is invalid")
                }

                let number_string = String::from_utf8(number_bytes.to_vec()).unwrap();
                let number = number_string.parse::<i64>().unwrap();

                return (Bencode::Integer(number), rest.to_vec());
            }
            b'l' => {
                let mut list_string = encoded_value.split_at(1).1.to_vec();

                let mut list = Vec::new();

                loop {
                    let (decoded_value, rest) = Self::decode_value(list_string.to_vec());
                    list.push(decoded_value);
                    if rest.first().unwrap() == &b'e' {
                        return (Bencode::List(list), rest.split_at(1).1.to_vec());
                    };

                    list_string = rest;
                }
            }
            b'd' => {
                let mut dict_string = encoded_value.split_at(1).1.to_vec();

                let mut dict = IndexMap::new();

                while let (Bencode::String(key_bytes), rest) =
                    Self::decode_value(dict_string.to_vec())
                {
                    let (value, rest) = Self::decode_value(rest);
                    dict.insert(String::from_utf8(key_bytes).unwrap(), value);
                    if rest.first().unwrap() == &b'e' {
                        return (Bencode::Dictionary(dict), rest.split_at(1).1.to_vec());
                    }

                    dict_string = rest;
                }

                panic!("Error decoding Bencode dictionary")
            }
            _ => panic!(
                "Unhandled encoded value: {}",
                String::from_utf8_lossy(&encoded_value)
            ),
        }
    }

    #[allow(dead_code)]
    pub fn encode_value(&mut self) -> Vec<u8> {
        match self {
            Bencode::String(s) => {
                let mut len = s.len().to_string().into_bytes();
                len.push(b':');
                len.append(s);

                return len;
            }
            Bencode::Integer(i) => return format!("i{i}e").into_bytes(),
            Bencode::List(l) => {
                let mut out = vec![b'l'];

                for value in l {
                    out.extend_from_slice(&Bencode::encode_value(value));
                }

                out.push(b'e');

                return out;
            }
            Bencode::Dictionary(d) => {
                let mut out = vec![b'd'];

                for (key, value) in d {
                    out.extend_from_slice(
                        &Bencode::String(key.clone().into_bytes()).encode_value(),
                    );
                    out.extend_from_slice(&value.encode_value());
                }

                out.push(b'e');

                return out;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bencode_string() {
        assert_eq!(
            Bencode::decode_value(b"3:Hey".to_vec()),
            (Bencode::String(b"Hey".to_vec()), vec![])
        );
        assert_eq!(
            Bencode::decode_value(b"4:Test".to_vec()),
            (Bencode::String(b"Test".to_vec()), vec![])
        )
    }

    #[test]
    fn decode_bencode_integer() {
        assert_eq!(
            Bencode::decode_value(b"i30e".to_vec()),
            (Bencode::Integer(30), vec![])
        );
        assert_eq!(
            Bencode::decode_value(b"i-42e".to_vec()),
            (Bencode::Integer(-42), vec![])
        );
    }

    #[test]
    fn decode_bencode_list() {
        assert_eq!(
            Bencode::decode_value(b"l4:spam4:eggse".to_vec()),
            (
                Bencode::List(vec![
                    Bencode::String(b"spam".to_vec()),
                    Bencode::String(b"eggs".to_vec())
                ]),
                vec![]
            )
        );
        assert_eq!(
            Bencode::decode_value(b"l5:helloi52ee".to_vec()),
            (
                Bencode::List(vec![
                    Bencode::String(b"hello".to_vec()),
                    Bencode::Integer(52)
                ]),
                vec![]
            )
        )
    }

    #[test]
    fn decode_bencode_nested_list() {
        assert_eq!(
            Bencode::decode_value(b"l4:spaml3:heyei52ee".to_vec()),
            (
                Bencode::List(vec![
                    Bencode::String(b"spam".to_vec()),
                    Bencode::List(vec![Bencode::String(b"hey".to_vec())]),
                    Bencode::Integer(52)
                ]),
                vec![]
            )
        );
    }

    #[test]
    fn decode_bencode_dictionary() {
        let mut test = IndexMap::new();
        test.insert("foo".to_string(), Bencode::String(b"bar".to_vec()));
        test.insert("hello".to_string(), Bencode::Integer(52));

        assert_eq!(
            Bencode::decode_value(b"d3:foo3:bar5:helloi52ee".to_vec()),
            (Bencode::Dictionary(test), vec![])
        )
    }

    #[test]
    fn decode_bencode_nested_dict() {
        let mut test_nested = IndexMap::new();
        test_nested.insert("hello".to_string(), Bencode::Integer(52));

        let mut test = IndexMap::new();
        test.insert("foo".to_string(), Bencode::String(b"bar".to_vec()));
        test.insert("hi".to_string(), Bencode::Dictionary(test_nested));

        assert_eq!(
            Bencode::decode_value(b"d3:foo3:bar2:hid5:helloi52eee".to_vec()),
            (Bencode::Dictionary(test), vec![])
        )
    }

    #[test]
    fn encode_bencode_string() {
        assert_eq!(
            Bencode::String(b"Hello".to_vec()).encode_value(),
            b"5:Hello".to_vec()
        )
    }

    #[test]
    fn encode_bencode_integer() {
        assert_eq!(Bencode::Integer(231).encode_value(), b"i231e".to_vec())
    }

    #[test]
    fn encode_bencode_list() {
        assert_eq!(
            Bencode::List(vec![
                Bencode::String(b"Test".to_vec()),
                Bencode::List(vec![Bencode::String(b"Hey".to_vec())]),
                Bencode::Integer(32)
            ])
            .encode_value(),
            b"l4:Testl3:Heyei32ee".to_vec()
        )
    }

    #[test]
    fn encode_bencode_dict() {
        let mut test_nested = IndexMap::new();
        test_nested.insert("hello".to_string(), Bencode::Integer(52));

        let mut test = IndexMap::new();
        test.insert("foo".to_string(), Bencode::String(b"bar".to_vec()));
        test.insert("hi".to_string(), Bencode::Dictionary(test_nested));

        assert_eq!(
            Bencode::Dictionary(test).encode_value(),
            b"d3:foo3:bar2:hid5:helloi52eee".to_vec()
        )
    }
}
