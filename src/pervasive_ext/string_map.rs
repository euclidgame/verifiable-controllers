use crate::pervasive::prelude::*;
use crate::pervasive::string::*;

verus! {

#[verifier(external_body)]
pub struct StringMap {
    inner: std::collections::BTreeMap<std::string::String, std::string::String>,
}

impl StringMap {
    pub spec fn view(&self) -> Map<Seq<char>, Seq<char>>;

    #[verifier(external_body)]
    pub fn insert(&mut self, key: String, value: String) -> (old_v: Option<String>)
        ensures
            self@ == old(self)@.insert(key@, value@),
            old(self)@.contains_key(key@) == old_v.is_Some(),
            old_v.is_Some() ==> old_v.get_Some_0()@ == old(self)@[key@],
    {
        match self.inner.insert(key.into_rust_string(), value.into_rust_string()) {
            std::option::Option::Some(old_v) => Some(String::from_rust_string(old_v)),
            std::option::Option::None => None,
        }
    }

    #[verifier(external_body)]
    pub fn get<'a>(&'a self, key: String) -> (v: Option<StrSlice<'a>>)
        ensures
            self@.contains_key(key@) == v.is_Some(),
            v.is_Some() ==> v.get_Some_0()@ == self@[key@],
    {
        match self.inner.get(&key.into_rust_string()) {
            std::option::Option::Some(v) => Some(StrSlice::from_rust_str(v)),
            std::option::Option::None => None,
        }
    }
}

}
