#![allow(unused)]
use seed::browser::fetch::{Request, Header};
use seed::fetch::Method;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde::Deserialize;

const FRAGMENT: &'static AsciiSet = &CONTROLS.add(b' ');

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RidingLookupResult {
    electoral_districts: Vec<ElectoralDistrict>,
    poll_division_ids: Vec<u32>,
    postal_code: String,
    street_name: String,
    street_direction_id: Option<String>,
    street_type_id: String,
    place_name: String,
    street_name_display_text: String,
}

#[derive(Deserialize)]
pub struct ElectoralDistrict {
    election: Option<String>,
    id: u16,
    name: String,
}

pub async fn lookup_postal_code(code: &str) -> String {
    let mut results = Request::new(format!("/api/postal_code?query={}", utf8_percent_encode(code, FRAGMENT)))
        .method(Method::Get)
        .fetch()
        .await.unwrap()
        .json::<Vec<RidingLookupResult>>()
        .await.unwrap();
    let first_result = results.first().unwrap();
    let district = first_result.electoral_districts.first().unwrap();
    district.name.clone().replace("—", "-").replace("—", "-")
}
