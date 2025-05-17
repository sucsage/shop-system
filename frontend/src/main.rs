mod handlers;
mod models;

use actix_multipart::Multipart;
use actix_web::{App, Error, HttpResponse, HttpServer, Result, get, post, web};
use dotenvy::dotenv;
use futures_util::{StreamExt, TryStreamExt};
use image::ImageFormat;
use models::PageQuery;
use serde_json::Value;
use std::fs;
use std::io::Write;
use tera::{Context, Tera};

use crate::handlers::products::fetch_products;
use handlers::{
    products::{delete_product, post_products},
    products_type::{
        delete_product_type, delete_product_type_all, fetch_all_product_types, fetch_products_types, post_products_type
    },
};

use std::io::Cursor;

pub async fn get_products(tmpl: web::Data<Tera>, query: web::Query<PageQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let search = query.search.clone();
    let type_id = query.type_id.clone();

    let mut context = Context::new();

    // ดึงข้อมูลสินค้า
    match fetch_products(page, search.clone(), type_id).await {
        Ok(api_data) => {
            context.insert("pagination", &api_data.pagination);
            context.insert("products", &api_data.data);

            // หากสินค้ามี detail เป็น object JSON ให้ส่งเข้า template ด้วย
            if let Some(product) = api_data.data.get(0) {
                if let Value::Object(detail_map) = &product.detail {
                    context.insert("detail_map", detail_map);
                } else {
                    context.insert("detail_map", &serde_json::json!({}));
                }
            } else {
                context.insert("detail_map", &serde_json::json!({}));
            }
        }
        Err(err) => {
            context.insert("error", &err);
        }
    }

    // ✅ ดึงข้อมูลประเภทสินค้าเพิ่ม และส่งเข้า context
    match fetch_all_product_types().await {
        Ok(all_types) => {
            context.insert("product_types", &all_types);
        }
        Err(err) => {
            context.insert("product_type_error", &err);
        }
    }

    // แสดงผล template
    let rendered = match tmpl.render("products.html", &context) {
        Ok(html) => html,
        Err(err) => {
            println!("❌ Tera render error: {:?}", err);
            return HttpResponse::InternalServerError().body("Template render error");
        }
    };

    HttpResponse::Ok().content_type("text/html").body(rendered)
}

async fn get_product_types(tmpl: web::Data<Tera>, query: web::Query<PageQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let search = query.search.clone();

    let mut context = Context::new();

    // เพิ่มค่า search ไปยัง context เพื่อแสดงในช่องค้นหา (ถ้ามี)
    if let Some(search_term) = &search {
        context.insert("search", search_term);
    }

    match fetch_products_types(page, search).await {
        Ok(api_data) => {
            context.insert("pagination", &api_data.pagination);
            context.insert("product_types", &api_data.data);
        }
        Err(err) => {
            context.insert("error", &err);
        }
    }

    let rendered = match tmpl.render("type_products.html", &context) {
        Ok(html) => html,
        Err(err) => {
            println!("❌ Tera render error: {:?}", err);
            return HttpResponse::InternalServerError().body("Template render error");
        }
    };

    HttpResponse::Ok().content_type("text/html").body(rendered)
}

#[post("/api/product/upload")]
async fn post_product(mut payload: Multipart) -> Result<HttpResponse, Error> {
    let mut product_name = String::new();
    let mut product_price = 0.0;
    let mut product_detail = String::new();
    let mut temp_files = Vec::new();
    let mut product_stock = 0usize;
    let mut product_type_name = String::new();

    let temp_dir = "./temp_uploads";
    if !std::path::Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!(
                "Failed to create temp directory: {}",
                e
            ))
        })?;
    }

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field
            .content_disposition()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Content disposition not found"))?;

        let name = content_disposition.get_name().unwrap_or("");

        if name == "name" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            product_name = String::from_utf8(data).unwrap_or_default();
        } else if name == "price" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            let string_data = String::from_utf8(data).unwrap_or_default();
            product_price = string_data.parse::<f64>().unwrap_or(0.0);
        } else if name == "stock" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            let string_data = String::from_utf8(data).unwrap_or_default();
            product_stock = string_data.parse::<usize>().unwrap_or(0);
        } else if name == "detail" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            product_detail = String::from_utf8(data).unwrap_or_default();
        } else if name == "product_type_name" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            product_type_name = String::from_utf8(data).unwrap_or_default();
        } else if name == "main_image[]" {
            // อ่านและบันทึกไฟล์รูปภาพลงในโฟลเดอร์ชั่วคราว
            let filename = content_disposition
                .get_filename()
                .unwrap_or("unknown.jpg")
                .to_string();

            let filepath = format!("{}/{}", temp_dir, filename);
            let mut file = std::fs::File::create(&filepath).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Failed to create file: {}", e))
            })?;

            // อ่านไฟล์และเขียนลงในไฟล์ชั่วคราว
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading multipart chunk: {}",
                        e
                    ))
                })?;
                file.write_all(&data).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Failed to write to file: {}",
                        e
                    ))
                })?;
            }

            temp_files.push(filepath);
        }
    }
    match post_products(
        &product_name,
        &product_price,
        &product_stock,
        &product_detail,
        &product_type_name,
        &temp_files,
    )
    .await
    {
        Ok(_) => {
            // ลบไฟล์ชั่วคราวหลังจากส่งข้อมูลเสร็จ
            for file in &temp_files {
                let _ = fs::remove_file(file);
            }

            // Redirect กลับไปที่หน้า product-types
            Ok(HttpResponse::Found()
                .append_header(("Location", "/products"))
                .finish())
        }
        Err(e) => {
            // ลบไฟล์ชั่วคราวในกรณีเกิดข้อผิดพลาด
            for file in &temp_files {
                let _ = fs::remove_file(file);
            }

            Ok(HttpResponse::InternalServerError()
                .body(format!("Error forwarding to backend: {}", e)))
        }
    }
}

#[post("/api/product-type/upload")]
async fn post_product_type(mut payload: Multipart) -> Result<HttpResponse, Error> {
    let mut product_type_name = String::new();
    let mut temp_files = Vec::new();

    // สร้างโฟลเดอร์ชั่วคราวสำหรับเก็บไฟล์
    let temp_dir = "./temp_uploads";
    if !std::path::Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!(
                "Failed to create temp directory: {}",
                e
            ))
        })?;
    }

    // วนลูปอ่านข้อมูลจาก multipart form
    while let Ok(Some(mut field)) = payload.try_next().await {
        // แก้ไขการใช้งาน content_disposition
        let content_disposition = field
            .content_disposition()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Content disposition not found"))?;

        let name = content_disposition.get_name().unwrap_or("");

        if name == "name" {
            // อ่านชื่อประเภทสินค้า
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk_data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading field chunk: {}",
                        e
                    ))
                })?;
                data.extend_from_slice(&chunk_data);
            }
            product_type_name = String::from_utf8(data).unwrap_or_default();
        } else if name == "main_image[]" {
            // อ่านและบันทึกไฟล์รูปภาพลงในโฟลเดอร์ชั่วคราว
            let filename = content_disposition
                .get_filename()
                .unwrap_or("unknown.jpg")
                .to_string();

            let filepath = format!("{}/{}", temp_dir, filename);
            let mut file = std::fs::File::create(&filepath).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Failed to create file: {}", e))
            })?;

            // อ่านไฟล์และเขียนลงในไฟล์ชั่วคราว
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Error reading multipart chunk: {}",
                        e
                    ))
                })?;
                file.write_all(&data).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!(
                        "Failed to write to file: {}",
                        e
                    ))
                })?;
            }

            temp_files.push(filepath);
        }
    }

    // ตรวจสอบว่ามีข้อมูลครบถ้วนหรือไม่
    if product_type_name.is_empty() {
        return Ok(HttpResponse::BadRequest().body("Missing product type name"));
    }

    if temp_files.is_empty() {
        return Ok(HttpResponse::BadRequest().body("No images uploaded"));
    }

    // ส่งข้อมูลต่อไปยัง backend API
    match post_products_type(&product_type_name, &temp_files).await {
        Ok(_) => {
            // ลบไฟล์ชั่วคราวหลังจากส่งข้อมูลเสร็จ
            for file in &temp_files {
                let _ = fs::remove_file(file);
            }

            // Redirect กลับไปที่หน้า product-types
            Ok(HttpResponse::Found()
                .append_header(("Location", "/product-types"))
                .finish())
        }
        Err(e) => {
            // ลบไฟล์ชั่วคราวในกรณีเกิดข้อผิดพลาด
            for file in &temp_files {
                let _ = fs::remove_file(file);
            }

            Ok(HttpResponse::InternalServerError()
                .body(format!("Error forwarding to backend: {}", e)))
        }
    }
}

#[post("/api/products/delete")]
async fn delete_product_form(
    form: web::Form<std::collections::HashMap<String, String>>,
) -> impl actix_web::Responder {
    if let Some(id_str) = form.get("delete_id") {
        if let Ok(id) = id_str.parse::<u64>() {
            match delete_product(id).await {
                Ok(_) => {
                    return HttpResponse::Found()
                        .append_header(("Location", "/products"))
                        .finish();
                }
                Err(e) => {
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to delete: {}", e));
                }
            }
        }
    }
    HttpResponse::BadRequest().body("Invalid or missing delete_id")
}


#[post("/api/product-type-all/delete")]
async fn delete_product_type_all_form(
    form: web::Form<std::collections::HashMap<String, String>>,
) -> impl actix_web::Responder {
    if let Some(id_str) = form.get("delete_id") {
        if let Ok(id) = id_str.parse::<u64>() {
            match delete_product_type_all(id).await {
                Ok(_) => {
                    return HttpResponse::Found()
                        .append_header(("Location", "/product-types"))
                        .finish();
                }
                Err(e) => {
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to delete: {}", e));
                }
            }
        }
    }
    HttpResponse::BadRequest().body("Invalid or missing delete_id")
}

#[post("/api/product-type/delete")]
async fn delete_product_type_form(
    form: web::Form<std::collections::HashMap<String, String>>,
) -> impl actix_web::Responder {
    if let Some(id_str) = form.get("delete_id") {
        if let Ok(id) = id_str.parse::<u64>() {
            match delete_product_type(id).await {
                Ok(_) => {
                    return HttpResponse::Found()
                        .append_header(("Location", "/product-types"))
                        .finish();
                }
                Err(e) => {
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to delete: {}", e));
                }
            }
        }
    }
    HttpResponse::BadRequest().body("Invalid or missing delete_id")
}

#[get("/api/images/{tail:.*}")]
async fn proxy_images(path: web::Path<String>) -> Result<HttpResponse> {
    let rel_path = path.into_inner(); // เช่น: other/phone_0.jpg
    let target_url = format!("http://localhost:2001/{}", rel_path);

    let response = reqwest::get(&target_url).await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let bytes = resp.bytes().await;

                match bytes {
                    Ok(img_bytes) => {
                        if let Ok(img) = image::load_from_memory(&img_bytes) {
                            let mut buffer = Cursor::new(Vec::new());
                            if img.write_to(&mut buffer, ImageFormat::Png).is_ok() {
                                return Ok(HttpResponse::Ok()
                                    .content_type("image/png")
                                    .body(buffer.into_inner()));
                            } 
                        } 
                    }
                    Err(e) => {
                        println!("Failed to get image bytes: {}", e);
                    }
                }
            } 
        }
        Err(e) => {
            println!("Failed to fetch image from URL: {}", e);
        }
    }

    Ok(HttpResponse::BadRequest().body("Invalid or missing image"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let tera = Tera::new("public/**/*.html").unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(tera.clone()))
            .route("/products", web::get().to(get_products))
            .route("/product-types", web::get().to(get_product_types))
            .service(post_product_type)
            .service(delete_product_type_all_form)
            .service(delete_product_type_form)
            .service(post_product)
            .service(delete_product_form)
            .service(proxy_images)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
