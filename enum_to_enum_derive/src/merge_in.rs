use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

pub trait MergeIn {
    fn merge_in(&mut self, other: Self);
}

impl<K: Eq + Hash, Vs: MergeIn> MergeIn for HashMap<K, Vs> {
    fn merge_in(&mut self, other: Self) {
        other.into_iter().for_each(|(k, vs)| match self.entry(k) {
            Entry::Occupied(mut occ) => {
                occ.get_mut().merge_in(vs);
            }
            Entry::Vacant(vac) => {
                vac.insert(vs);
            }
        });
    }
}

impl<T> MergeIn for Vec<T> {
    fn merge_in(&mut self, other: Self) {
        self.extend(other);
    }
}
