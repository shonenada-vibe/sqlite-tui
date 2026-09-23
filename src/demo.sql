CREATE TABLE customers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    city TEXT,
    joined_at TEXT NOT NULL
);
CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    price REAL NOT NULL CHECK (price >= 0),
    stock INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    customer_id INTEGER NOT NULL REFERENCES customers(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    quantity INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    ordered_at TEXT NOT NULL
);
INSERT INTO customers VALUES
    (1, 'Alex Chen', 'alex@example.com', 'Shanghai', '2026-01-12'),
    (2, 'Sam Rivera', 'sam@example.com', 'Lisbon', '2026-02-03'),
    (3, 'Yuki Tanaka', 'yuki@example.com', 'Tokyo', '2026-02-18'),
    (4, 'Amara Okafor', 'amara@example.com', 'Lagos', '2026-03-05'),
    (5, 'Robin Kim', 'robin@example.com', 'Seoul', '2026-03-21'),
    (6, 'Noor Ali', 'noor@example.com', 'London', '2026-04-10'),
    (7, 'Jules Martin', 'jules@example.com', 'Paris', '2026-04-22'),
    (8, 'Taylor Brooks', 'taylor@example.com', 'Portland', '2026-05-01');
INSERT INTO products VALUES
    (1, 'Mechanical keyboard', 'Workspace', 129.00, 42),
    (2, 'Desk lamp', 'Workspace', 65.00, 18),
    (3, 'Field notebook', 'Stationery', 12.50, 120),
    (4, 'Ceramic mug', 'Everyday', 24.00, 67),
    (5, 'Canvas tote', 'Everyday', 32.00, 35),
    (6, 'Fountain pen', 'Stationery', 48.00, 24);
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<240)
INSERT INTO orders
SELECT x, (x%8)+1, (x%6)+1, (x%3)+1,
    CASE x%4 WHEN 0 THEN 'pending' WHEN 1 THEN 'shipped' WHEN 2 THEN 'delivered' ELSE 'processing' END,
    date('2026-06-01', '+' || (x%90) || ' days') FROM n;
CREATE VIEW order_summary AS
SELECT o.id, c.name AS customer, p.name AS product, o.quantity,
    round(p.price * o.quantity, 2) AS total, o.status, o.ordered_at
FROM orders o JOIN customers c ON c.id=o.customer_id JOIN products p ON p.id=o.product_id;
