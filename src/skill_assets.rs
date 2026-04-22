use std::borrow::Cow;

use rust_embed::RustEmbed;

use crate::error::BioMcpError;

#[derive(RustEmbed)]
#[folder = "skills/"]
struct EmbeddedSkillAssets;

pub(crate) fn iter() -> impl Iterator<Item = Cow<'static, str>> {
    EmbeddedSkillAssets::iter()
}

pub(crate) fn bytes(path: &str) -> Result<Cow<'static, [u8]>, BioMcpError> {
    EmbeddedSkillAssets::get(path)
        .map(|asset| asset.data)
        .ok_or_else(|| BioMcpError::NotFound {
            entity: "skill asset".into(),
            id: path.to_string(),
            suggestion: "Check the embedded skills/ tree".into(),
        })
}

pub(crate) fn text(path: &str) -> Result<String, BioMcpError> {
    String::from_utf8(bytes(path)?.into_owned()).map_err(|_| {
        BioMcpError::InvalidArgument(format!("Embedded skill asset is not valid UTF-8: {path}"))
    })
}
