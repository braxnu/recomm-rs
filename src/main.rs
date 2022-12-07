#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::{sync::Mutex, collections::HashMap};
use serde::{Serialize, Deserialize};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, delete};
use mongodb::{Client, bson::doc, Database, options::FindOptions};
use env_logger;
use serde_json::json;

type ProductId = String;

#[derive(Serialize, Deserialize)]
struct Product {
    id: ProductId,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct OrderItem {
    product: Product,
    quantity: u16,
}

#[derive(Serialize, Deserialize)]
struct Order {
    id: String,
    items: Vec<OrderItem>,
}

struct AppState {
    orders: Vec<Order>,
}

#[get("/")]
async fn get_index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/orders")]
async fn post_order(state: web::Data<Mutex<AppState>>, order: web::Json<Order>) -> impl Responder {
    let order_id = &order.id;

    if order.items.len() == 0 {
        return HttpResponse::BadRequest().json(json!({
            "success": false,
            "reason": "no items",
        }));
    }

    let orders = &mut state.lock().unwrap().orders;

    if orders.iter().any(|o| { &o.id == order_id }) {
        return HttpResponse::Conflict().json(json!({
            "success": false,
            "reason": format!("order {} already exists", order_id),
        }));
    }

    orders.push(order.into_inner());

    HttpResponse::Ok().json(json!({
        "success": true,
    }))
}

#[get("/orders")]
async fn get_orders(state: web::Data<Mutex<AppState>>) -> impl Responder {
    let orders = & state.lock().unwrap().orders;

    HttpResponse::Ok().json(orders)
}

#[delete("/orders")]
async fn delete_orders(state: web::Data<Mutex<AppState>>) -> impl Responder {
    let s = &mut state.lock().unwrap();

    s.orders = vec![];

    HttpResponse::Accepted().json(json!({
        "success": true,
    }))
}

#[get("/products/{product_id}/bought_together")]
async fn get_bought_together(state: web::Data<Mutex<AppState>>, path: web::Path<ProductId>) -> impl Responder {
    let product_id = path.into_inner();
    let mut product_count_map: HashMap<ProductId, u32> = HashMap::new();
    let orders = & state.lock().unwrap().orders;

    for o in orders {
        if o.items.iter().any(|i| { i.product.id == product_id }) {
            for p in o.items.iter() {
                product_count_map.insert(
                    p.product.id.clone(),
                    product_count_map.get(&p.product.id).or(Some(&0)).unwrap() + 1
                );
            }
        }
    }

    let mut entries: Vec<(ProductId, u32)> =
        product_count_map
            .into_iter()
            .collect();

    let serached_product_index = entries.iter()
        .position(|p| {
            println!("{}", json!({"p": p}));

            p.0 == product_id
        })
        .unwrap();

    entries.remove(serached_product_index);
    entries.sort_by_key(|e| { e.1.clone() });
    entries.reverse();

    let last_index = std::cmp::min(10, entries.len());

    let product_list: Vec<ProductId> = entries[0..last_index].iter()
        .map(|e| { e.0.clone() })
        .collect();

    HttpResponse::Ok().json(product_list)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let state = web::Data::new(
        Mutex::new(
            AppState {
                orders: vec![],
            }
        )
    );

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(get_index)
            .service(get_bought_together)
            .service(get_orders)
            .service(delete_orders)
            .service(post_order)
    })
        .bind(("127.0.0.1", 4600))?
        .run()
        .await
}
