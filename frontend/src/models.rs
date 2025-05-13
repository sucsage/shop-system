use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize ,Debug)]
pub struct ApiResponse<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Deserialize, Serialize ,Debug)] // 👈 เพิ่ม Serialize ด้วย
pub struct Products {
    pub id: i32,
    pub name_product: String,
    pub price: f64,
    pub detail: Value,
    pub images_path: Vec<String>,
    pub stock: i32,
    pub create_at: String,
    pub products_type_id: Option<i32>,
    pub products_type_name: Option<String>,
}

#[derive(Serialize ,Deserialize)]
pub struct ProductType {
    pub id: i64,
    pub name: String,
    pub images_path: Vec<String>,
}

#[derive(Debug, Deserialize ,Serialize)]
pub struct Pagination {
    pub total_items: u32,
    pub items_per_page: u32,
    pub current_page: u32,
    pub total_pages: u32,
}
#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub search: Option<String>,
    pub type_id: Option<String>,
}
