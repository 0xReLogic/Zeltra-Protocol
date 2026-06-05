use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

pub fn serialize_to_bytes<T: CanonicalSerialize>(val: &T) -> Vec<u8> {
    let mut buf = vec![];
    val.serialize_compressed(&mut buf).unwrap();
    buf
}

pub fn deserialize_from_bytes<T: CanonicalDeserialize>(bytes: &[u8]) -> Option<T> {
    T::deserialize_compressed(bytes).ok()
}
