use indexmap::IndexMap;

pub fn set<T: Clone>(object: &IndexMap<String, T>, key: String, value: T) -> IndexMap<String, T> {
    let mut output = object.clone();
    output.insert(key, value);
    output
}

pub fn delete<T: Clone>(object: &IndexMap<String, T>, key: &str) -> IndexMap<String, T> {
    let mut output = object.clone();
    output.shift_remove(key);
    output
}

pub fn concat<T: Clone>(
    objects: impl IntoIterator<Item = IndexMap<String, T>>,
) -> IndexMap<String, T> {
    let mut output = IndexMap::new();
    for object in objects {
        output.extend(object);
    }
    output
}
