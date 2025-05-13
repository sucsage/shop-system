use reqwest::multipart::{Form, Part};
use crate::models::{ApiResponse, ProductType};
use std::env;

pub async fn fetch_products_types(page: u32, search: Option<String>) -> Result<ApiResponse<ProductType>, String> {
    let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
    let url = format!("{}/api/product-types", base_url);
    
    // สร้าง URL พร้อมพารามิเตอร์
    let mut query_params = vec![format!("page={}", page)];
    
    // เพิ่มพารามิเตอร์ search ถ้ามีค่า
    if let Some(search_term) = search {
        if !search_term.trim().is_empty() {
            query_params.push(format!("search={}", urlencoding::encode(&search_term)));
        }
    }
    
    // รวมพารามิเตอร์ทั้งหมดเข้ากับ URL
    let full_url = format!("{}?{}", url, query_params.join("&"));
    
    match reqwest::get(&full_url).await {
        Ok(response) => {
            let response_text = response.text().await.unwrap_or_default();
            match serde_json::from_str::<ApiResponse<ProductType>>(&response_text) {
                Ok(data) => Ok(data),
                Err(e) => Err(format!("JSON error: {}", e)),
            }
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

pub async fn fetch_all_product_types() -> Result<Vec<ProductType>, String> {
    let mut all_types = Vec::new();
    let mut page = 1;

    loop {
        // เรียกใช้ API หน้าปัจจุบัน
        match fetch_products_types(page, None).await {
            Ok(api_data) => {
                let current_page_data = api_data.data;
                let total_pages = api_data.pagination.total_pages;

                all_types.extend(current_page_data);

                if page >= total_pages {
                    break;
                }

                page += 1;
            }
            Err(err) => return Err(err),
        }pub async fn fetch_products_types(page: u32, search: Option<String>) -> Result<ApiResponse<ProductType>, String> {
            let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
            let url = format!("{}/api/product-types", base_url);
            
            // สร้าง URL พร้อมพารามิเตอร์
            let mut query_params = vec![format!("page={}", page)];
            
            // เพิ่มพารามิเตอร์ search ถ้ามีค่า
            if let Some(search_term) = search {
                if !search_term.trim().is_empty() {
                    query_params.push(format!("search={}", urlencoding::encode(&search_term)));
                }
            }
            
            // รวมพารามิเตอร์ทั้งหมดเข้ากับ URL
            let full_url = format!("{}?{}", url, query_params.join("&"));
            
            match reqwest::get(&full_url).await {
                Ok(response) => {
                    let response_text = response.text().await.unwrap_or_default();
                    match serde_json::from_str::<ApiResponse<ProductType>>(&response_text) {
                        Ok(data) => Ok(data),
                        Err(e) => Err(format!("JSON error: {}", e)),
                    }
                }
                Err(e) => Err(format!("Request error: {}", e)),
            }
        }
    }

    Ok(all_types)
}

pub async fn post_products_type(name: &str, file_paths: &[String]) -> Result<(), String> {
    let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
    let backend_url = format!("{}/api/product-types",base_url);
    let client = reqwest::Client::new();
    
    // สร้าง multipart form สำหรับส่งไปยัง backend
    let mut form = Form::new().text("name", name.to_string());
    
    // เพิ่มไฟล์รูปภาพเข้าไปใน form
    for file_path in file_paths {
        match tokio::fs::read(file_path).await {
            Ok(file_data) => {
                let file_name = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image.jpg")
                    .to_string();
                
                let part = Part::bytes(file_data)
                    .file_name(file_name)
                    .mime_str("image/jpeg")
                    .map_err(|e| format!("Invalid MIME type: {}", e))?;
                
                form = form.part("main_image[]", part);
            },
            Err(e) => return Err(format!("Failed to read file {}: {}", file_path, e)),
        }
    }
    
    // ส่ง POST request ไปยัง backend
    match client.post(backend_url)
        .multipart(form)
        .send()
        .await {
        Ok(response) => {
            // เก็บ status ไว้ใช้ก่อนที่จะเรียก response.text()
            let status = response.status();
            
            if status.is_success() || status.as_u16() == 302 {
                Ok(())
            } else {
                // ใช้ status ที่เก็บไว้แล้ว แทนที่จะเรียก response.status() อีกครั้ง
                let error_text = response.text().await.unwrap_or_default();
                Err(format!("Backend error: {}, Details: {}", status, error_text))
            }
        },
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

pub async fn delete_product_type(id: u64) -> Result<(), String> {
    let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
    let backend_url = format!("{}/api/product-types/{}", base_url, id);
    let client = reqwest::Client::new();

    match client.delete(&backend_url).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() || status.as_u16() == 302 {
                Ok(())
            } else {
                let error_text = response.text().await.unwrap_or_default();
                Err(format!("Backend error: {}, Details: {}", status, error_text))
            }
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}