//! Music catalogs outside YouTube Music.

use crate::model::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Catalog {
    Zing,
    Nct,
}

impl Catalog {
    pub fn split_track(id: &str) -> Option<(Self, &str)> {
        let (prefix, key) = id.split_once(':')?;
        let catalog = match prefix {
            "zing" => Self::Zing,
            "nct" => Self::Nct,
            _ => return None,
        };
        Some((catalog, key))
    }

    pub fn owns_track(id: &str) -> bool {
        Self::split_track(id).is_some()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Zing => "Zing MP3",
            Self::Nct => "NhacCuaTui",
        }
    }

    pub fn media_headers(self) -> (&'static str, &'static str) {
        match self {
            Self::Zing => (crate::zing::UA, "https://zingmp3.vn/"),
            Self::Nct => (crate::nct::UA, "https://www.nhaccuatui.com/"),
        }
    }
}

pub struct SongShelf {
    pub title: String,
    pub tracks: Vec<Track>,
}
