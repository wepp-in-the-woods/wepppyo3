use arrow_array::StringArray;

pub fn string_array_from_strings(values: Vec<String>) -> StringArray {
    StringArray::from(values)
}

pub fn string_array_from_optional_strings(values: Vec<Option<String>>) -> StringArray {
    StringArray::from(values)
}
