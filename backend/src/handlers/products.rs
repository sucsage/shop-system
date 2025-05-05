use crate::models::{NewProducts, PaginatedResponse, PaginationInfo, Products, Querysearchandpage};
use actix_web::{HttpResponse, Responder, get, post, put, web , delete};
use rust_decimal::prelude::*;
use sqlx::Row;
use sqlx::SqlitePool;

#[get("/api/products")]
pub async fn get_products(
    db: web::Data<SqlitePool>,
    query: web::Query<Querysearchandpage>,
) -> impl Responder {
    let search_term = &query.search.clone().unwrap_or_default();
    let page = query.page.unwrap_or(1);
    let items_per_page = 10;
    let offset = (page - 1) * items_per_page;

    let count_query = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(DISTINCT p.id)
        FROM products p
        WHERE p.name_products LIKE '%' || ? || '%'
        "#
    )
    .bind(search_term)
    .fetch_one(db.get_ref())
    .await;

    let rows = sqlx::query(
        r#"
        SELECT 
            p.id,
            p.name_products,
            p.price,
            p.detail,
            p.stock,
            p.created_at,
            pt.products_type_name,
            i.image_path
        FROM 
            products p
        LEFT JOIN 
            products_type pt ON p.products_type_id = pt.id
        LEFT JOIN 
            images i ON p.id = i.product_id
        WHERE 
            p.name_products LIKE '%' || ? || '%'
        ORDER BY 
            p.id, i.id
        LIMIT ? OFFSET ?;
        "#
    )
    .bind(search_term)
    .bind(items_per_page)
    .bind(offset)
    .fetch_all(db.get_ref())
    .await;

    match (count_query, rows) {
        (Ok(total_count), Ok(result_rows)) => {
            let mut products: Vec<Products> = Vec::new();

            for row in result_rows {
                let id: i64 = row.get("id");
                let name_product: String = row.get("name_products");

                // FIX: read as f64 and convert to Decimal
                let price: f64 = row.get("price");
                let detail: serde_json::Value = row.get("detail");
                let stock: i64 = row.get("stock");
                let create_at: chrono::NaiveDateTime = row.get("created_at");
                let products_type_name: Option<String> = row.get("products_type_name");
                let image_path: Option<String> = row.get("image_path");

                if let Some(existing) = products.iter_mut().find(|p| p.id == id) {
                    if let Some(path) = image_path {
                        existing.images_path.push(path);
                    }
                } else {
                    let mut images = Vec::new();
                    if let Some(path) = image_path {
                        images.push(path);
                    }

                    products.push(Products {
                        id,
                        name_product,
                        price,
                        detail,
                        stock,
                        create_at,
                        products_type_id: None,
                        products_type_name,
                        images_path: images,
                    });
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
pub async fn post_products(
    db: web::Data<SqlitePool>,
    json: web::Json<NewProducts>
) -> impl Responder {
    println!("🔵 Received request: {:?}", json);

    let name_product = &json.name_product;
    let price = &json.price;
    let images = &json.images_path;
    let detail = &json.detail;
    let stock = &json.stock;
    let products_type_name = &json.products_type_name;


    let mut tx = match db.begin().await {
        Ok(tx) => {
            println!("✅ Started DB transaction");
            tx
        },
        Err(e) => {
            eprintln!("❌ Failed to begin transaction: {:?}", e);
            return HttpResponse::InternalServerError().body("Failed to begin transaction");
        }
    };

    // 🟡 หา id ของ products_type_name
    let products_type_id: Option<i64> = if let Some(type_name) = products_type_name {
        println!("🔍 Looking up product type: {}", type_name);
        let row = sqlx::query("SELECT id FROM products_type WHERE products_type_name = ?")
            .bind(type_name)
            .fetch_optional(&mut *tx)
            .await;

        match row {
            Ok(Some(row)) => {
                let id = row.get::<i64, _>("id");
                println!("✅ Found product type ID: {}", id);
                Some(id)
            },
            Ok(None) => {
                println!("⚠️ Product type not found: {}", type_name);
                return HttpResponse::BadRequest().body("Invalid product type name");
            },
            Err(e) => {
                eprintln!("❌ DB error when looking for product type: {:?}", e);
                return HttpResponse::InternalServerError().body("Failed to find product type");
            }
        }
    } else {
        println!("ℹ️ No product type provided");
        None
    };

    // 🔵 Insert product พร้อม products_type_id
    println!("📝 Inserting product: {}, price: {}, stock: {}", name_product, price, stock);

    let result = sqlx::query(
        "INSERT INTO products (name_products, price, detail, stock, products_type_id)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(name_product)
    .bind(price.to_f64().unwrap_or(0.0))
    .bind(detail)
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

    for path in images {
        println!("📷 Inserting image path: {}", path);
        if let Err(e) =
            sqlx::query("INSERT INTO images (image_path, product_id) VALUES (?, ?)")
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
    HttpResponse::Ok().body("Product inserted successfully")
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
            },
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
         WHERE id = ?"
    )
    .bind(name_product)
    .bind(price.to_f64().unwrap_or(0.0))
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
pub async fn delete_product(
    db: web::Data<SqlitePool>,
    path: web::Path<i64>,
) -> impl Responder {
    let product_id = path.into_inner();
    println!("🔴 Deleting product with ID: {}", product_id);

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to begin transaction: {:?}", e);
            return HttpResponse::InternalServerError().body("Failed to begin transaction");
        }
    };

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
