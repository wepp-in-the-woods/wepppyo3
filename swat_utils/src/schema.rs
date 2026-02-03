#[derive(Debug, Clone, Copy)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
}

impl VersionInfo {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}
