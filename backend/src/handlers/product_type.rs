use crate::models::{
    NewProductType, PaginatedResponse, PaginationInfo, ProductType, Querysearchandpage,
};
use actix_web::{HttpResponse, Responder, get, post, put, web ,delete};
use sqlx::Row;
use sqlx::SqlitePool;

#[get("/api/product-types")]
pub async fn get_product_types(
    db: web::Data<SqlitePool>,
    query: web::Query<Querysearchandpage>,
) -> impl Responder {
    let search_term = &query.search.clone().unwrap_or_default();
    let page = query.page.unwrap_or(1);
    let items_per_page = 10;
    let offset = (page - 1) * items_per_page;

    let count_query = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(DISTINCT pt.id)
        FROM products_type pt
        WHERE pt.products_type_name LIKE '%' || ? || '%'
        "#,
    )
    .bind(search_term)
    .fetch_one(db.get_ref())
    .await;

    let rows = sqlx::query(
        r#"
        SELECT pt.id, pt.products_type_name, i.image_path
        FROM products_type pt
        LEFT JOIN images i ON i.product_type_id = pt.id
        WHERE pt.products_type_name LIKE '%' || ? || '%'
        ORDER BY pt.id, i.id
        LIMIT ? OFFSET ?;
        "#,
    )
    .bind(search_term)
    .bind(items_per_page)
    .bind(offset)
    .fetch_all(db.get_ref())
    .await;

    match (count_query, rows) {
        (Ok(total_count), Ok(result_rows)) => {
            let mut product_types: Vec<ProductType> = Vec::new();

            for row in result_rows {
                let id: i64 = row.get("id");
                let name: String = row.get("products_type_name");
                let image_path: Option<String> = row.get("image_path");

                if let Some(existing) = product_types.iter_mut().find(|pt| pt.id == id) {
                    if let Some(path) = image_path {
                        existing.images_path.push(path);
                    }
                } else {
                    let mut images = Vec::new();
                    if let Some(path) = image_path {
                        images.push(path);
                    }

                    product_types.push(ProductType {
                        id,
                        name,
                        images_path: images,
                    });
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
pub async fn post_product_types(
    db: web::Data<SqlitePool>,
    json: web::Json<NewProductType>,
) -> impl Responder {
    let name = &json.name;
    let images = &json.images_path;

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to begin transaction"),
    };

    let result = sqlx::query("INSERT INTO products_type (products_type_name) VALUES (?)")
        .bind(name)
        .execute(&mut *tx)
        .await;

    let insert_result = match result {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Insert failed"),
    };

    let product_type_id = insert_result.last_insert_rowid();

    for path in images {
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

    HttpResponse::Ok().body("Product type created")
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

#[delete("/api/product-type/{id}")]
pub async fn delete_product_type(
    db: web::Data<SqlitePool>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to begin transaction"),
    };

    // ลบรูปภาพทั้งหมด
    if let Err(_) = sqlx::query("DELETE FROM images WHERE product_type_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Failed to delete related images");
    }

    // ลบสินค้าที่เกี่ยวข้อง
    if let Err(_) = sqlx::query("DELETE FROM products WHERE products_type_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return HttpResponse::InternalServerError().body("Failed to delete related products");
    }

    // ลบประเภทสินค้า
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
