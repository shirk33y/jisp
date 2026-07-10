pub fn prepend<T>(value: T, mut list: Vec<T>) -> Vec<T> {
    list.insert(0, value);
    list
}

pub fn append<T>(mut list: Vec<T>, value: T) -> Vec<T> {
    list.push(value);
    list
}

pub fn concat<T>(lists: impl IntoIterator<Item = Vec<T>>) -> Vec<T> {
    lists.into_iter().flatten().collect()
}

pub fn slice<T: Clone>(list: &[T], start: usize, end: usize) -> Option<Vec<T>> {
    list.get(start..end).map(ToOwned::to_owned)
}
