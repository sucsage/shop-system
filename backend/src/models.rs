use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::NaiveDateTime;
//ส่วนรับโครงสร้างข้อมูลของ products_type_colunm
#[derive(Serialize)]
pub struct ProductType {
    pub id: i64,
    pub name: String,
    pub images_path: Vec<String>,
}

#[derive(Deserialize)]
pub struct NewProductType {
    pub name: String,
    pub images_path: Vec<String>,
}

//ส่วนรับโครงสร้างข้อมูลของ products_colunm
#[derive(Serialize)]
pub struct Products{
    pub id:i64,
    pub name_product: String,
    pub price: f64,
    pub detail: Value,
    pub images_path: Vec<String>,
    pub stock:i64,
    pub create_at:NaiveDateTime,
    pub products_type_id: Option<i64>,
    pub products_type_name: Option<String>,
}

#[derive(Deserialize,Debug)]
pub struct NewProducts{
    pub name_product: String,
    pub price: f64,
    pub detail: Value,
    pub images_path: Vec<String>,
    pub stock:i64,
    pub products_type_name: Option<String>,
}

//ส่วนของsearch data
#[derive(Deserialize, Debug)]
pub struct Querysearchandpage {
    pub search: Option<String>,
    pub page: Option<i64>,
    pub type_id: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub data: T,
    pub pagination: PaginationInfo,
}
//ส่วนของpage 
#[derive(Serialize)]
pub struct PaginationInfo {
    pub total_items: i64,
    pub items_per_page: i64,
    pub current_page: i64,
    pub total_pages: i64,
}