use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Shared canonical dataset types used by other crates.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub raw: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cameras {
    pub main: Option<String>,
    pub wide: Option<String>,
    pub ultra_wide: Option<String>,
    pub telephoto: Option<String>,
    pub macro_camera: Option<String>,
    pub selfie_camera: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub url: String,
    pub scraped_at: String, // ISO timestamp
    pub source_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub brand: Option<String>,
    pub model: String,
    pub model_id: Option<String>,
    pub release_date: Option<String>,
    pub dimensions: Option<String>,
    pub weight: Option<String>,
    pub display: Option<String>,
    pub os: Option<String>,
    pub chipset: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub memory: Option<String>,
    pub storage: Option<String>,
    pub cameras: Option<Cameras>,
    pub selfie_camera: Option<String>,
    pub battery: Option<String>,
    pub charging: Option<String>,
    pub sensors: Option<String>,
    pub price: Option<Price>,
    pub raw: BTreeMap<String, String>,
    pub sources: Vec<SourceInfo>,
}

impl Spec {
    pub fn new(model: String) -> Self {
        Self {
            brand: None,
            model,
            model_id: None,
            release_date: None,
            dimensions: None,
            weight: None,
            display: None,
            os: None,
            chipset: None,
            cpu: None,
            gpu: None,
            memory: None,
            storage: None,
            cameras: None,
            selfie_camera: None,
            battery: None,
            charging: None,
            sensors: None,
            price: None,
            raw: BTreeMap::new(),
            sources: Vec::new(),
        }
    }
}
