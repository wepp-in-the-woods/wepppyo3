use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::DictionaryArray;

use crate::errors::InterchangeError;

pub fn dictionary_array_from_strings(
    values: Vec<String>,
) -> Result<DictionaryArray<Int32Type>, InterchangeError> {
    let optional = values.into_iter().map(Some).collect::<Vec<_>>();
    dictionary_array_from_optional_strings(optional)
}

pub fn dictionary_array_from_optional_strings(
    values: Vec<Option<String>>,
) -> Result<DictionaryArray<Int32Type>, InterchangeError> {
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

    let keys = arrow_array::Int32Array::from(keys);
    let values = Arc::new(arrow_array::StringArray::from(dict_values)) as _;
    DictionaryArray::try_new(keys, values).map_err(InterchangeError::from)
}
