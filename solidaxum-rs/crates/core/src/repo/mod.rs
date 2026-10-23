pub fn str_trim_or_empty(opt: &Option<String>) -> String {
    opt.as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}
