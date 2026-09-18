use serde_json::Value;

pub struct ApplyPatchTrace<'a> {
    pub source: &'a str,
    pub model: &'a str,
    pub call_id: &'a str,
    pub fc_id: &'a str,
    pub args_raw: &'a str,
    pub input: &'a str,
    pub interrupted: bool,
    pub json_truncation: Option<&'a str>,
    pub v4a_truncation: Option<&'a str>,
    pub v4a_validation: Option<(usize, &'a str)>,
    pub decision: &'a str,
    pub repairs: Option<&'a Value>,
}

pub fn emit(_trace: &ApplyPatchTrace) {}
