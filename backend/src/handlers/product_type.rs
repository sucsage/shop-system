use crate::models::{
    NewProductType, PaginatedResponse, PaginationInfo, ProductType, Querysearchandpage,
};
use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use futures_util::StreamExt;
use sqlx::Row;
use sqlx::SqlitePool;
use std::{fs, io::Write};

#[get("/api/product-types")]
pub async fn get_product_types(
    db: web::Data<SqlitePool>,
    query: web::Query<Querysearchandpage>,
) -> impl Responder {
    let search_term = &query.search.clone().unwrap_or_default();
    let page = query.page.unwrap_or(1);
    let items_per_page = 10;
    let offset = (page - 1) * items_per_page;

    // คำนวณจำนวนประเภทสินค้าทั้งหมดที่ตรงกับการค้นหา
    let count_query = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM products_type pt
        WHERE pt.products_type_name LIKE '%' || ? || '%'
        "#,
    )
    .bind(search_term)
    .fetch_one(db.get_ref())
    .await;

    // ดึงข้อมูลประเภทสินค้าพื้นฐานก่อน - จำกัดแค่ items_per_page รายการ
    let types_query = sqlx::query(
        r#"
        SELECT 
            pt.id, 
            pt.products_type_name
        FROM 
            products_type pt
        WHERE 
            pt.products_type_name LIKE '%' || ? || '%'
        ORDER BY 
            pt.id
        LIMIT ? OFFSET ?;
        "#,
    )
    .bind(search_term)
    .bind(items_per_page)
    .bind(offset)
    .fetch_all(db.get_ref())
    .await;

    match (count_query, types_query) {
        (Ok(total_count), Ok(type_rows)) => {
            let mut product_types: Vec<ProductType> = Vec::new();

            // สร้าง vector เก็บ ID ประเภทสินค้าที่ต้องการดึงรูปภาพ
            let type_ids: Vec<i64> = type_rows
                .iter()
                .map(|row| row.get::<i64, _>("id"))
                .collect();

            // สร้าง ProductType จากข้อมูลพื้นฐาน (ยังไม่มีรูปภาพ)
            for row in type_rows {
                let id: i64 = row.get("id");
                let name: String = row.get("products_type_name");

                product_types.push(ProductType {
                    id,
                    name,
                    images_path: Vec::new(), // เริ่มต้นด้วย vector ว่าง
                });
            }

            // ถ้ามีประเภทสินค้าที่ต้องดึงรูปภาพ
            if !type_ids.is_empty() {
                // ดึงรูปภาพแยกต่างหาก
                let images_query = sqlx::query(
                    r#"
                    SELECT product_type_id, image_path
                    FROM images
                    WHERE product_type_id IN (SELECT value FROM json_each(?))
                    ORDER BY product_type_id, id
                    "#,
                )
                .bind(serde_json::to_string(&type_ids).unwrap())
                .fetch_all(db.get_ref())
                .await;

                // เพิ่มรูปภาพเข้าไปในประเภทสินค้าที่ตรงกัน
                if let Ok(image_rows) = images_query {
                    for row in image_rows {
                        let type_id: i64 = row.get("product_type_id");
                        let image_path: String = row.get("image_path");

                        // แปลง path จาก "../databases/dbimages/main/food_0.jpg" → "http://localhost:2001/images/main/food_0.jpg"
                        let web_path = image_path
                            .replace("../databases/dbimages/", "http://localhost:2001/images/");

                        if let Some(product_type) =
                            product_types.iter_mut().find(|pt| pt.id == type_id)
                        {
                            product_type.images_path.push(web_path);
                        }
                    }
                }
            }

            let total_pages = (total_count + items_per_page - 1) / items_per_page;

            let response = PaginatedResponse {
                data: product_types,
                pagination: PaginationInfo {
                    total_items: total_count,
                    items_per_page,
                    current_page: page,
                    total_pages,
                },
            };

            HttpResponse::Ok().json(response)
        }
        _ => HttpResponse::InternalServerError().body("Database query failed"),
    }
}

#[post("/api/product-types")]
pub async fn post_product_types(db: web::Data<SqlitePool>, mut payload: Multipart) -> HttpResponse {
    let mut product_type_name = String::new();
    let mut main_image_paths = Vec::new();
    let mut index = 0;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(_) => return HttpResponse::BadRequest().body("Invalid form data"),
        };

        let name = field.name().unwrap_or("").to_string();

        if name == "name" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }
            product_type_name = String::from_utf8(data).unwrap_or_default();
        }

        // รองรับ main_image[] สำหรับการอัปโหลดหลายไฟล์
        if name == "main_image" || name == "main_image[]" {
            // ตรวจสอบว่ามีชื่อประเภทสินค้าแล้วหรือยัง ถ้ายังว่างให้ใช้ชื่อชั่วคราว
            let temp_name = if product_type_name.is_empty() {
                "temp_product".to_string()
            } else {
                product_type_name.clone()
            };

            let folder_path = format!("../databases/dbimages/{}/main",product_type_name);
            let filename = format!("{}_{}.jpg", temp_name, index);
            let file_path = format!("{}/{}", folder_path, filename);

            fs::create_dir_all(&folder_path).unwrap();

            if let Err(_) = save_file(&mut field, &file_path).await {
                return HttpResponse::InternalServerError().body("Failed to save file");
            }

            main_image_paths.push(file_path.clone());
            index += 1;
        }
    }

    // ❗ ตรวจสอบว่ามีชื่อ
    if product_type_name.is_empty() {
        return HttpResponse::BadRequest().body("Missing product type name");
    }

    // ตรวจสอบว่ามีรูปภาพอย่างน้อย 1 รูป
    if main_image_paths.is_empty() {
        return HttpResponse::BadRequest().body("At least one image is required");
    }

    // 🔄 เริ่ม Transaction
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::InternalServerError().body("Failed to start DB transaction");
        }
    };

    // 🧠 Insert ชื่อประเภท
    let insert_result = sqlx::query("INSERT INTO products_type (products_type_name) VALUES (?)")
        .bind(&product_type_name)
        .execute(&mut *tx)
        .await;

    let result = match insert_result {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Insert product type failed"),
    };

    let product_type_id = result.last_insert_rowid();

    // 💾 Insert path รูป
    for path in &main_image_paths {
        if let Err(_) =
            sqlx::query("INSERT INTO images (image_path, product_type_id) VALUES (?, ?)")
                .bind(path)
                .bind(product_type_id)
                .execute(&mut *tx)
                .await
        {
            return HttpResponse::InternalServerError().body("Insert image failed");
        }
    }

    if let Err(_) = tx.commit().await {
        return HttpResponse::InternalServerError().body("Commit failed");
    }

    HttpResponse::Ok().body("Product type post successfully")
}

// ฟังก์ชันช่วยบันทึกไฟล์
async fn save_file(field: &mut actix_multipart::Field, filepath: &str) -> std::io::Result<()> {
    let mut f = std::fs::File::create(filepath)?;

    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("multipart error: {}", e))
        })?;
        f.write_all(&data)?;
    }

    Ok(())
}

#[delete("/api/product-types/{id}")]
pub async fn delete_product_type(
    db: web::Data<SqlitePool>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to begin transaction"),
    };

    // 🔍 ดึง path ของรูปภาพที่ต้องลบ
    let image_paths = match sqlx::query_scalar::<_, String>(
        "SELECT image_path FROM images WHERE product_type_id = ?",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    {
        Ok(paths) => paths,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to fetch image paths"),
    };
    
    // สมมุติว่า path มีรูปแบบเหมือนกันทั้งหมด
    if let Some(first_path) = image_paths.get(0) {
        let path = std::path::Path::new(first_path);
    
        // ลบ 2 ระดับสุดท้ายออก: game/minecraft/minecraft_0.jpg -> ../databases/dbimages/
        if let Some(grand_parent) = path.parent().and_then(|p| p.parent()) {
            if grand_parent.exists() {
                if let Err(e) = fs::remove_dir_all(grand_parent) {
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to delete directory:{} , {}",e, grand_parent.display()));
                }
            }
        }
    }

    // ลบจากฐานข้อมูล
    if let Err(_) = sqlx::query("DELETE FROM images WHERE product_type_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Failed to delete related images");
    }

    if let Err(_) = sqlx::query("DELETE FROM products WHERE products_type_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Failed to delete related products");
    }

    if let Err(_) = sqlx::query("DELETE FROM products_type WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Failed to delete product type");
    }

    if let Err(_) = tx.commit().await {
        return HttpResponse::InternalServerError().body("Failed to commit transaction");
    }

    HttpResponse::Ok().body("Product type deleted successfully")
}


#[put("/api/product-types/{id}")]
pub async fn update_product_type(
    db: web::Data<SqlitePool>,
    path: web::Path<i64>,
    json: web::Json<NewProductType>,
) -> impl Responder {
    let id = path.into_inner();
    let name = &json.name;
    let images = &json.images_path;

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to begin transaction"),
    };

    if let Err(_) = sqlx::query("UPDATE products_type SET products_type_name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Update failed");
    }

    if let Err(_) = sqlx::query("DELETE FROM images WHERE product_type_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Delete old images failed");
    }

    for path in images {
        if let Err(_) =
            sqlx::query("INSERT INTO images (image_path, product_type_id) VALUES (?, ?)")
                .bind(path)
                .bind(id)
                .execute(&mut *tx)
                .await
        {
            return HttpResponse::InternalServerError().body("Insert new image failed");
        }
    }

    if let Err(_) = tx.commit().await {
        return HttpResponse::InternalServerError().body("Commit failed");
    }

    HttpResponse::Ok().body("Product type updated")
}