-- ประเภทสินค้า
CREATE TABLE products_type(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    products_type_name TEXT NOT NULL UNIQUE
);

-- สินค้า
CREATE TABLE products(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name_products TEXT NOT NULL,
    price DECIMAL(10 ,2) NOT NULL,
    detail TEXT NOT NULL,
    image_path TEXT NOT NULL,
    stock INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    products_type_id INTEGER,
    FOREIGN KEY(products_type_id) REFERENCES products_type(id)
);

CREATE TABLE images(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_path TEXT NOT NULL,
    product_id INTEGER,
    product_type_id INTEGER,
    FOREIGN KEY(product_id) REFERENCES products(id),
    FOREIGN KEY(product_type_id) REFERENCES products_type(id)
);

-- ที่อยู่
CREATE TABLE address(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    address_main TEXT NOT NULL,
    district TEXT NOT NULL,น
    province TEXT NOT NULL,
    zip_code INTEGER NOT NULL
);

-- ข้อมูลผู้ใช้
CREATE TABLE personal_data(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fullname TEXT NOT NULL,
    lastname TEXT NOT NULL,
    birthday DATE NOT NULL,
    address_id INTEGER,
    FOREIGN KEY(address_id) REFERENCES address(id)
);

-- ประเภทบัญชี (คนขาย, คนซื้อ)
CREATE TABLE account_type(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_type TEXT NOT NULL
);

-- ข้อมูลบัญชีผู้ใช้
CREATE TABLE account(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    account_type_id INTEGER NOT NULL,
    personal_data_id INTEGER,
    FOREIGN KEY(account_type_id) REFERENCES account_type(id),
    FOREIGN KEY(personal_data_id) REFERENCES personal_data(id)
);

-- ออเดอร์
CREATE TABLE order_data(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    products_id INTEGER NOT NULL,
    personal_data_id INTEGER NOT NULL,
    FOREIGN KEY(products_id) REFERENCES products(id),
    FOREIGN KEY(personal_data_id) REFERENCES personal_data(id)
);

-- สถานะออเดอร์
CREATE TABLE order_status(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    status TEXT NOT NULL
);

-- ออเดอร์
CREATE TABLE orders(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_data_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    order_status_id INTEGER NOT NULL,
    FOREIGN KEY(order_data_id) REFERENCES order_data(id),
    FOREIGN KEY(order_status_id) REFERENCES order_status(id)
);

-- วันที่จัดส่ง
CREATE TABLE delivery(
    Sent_date DATETIME NOT NULL,
    Delivery_date DATETIME NOT NULL,
    orders_id INTEGER NOT NULL,      
    FOREIGN KEY(orders_id) REFERENCES orders(id)
);