use reqwest::multipart::Form;

use crate::models::{ApiResponse, Products};
use std::env;

pub async fn fetch_products(
    page: u32,
    search: Option<String>,
    type_id: Option<String>,  // ✅ เปลี่ยนเป็น Option<String>
) -> Result<ApiResponse<Products>, String> {
    let base_url = env::var("API").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/api/products", base_url);
    
    // สร้าง query parameters
    let mut params = vec![("page", page.to_string())];
    
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            params.push(("search", search_term));
        }
    }

    // ✅ รองรับ "null" แบบ string
    if let Some(tid) = type_id {
        if tid == "null" {
            params.push(("type_id", "null".to_string()));  // ส่ง string "null"
        } else if let Ok(parsed_tid) = tid.parse::<i64>() {
            params.push(("type_id", parsed_tid.to_string()));
        }
    }

    let client = reqwest::Client::new();
    let request = client.get(&url).query(&params);
    
    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<Products>>().await {
                    Ok(data) => Ok(data),
                    Err(e) => Err(format!("Failed to parse JSON: {}", e)),
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                Err(format!("API error: {} - {}", status, error_text))
            }
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

pub async fn post_products(
    name: &str,
    price: &f64,
    stock: &usize,
    detail: &str,
    product_type_name: &str,
    file_paths: &[String]
) -> Result<(), String> {
    let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
    let backend_url = format!("{}/api/products",base_url);
    let client = reqwest::Client::new();

    let mut form = Form::new()
        .text("name", name.to_string())
        .text("price", price.to_string())
        .text("stock", stock.to_string())
        .text("detail", detail.to_string())
        .text("product_type_name", product_type_name.to_string())
    ;
    for file_path in file_paths {
        match tokio::fs::read(file_path).await {
            Ok(file_data) => {
                let file_name = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image.jpg")
                    .to_string();
                
                let part = reqwest::multipart::Part::bytes(file_data)
                    .file_name(file_name)
                    .mime_str("image/jpeg")
                    .map_err(|e| format!("Invalid MIME type: {}", e))?;
                
                form = form.part("main_image[]", part);
            },
            Err(e) => return Err(format!("Failed to read file {}: {}", file_path, e)),
        }
    }

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

pub async fn delete_product(id: u64) -> Result<(), String>{
    let base_url = env::var("API").unwrap_or_else(|_| "unknown".to_string());
    let backend_url = format!("{}/api/products/{}", base_url, id);
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