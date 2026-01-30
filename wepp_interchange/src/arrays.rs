use std::collections::HashMap;

use arrow2::array::{DictionaryArray, PrimitiveArray, Utf8Array};

use crate::errors::InterchangeError;

pub fn dictionary_array_from_strings(values: Vec<String>) -> Result<DictionaryArray<i32>, InterchangeError> {
    let optional = values.into_iter().map(Some).collect::<Vec<_>>();
    dictionary_array_from_optional_strings(optional)
}

pub fn dictionary_array_from_optional_strings(
    values: Vec<Option<String>>,
) -> Result<DictionaryArray<i32>, InterchangeError> {
    let mut dict_values: Vec<String> = Vec::new();
    let mut dict_map: HashMap<String, i32> = HashMap::new();
    let mut keys: Vec<Option<i32>> = Vec::with_capacity(values.len());

    for value in values {
        match value {
            Some(val) => {
                if let Some(existing) = dict_map.get(&val) {
                    keys.push(Some(*existing));
                } else {
                    let idx = dict_values.len() as i32;
                    dict_values.push(val.clone());
                    dict_map.insert(val, idx);
                    keys.push(Some(idx));
                }
            }
            None => keys.push(None),
        }
    }

    let keys = PrimitiveArray::<i32>::from(keys);
    let dict_array = Utf8Array::<i32>::from_slice(dict_values).boxed();
    DictionaryArray::try_from_keys(keys, dict_array).map_err(InterchangeError::from)
}
