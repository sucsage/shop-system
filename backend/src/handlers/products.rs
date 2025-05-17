use std::fs;
use std::io::Write;

use crate::models::{NewProducts, PaginatedResponse, PaginationInfo, Products, Querysearchandpage};
use actix_multipart::Multipart;
use actix_web::http::header;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use futures_util::StreamExt;
use sqlx::Row;
use sqlx::SqlitePool;

enum TypeIdFilter {
    None,
    IsNull,
    Equal(i64),
}

#[get("/api/products")]
pub async fn get_products(
    db: web::Data<SqlitePool>,
    query: web::Query<Querysearchandpage>,
) -> impl Responder {
    let search_term = &query.search.clone().unwrap_or_default();
    let items_per_page = 10;
    let mut page = query.page.unwrap_or(1);
    if page == 0 {
        page = 1;
    }
    let offset = (page - 1) * items_per_page;

    let type_filter = match &query.type_id {
        None => TypeIdFilter::None,
        Some(s) if s == "null" => TypeIdFilter::IsNull,
        Some(s) => match s.parse::<i64>() {
            Ok(id) => TypeIdFilter::Equal(id),
            Err(_) => TypeIdFilter::None,
        },
    };

    // สร้าง query พื้นฐานสำหรับนับจำนวนสินค้า
    let mut count_sql = String::from(
        r#"
        SELECT COUNT(*)
        FROM products p
        WHERE p.name_products LIKE '%' || ? || '%'
        "#,
    );

    // สร้าง query พื้นฐานสำหรับดึงข้อมูลสินค้า
    let mut products_sql = String::from(
        r#"
        SELECT 
            p.id,
            p.name_products,
            p.price,
            p.detail,
            p.stock,
            p.created_at,
            p.products_type_id,
            pt.products_type_name
        FROM 
            products p
        LEFT JOIN 
            products_type pt ON p.products_type_id = pt.id
        WHERE 
            p.name_products LIKE '%' || ? || '%'
        "#,
    );

    // เพิ่มเงื่อนไขกรองตาม type_id ถ้ามีการระบุ
    match type_filter {
        TypeIdFilter::IsNull => {
            count_sql.push_str(" AND p.products_type_id IS NULL");
            products_sql.push_str(" AND p.products_type_id IS NULL");
        }
        TypeIdFilter::Equal(_) => {
            count_sql.push_str(" AND p.products_type_id = ?");
            products_sql.push_str(" AND p.products_type_id = ?");
        }
        TypeIdFilter::None => {}
    }

    // เพิ่ม ORDER BY, LIMIT และ OFFSET ให้กับ query สินค้า
    products_sql.push_str(
        r#"
        ORDER BY 
            p.id
        LIMIT ? OFFSET ?;
        "#,
    );

    // สร้าง query สำหรับนับจำนวนสินค้า
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(search_term);

    // สร้าง query สำหรับดึงข้อมูลสินค้า
    let mut products_query = sqlx::query(&products_sql).bind(search_term);

    // เพิ่ม parameter type_id ถ้ามีการระบุ
    if let TypeIdFilter::Equal(id) = type_filter {
        count_query = count_query.bind(id);
        products_query = products_query.bind(id);
    }

    // เพิ่ม parameter limit และ offset
    products_query = products_query.bind(items_per_page).bind(offset);

    // ดำเนินการ query
    let count_result = count_query.fetch_one(db.get_ref()).await;
    let products_result = products_query.fetch_all(db.get_ref()).await;

    match (count_result, products_result) {
        (Ok(total_count), Ok(product_rows)) => {
            let mut products: Vec<Products> = Vec::new();

            // สร้าง vector เก็บ ID สินค้าที่ต้องการดึงรูปภาพ
            let product_ids: Vec<i64> = product_rows
                .iter()
                .map(|row| row.get::<i64, _>("id"))
                .collect();

            // สร้าง Products จากข้อมูลพื้นฐาน (ยังไม่มีรูปภาพ)
            for row in product_rows {
                let id: i64 = row.get("id");
                let name_product: String = row.get("name_products");
                let price: f64 = row.get("price");
                let detail: serde_json::Value = row.get("detail");
                let stock: i64 = row.get("stock");
                let create_at: chrono::NaiveDateTime = row.get("created_at");
                let products_type_id: Option<i64> = row.get("products_type_id");
                let products_type_name: Option<String> = row.get("products_type_name");

                products.push(Products {
                    id,
                    name_product,
                    price,
                    detail,
                    stock,
                    create_at,
                    products_type_id,
                    products_type_name,
                    images_path: Vec::new(), // เริ่มต้นด้วย vector ว่าง
                });
            }

            // ถ้ามีสินค้าที่ต้องดึงรูปภาพ
            if !product_ids.is_empty() {
                // ดึงรูปภาพแยกต่างหาก
                let images_query = sqlx::query(
                    r#"
                    SELECT product_id, image_path
                    FROM images
                    WHERE product_id IN (SELECT value FROM json_each(?))
                    ORDER BY product_id, id
                    "#,
                )
                .bind(serde_json::to_string(&product_ids).unwrap())
                .fetch_all(db.get_ref())
                .await;

                // เพิ่มรูปภาพเข้าไปในสินค้าที่ตรงกัน
                if let Ok(image_rows) = images_query {
                    for row in image_rows {
                        let product_id: i64 = row.get("product_id");
                        let image_path: String = row.get("image_path");

                        if image_path.starts_with("../databases/dbimages/") {
                            // แปลงเป็น path ที่ให้ frontend เรียกผ่าน endpoint ใหม่
                            let rel_path = image_path.replace("../databases/dbimages/", "");
                            let web_path = format!("/images/{}", rel_path); // ✅ เปลี่ยนตรงนี้

                            if let Some(product) = products.iter_mut().find(|p| p.id == product_id)
                            {
                                product.images_path.push(web_path);
                            }
                        }
                    }
                }
            }

            let total_pages = (total_count + items_per_page - 1) / items_per_page;

            let response = PaginatedResponse {
                data: products,
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

#[post("/api/products")]
pub async fn post_products(db: web::Data<SqlitePool>, mut payload: Multipart) -> impl Responder {
    let mut name_products = String::new();
    let mut price = 0.0;
    let mut detail = String::new();
    let mut main_image_paths = Vec::new();
    let mut stock = 0;
    let mut product_type_name = String::new();
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
            name_products = String::from_utf8(data).unwrap_or_default();
        }

        if name == "price" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }
            price = String::from_utf8(data)
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or(0.0);
        }

        if name == "detail" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }
            detail = String::from_utf8(data).unwrap_or_default();
        }

        if name == "stock" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }
            stock = String::from_utf8(data)
                .unwrap_or_default()
                .parse::<i32>()
                .unwrap_or(0);
        }

        if name == "product_type_name" {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }
            product_type_name = String::from_utf8(data).unwrap_or_default();
        }

        if name == "main_image" || name == "main_image[]" {
            // ตรวจสอบว่ามีชื่อประเภทสินค้าแล้วหรือยัง ถ้ายังว่างให้ใช้ชื่อชั่วคราว
            let temp_name = if name_products.is_empty() {
                "temp_product".to_string()
            } else {
                name_products.clone()
            };

            if product_type_name == "null" {
                product_type_name = "other".to_string();
            }
            let folder_path = format!(
                "../databases/dbimages/{}/{}",
                product_type_name, name_products
            );
            let filename = format!("{}_{}.jpg", temp_name, index);
            let file_path = format!("{}/{}", folder_path, filename);
            
            fs::create_dir_all(&folder_path).unwrap();

            if save_file(&mut field, &file_path).await.is_err() {
                return HttpResponse::InternalServerError().body("Failed to save file");
            }

            main_image_paths.push(file_path.clone());
            index += 1;
        }
    }

    let mut tx = match db.begin().await {
        Ok(tx) => {
            println!("✅ Started DB transaction");
            tx
        }
        Err(e) => {
            eprintln!("❌ Failed to begin transaction: {:?}", e);
            return HttpResponse::InternalServerError().body("Failed to begin transaction");
        }
    };

    // 🟡 หา id ของ products_type_name
    let products_type_id: Option<i64> =
        if !product_type_name.is_empty() && product_type_name != "other"{
            println!("🔍 Looking up product type: {}", product_type_name);
            let row = sqlx::query("SELECT id FROM products_type WHERE products_type_name = ?")
                .bind(&product_type_name)
                .fetch_optional(&mut *tx)
                .await;

            match row {
                Ok(Some(row)) => {
                    let id = row.get::<i64, _>("id");
                    println!("✅ Found product type ID: {}", id);
                    Some(id)
                }
                Ok(None) => {
                    println!("⚠️ Product type not found: {}", product_type_name);
                    return HttpResponse::BadRequest().body("Invalid product type name");
                }
                Err(e) => {
                    eprintln!("❌ DB error when looking for product type: {:?}", e);
                    return HttpResponse::InternalServerError().body("Failed to find product type");
                }
            }
        } else {
            println!("ℹ️ No product type provided");
            None
        };

    let result = sqlx::query(
        "INSERT INTO products (name_products, price, detail, stock, products_type_id)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&name_products)
    .bind(price)
    .bind(&detail)
    .bind(stock)
    .bind(products_type_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = result {
        eprintln!("❌ Failed to insert product: {:?}", e);
        return HttpResponse::InternalServerError().body("Insert failed");
    }

    let insert_result = result.unwrap();
    let product_id = insert_result.last_insert_rowid();
    println!("✅ Product inserted with ID: {}", product_id);

    for path in &main_image_paths {
        println!("📷 Inserting image path: {}", path);
        if let Err(e) = sqlx::query("INSERT INTO images (image_path, product_id) VALUES (?, ?)")
            .bind(path)
            .bind(product_id)
            .execute(&mut *tx)
            .await
        {
            eprintln!("❌ Failed to insert image {}: {:?}", path, e);
            return HttpResponse::InternalServerError().body("Insert image failed");
        }
    }

    if let Err(e) = tx.commit().await {
        eprintln!("❌ Failed to commit transaction: {:?}", e);
        return HttpResponse::InternalServerError().body("Transaction commit failed");
    }

    println!("✅ Product and images inserted successfully");
    HttpResponse::Found()
        .append_header((header::LOCATION, "http://localhost:8080/products"))
        .finish()
}

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

#[put("/api/products/{id}")]
pub async fn update_product(
    db: web::Data<SqlitePool>,
    path: web::Path<i64>,
    json: web::Json<NewProducts>,
) -> impl Responder {
    let product_id = path.into_inner();
    println!("🟢 Updating product with ID: {}", product_id);

    let name_product = &json.name_product;
    let price = &json.price;
    let images = &json.images_path;
    let detail = &json.detail;
    let stock = &json.stock;
    let products_type_name = &json.products_type_name;

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to begin transaction: {:?}", e);
            return HttpResponse::InternalServerError().body("Failed to begin transaction");
        }
    };

    let products_type_id: Option<i64> = if let Some(type_name) = products_type_name {
        let row = sqlx::query("SELECT id FROM products_type WHERE products_type_name = ?")
            .bind(type_name)
            .fetch_optional(&mut *tx)
            .await;

        match row {
            Ok(Some(row)) => Some(row.get("id")),
            Ok(None) => {
                return HttpResponse::BadRequest().body("Invalid product type name");
            }
            Err(e) => {
                eprintln!("❌ Error finding product type: {:?}", e);
                return HttpResponse::InternalServerError().body("Failed to find product type");
            }
        }
    } else {
        None
    };

    let result = sqlx::query(
        "UPDATE products
         SET name_products = ?, price = ?, detail = ?, stock = ?, products_type_id = ?
         WHERE id = ?",
    )
    .bind(name_product)
    .bind(price)
    .bind(detail)
    .bind(stock)
    .bind(products_type_id)
    .bind(product_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = result {
        eprintln!("❌ Failed to update product: {:?}", e);
        return HttpResponse::InternalServerError().body("Update failed");
    }

    // 🔴 ลบรูปเก่าก่อน
    if let Err(e) = sqlx::query("DELETE FROM images WHERE product_id = ?")
        .bind(product_id)
        .execute(&mut *tx)
        .await
    {
        eprintln!("❌ Failed to delete old images: {:?}", e);
        return HttpResponse::InternalServerError().body("Delete old images failed");
    }

    // 🟢 เพิ่มรูปใหม่
    for path in images {
        if let Err(e) = sqlx::query("INSERT INTO images (image_path, product_id) VALUES (?, ?)")
            .bind(path)
            .bind(product_id)
            .execute(&mut *tx)
            .await
        {
            eprintln!("❌ Failed to insert image {}: {:?}", path, e);
            return HttpResponse::InternalServerError().body("Insert image failed");
        }
    }

    if let Err(e) = tx.commit().await {
        eprintln!("❌ Failed to commit update transaction: {:?}", e);
        return HttpResponse::InternalServerError().body("Transaction commit failed");
    }

    println!("✅ Product updated successfully");
    HttpResponse::Ok().body("Product updated successfully")
}

#[delete("/api/products/{id}")]
pub async fn delete_product(db: web::Data<SqlitePool>, path: web::Path<i64>) -> impl Responder {
    let product_id = path.into_inner();
    println!("🔴 Deleting product with ID: {}", product_id);

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to begin transaction: {:?}", e);
            return HttpResponse::InternalServerError().body("Failed to begin transaction");
        }
    };

    let image_paths =
        match sqlx::query_scalar::<_, String>("SELECT image_path FROM images WHERE product_id = ?")
            .bind(product_id)
            .fetch_all(&mut *tx)
            .await
        {
            Ok(paths) => paths,
            Err(_) => {
                return HttpResponse::InternalServerError()
                    .body("Failed to fetch image path of products");
            }
        };

    if let Some(first_path) = image_paths.get(0) {
        let path = std::path::Path::new(first_path);

        // ลบโฟลเดอร์ระดับ parent (1 ระดับ) เช่น game/minecraft/minecraft_0.jpg -> ลบ minecraft
        if let Some(parent_folder) = path.parent() {
            if parent_folder.exists() {
                if let Err(e) = fs::remove_dir_all(parent_folder) {
                    return HttpResponse::InternalServerError().body(format!(
                        "Failed to delete folder: {}, {}",
                        e,
                        parent_folder.display()
                    ));
                }
            }
        }
    }

    // 🔴 ลบรูปภาพที่เกี่ยวข้อง
    if let Err(e) = sqlx::query("DELETE FROM images WHERE product_id = ?")
        .bind(product_id)
        .execute(&mut *tx)
        .await
    {
        eprintln!("❌ Failed to delete images: {:?}", e);
        return HttpResponse::InternalServerError().body("Failed to delete images");
    }

    // 🔴 ลบสินค้า
    let result = sqlx::query("DELETE FROM products WHERE id = ?")
        .bind(product_id)
        .execute(&mut *tx)
        .await;

    if let Err(e) = result {
        eprintln!("❌ Failed to delete product: {:?}", e);
        return HttpResponse::InternalServerError().body("Failed to delete product");
    }

    if let Err(e) = tx.commit().await {
        eprintln!("❌ Failed to commit transaction: {:?}", e);
        return HttpResponse::InternalServerError().body("Transaction commit failed");
    }

    println!("✅ Product and associated images deleted successfully");
    HttpResponse::Ok().body("Product deleted successfully")
}
