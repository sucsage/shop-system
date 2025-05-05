mod db;
mod models;
mod handlers;

use actix_web::{App, HttpServer};
use db::init_db;
use handlers::product_type::{delete_product_type, get_product_types, post_product_types, update_product_type};
use handlers::products::{get_products , post_products ,update_product ,delete_product};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = init_db().await;

    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(pool.clone()))
            //product_types
            .service(get_product_types)
            .service(post_product_types)
            .service(update_product_type)
            .service(delete_product_type)
            //products
            .service(get_products)
            .service(post_products)
            .service(update_product)
            .service(delete_product)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
