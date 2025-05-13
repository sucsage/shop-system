use actix_files::NamedFile;
use actix_web::{get, web, Result};
use std::path::PathBuf;

#[get("/images/{file:.*}")]
pub async fn get_image(file: web::Path<String>) -> Result<NamedFile> {
    let base_path = PathBuf::from("../databases/dbimages");
    let file_path = base_path.join(file.into_inner());

    // ตรวจสอบว่าไฟล์มีอยู่จริงไหม
    if file_path.exists() {
        Ok(NamedFile::open(file_path)?)
    } else {
        Ok(NamedFile::open("../databases/dbimages/404.jpg")?) // ไฟล์ 404 หรือไฟล์ที่แสดงถ้าไม่พบไฟล์
    }
}
