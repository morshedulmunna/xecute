//! An independent v2 contract; replace or extend this module as v2 evolves.
use xecute::prelude::*;

/// Version-two example controller, independent of the v1 resource collection.
#[controller("/status")]
pub struct StatusController;
#[routes]
impl StatusController {
    /// Report the contract served by this version group.
    #[get("")]
    #[api(summary = "Version two status", tag = "Status", operation_id = "v2Status")]
    #[resp(200, "Version two is available")]
    async fn status() -> Json<serde_json::Value> { Json(serde_json::json!({"api_version":"v2", "status":"ok"})) }
}
